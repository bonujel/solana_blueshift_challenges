use core::mem::size_of;

use pinocchio::{
    account_info::AccountInfo,
    instruction::Signer,
    program_error::ProgramError,
    pubkey::find_program_address,
    seeds,
    sysvars::Sysvar,
    ProgramResult,
};
use pinocchio_associated_token_account::instructions::Create;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::Transfer;

use crate::{
    helpers::{AssociatedTokenAccount, MintInterface, SignerAccount},
    state::Escrow,
    ESCROW_SEED, ID,
};

/// Make accounts structure
pub struct MakeAccounts<'a> {
    pub maker: &'a AccountInfo,
    pub escrow: &'a AccountInfo,
    pub mint_a: &'a AccountInfo,
    pub mint_b: &'a AccountInfo,
    pub maker_ata_a: &'a AccountInfo,
    pub vault: &'a AccountInfo,
    pub system_program: &'a AccountInfo,
    pub token_program: &'a AccountInfo,
    pub associated_token_program: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for MakeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [maker, escrow, mint_a, mint_b, maker_ata_a, vault, system_program, token_program, associated_token_program, _remaining @ ..] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // Basic account checks
        SignerAccount::check(maker)?;
        MintInterface::check(mint_a)?;
        MintInterface::check(mint_b)?;
        AssociatedTokenAccount::check(maker_ata_a, maker, mint_a, token_program)?;

        Ok(Self {
            maker,
            escrow,
            mint_a,
            mint_b,
            maker_ata_a,
            vault,
            system_program,
            token_program,
            associated_token_program,
        })
    }
}

/// Make instruction data
pub struct MakeInstructionData {
    pub seed: u64,
    pub receive: u64,
    pub amount: u64,
}

impl<'a> TryFrom<&'a [u8]> for MakeInstructionData {
    type Error = ProgramError;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if data.len() != size_of::<u64>() * 3 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let seed = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let receive = u64::from_le_bytes(data[8..16].try_into().unwrap());
        let amount = u64::from_le_bytes(data[16..24].try_into().unwrap());

        // Instruction checks
        if amount == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(Self {
            seed,
            receive,
            amount,
        })
    }
}

/// Make instruction - creates an escrow offer
pub struct Make<'a> {
    pub accounts: MakeAccounts<'a>,
    pub instruction_data: MakeInstructionData,
    pub bump: u8,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountInfo])> for Make<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        let accounts = MakeAccounts::try_from(accounts)?;
        let instruction_data = MakeInstructionData::try_from(data)?;

        // Derive escrow PDA and get bump
        let (_, bump) = find_program_address(
            &[
                ESCROW_SEED,
                accounts.maker.key().as_ref(),
                &instruction_data.seed.to_le_bytes(),
            ],
            &ID,
        );

        // Prepare seeds for PDA initialization
        let seed_bytes = instruction_data.seed.to_le_bytes();
        let bump_bytes = [bump];
        let signer_seeds = seeds!(
            ESCROW_SEED,
            accounts.maker.key().as_ref(),
            seed_bytes.as_ref(),
            bump_bytes.as_ref()
        );
        let signer = Signer::from(&signer_seeds);

        // Get rent
        let rent = pinocchio::sysvars::rent::Rent::get()?;

        // Initialize the escrow account
        CreateAccount {
            from: accounts.maker,
            to: accounts.escrow,
            lamports: rent.minimum_balance(Escrow::LEN),
            space: Escrow::LEN as u64,
            owner: &ID,
        }
        .invoke_signed(&[signer])?;

        // Initialize the vault via ATA program CPI
        Create {
            funding_account: accounts.maker,
            account: accounts.vault,
            wallet: accounts.escrow,
            mint: accounts.mint_a,
            system_program: accounts.system_program,
            token_program: accounts.token_program,
        }
        .invoke()?;

        Ok(Self {
            accounts,
            instruction_data,
            bump,
        })
    }
}

impl<'a> Make<'a> {
    /// Instruction discriminator
    pub const DISCRIMINATOR: &'static u8 = &0;

    /// Process the make instruction
    pub fn process(&mut self) -> ProgramResult {
        // Populate the escrow account
        let mut data = self.accounts.escrow.try_borrow_mut_data()?;
        let escrow = Escrow::load_mut(data.as_mut())?;

        escrow.set_inner(
            self.instruction_data.seed,
            *self.accounts.maker.key(),
            *self.accounts.mint_a.key(),
            *self.accounts.mint_b.key(),
            self.instruction_data.receive,
            [self.bump],
        );

        // Transfer tokens to vault
        Transfer {
            from: self.accounts.maker_ata_a,
            to: self.accounts.vault,
            authority: self.accounts.maker,
            amount: self.instruction_data.amount,
        }
        .invoke()?;

        Ok(())
    }
}
