use pinocchio::{
    account_info::AccountInfo,
    instruction::Signer,
    program_error::ProgramError,
    pubkey::create_program_address,
    seeds,
    ProgramResult,
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_token::{
    instructions::{CloseAccount, Transfer},
    state::TokenAccount,
};

use crate::{
    helpers::{AssociatedTokenAccount, MintInterface, ProgramAccount, SignerAccount},
    state::Escrow,
    ESCROW_SEED, ID,
};

/// Take accounts structure
pub struct TakeAccounts<'a> {
    pub taker: &'a AccountInfo,
    pub maker: &'a AccountInfo,
    pub escrow: &'a AccountInfo,
    pub mint_a: &'a AccountInfo,
    pub mint_b: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub taker_ata_a: &'a AccountInfo,
    pub taker_ata_b: &'a AccountInfo,
    pub maker_ata_b: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub associated_token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for TakeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [taker, maker, escrow, mint_a, mint_b, vault, taker_ata_a, taker_ata_b, maker_ata_b, system_program, token_program, associated_token_program, _remaining @ ..] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // Basic account checks
        SignerAccount::check(taker)?;
        ProgramAccount::check(escrow)?;
        MintInterface::check(mint_a)?;
        MintInterface::check(mint_b)?;
        AssociatedTokenAccount::check(taker_ata_b, taker, mint_b, token_program)?;
        AssociatedTokenAccount::check(vault, escrow, mint_a, token_program)?;

        Ok(Self {
            taker,
            maker,
            escrow,
            mint_a,
            mint_b,
            vault,
            taker_ata_a,
            taker_ata_b,
            maker_ata_b,
            system_program,
            token_program,
            associated_token_program,
        })
    }
}

/// Take instruction - accepts an escrow offer
pub struct Take<'a> {
    pub accounts: TakeAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountInfo]> for Take<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let accounts = TakeAccounts::try_from(accounts)?;

        // Initialize taker's Token A account if needed
        CreateIdempotent {
            funding_account: accounts.taker,
            account: accounts.taker_ata_a,
            wallet: accounts.taker,
            mint: accounts.mint_a,
            system_program: accounts.system_program,
            token_program: accounts.token_program,
        }
        .invoke()?;

        // Initialize maker's Token B account if needed
        CreateIdempotent {
            funding_account: accounts.taker,
            account: accounts.maker_ata_b,
            wallet: accounts.maker,
            mint: accounts.mint_b,
            system_program: accounts.system_program,
            token_program: accounts.token_program,
        }
        .invoke()?;

        Ok(Self { accounts })
    }
}

impl<'a> Take<'a> {
    /// Instruction discriminator
    pub const DISCRIMINATOR: &'static u8 = &1;

    /// Process the take instruction
    pub fn process(&mut self) -> ProgramResult {
        let data = self.accounts.escrow.try_borrow_data()?;
        let escrow = Escrow::load(&data)?;

        // Check if the escrow is valid
        let escrow_key = create_program_address(
            &[
                ESCROW_SEED,
                self.accounts.maker.key(),
                &escrow.seed.to_le_bytes(),
                &escrow.bump,
            ],
            &ID,
        )?;
        if &escrow_key != self.accounts.escrow.key() {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // Prepare signer seeds
        let seed_bytes = escrow.seed.to_le_bytes();
        let bump_bytes = escrow.bump;
        let signer_seeds = seeds!(
            ESCROW_SEED,
            self.accounts.maker.key().as_ref(),
            seed_bytes.as_ref(),
            bump_bytes.as_ref()
        );
        let signer = Signer::from(&signer_seeds);

        // Get vault balance
        let amount = TokenAccount::from_account_info(self.accounts.vault)?.amount();

        // Transfer from the Vault to the Taker
        Transfer {
            from: self.accounts.vault,
            to: self.accounts.taker_ata_a,
            authority: self.accounts.escrow,
            amount,
        }
        .invoke_signed(&[signer.clone()])?;

        // Close the Vault
        CloseAccount {
            account: self.accounts.vault,
            destination: self.accounts.maker,
            authority: self.accounts.escrow,
        }
        .invoke_signed(&[signer.clone()])?;

        // Transfer from the Taker to the Maker
        Transfer {
            from: self.accounts.taker_ata_b,
            to: self.accounts.maker_ata_b,
            authority: self.accounts.taker,
            amount: escrow.receive,
        }
        .invoke()?;

        // Close the Escrow
        drop(data);
        ProgramAccount::close(self.accounts.escrow, self.accounts.taker)?;

        Ok(())
    }
}
