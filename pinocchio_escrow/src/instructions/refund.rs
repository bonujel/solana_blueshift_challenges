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

/// Refund accounts structure
pub struct RefundAccounts<'a> {
    pub maker: &'a AccountInfo,
    pub escrow: &'a AccountInfo,
    pub mint_a: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub maker_ata_a: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for RefundAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [maker, escrow, mint_a, vault, maker_ata_a, system_program, token_program, _remaining @ ..] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // Basic account checks
        SignerAccount::check(maker)?;
        ProgramAccount::check(escrow)?;
        MintInterface::check(mint_a)?;
        AssociatedTokenAccount::check(vault, escrow, mint_a, token_program)?;

        // 确保 maker 的 ATA 存在（不存在时自动创建）
        CreateIdempotent {
            funding_account: maker,
            account: maker_ata_a,
            wallet: maker,
            mint: mint_a,
            system_program,
            token_program,
        }
        .invoke()?;

        // 再次校验 maker ATA 的归属与派生地址
        AssociatedTokenAccount::check(maker_ata_a, maker, mint_a, token_program)?;

        Ok(Self {
            maker,
            escrow,
            mint_a,
            vault,
            maker_ata_a,
            system_program,
            token_program,
        })
    }
}

/// Refund instruction - cancels an escrow offer
pub struct Refund<'a> {
    pub accounts: RefundAccounts<'a>,
}

impl<'a> TryFrom<&'a [AccountInfo]> for Refund<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let accounts = RefundAccounts::try_from(accounts)?;
        Ok(Self { accounts })
    }
}

impl<'a> Refund<'a> {
    /// Instruction discriminator
    pub const DISCRIMINATOR: &'static u8 = &2;

    /// Process the refund instruction
    pub fn process(&mut self) -> ProgramResult {
        let data = self.accounts.escrow.try_borrow_data()?;
        let escrow = Escrow::load(&data)?;

        // Check if maker matches
        if &escrow.maker != self.accounts.maker.key() {
            return Err(ProgramError::IllegalOwner);
        }

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

        // Transfer from vault back to maker
        Transfer {
            from: self.accounts.vault,
            to: self.accounts.maker_ata_a,
            authority: self.accounts.escrow,
            amount,
        }
        .invoke_signed(&[signer.clone()])?;

        // Close the vault
        CloseAccount {
            account: self.accounts.vault,
            destination: self.accounts.maker,
            authority: self.accounts.escrow,
        }
        .invoke_signed(&[signer.clone()])?;

        // Close the escrow
        drop(data);
        ProgramAccount::close(self.accounts.escrow, self.accounts.maker)?;

        Ok(())
    }
}
