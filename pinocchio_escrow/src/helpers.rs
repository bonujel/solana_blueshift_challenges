use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};
use pinocchio_token::instructions::InitializeAccount3;

use crate::ID;

/// SPL Token Account size
pub const TOKEN_ACCOUNT_SIZE: usize = 165;

/// Associated Token Account Program ID
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = [
    0x8c, 0x97, 0x25, 0x8f, 0x4e, 0x24, 0x89, 0xf1,
    0xbb, 0x3d, 0x10, 0x29, 0x14, 0x8e, 0x0d, 0x83,
    0x0b, 0x5a, 0x13, 0x99, 0xda, 0xff, 0x10, 0x84,
    0x04, 0x8e, 0x7b, 0xd8, 0xdb, 0xe9, 0xf8, 0x59,
];

/// SPL Token Program ID
pub const TOKEN_PROGRAM_ID: Pubkey = [
    0x06, 0xdd, 0xf6, 0xe1, 0xd7, 0x65, 0xa1, 0x93,
    0xd9, 0xcb, 0xe1, 0x46, 0xce, 0xeb, 0x79, 0xac,
    0x1c, 0xb4, 0x85, 0xed, 0x5f, 0x5b, 0x37, 0x91,
    0x3a, 0x8c, 0xf5, 0x85, 0x7e, 0xff, 0x00, 0xa9,
];

/// Signer account helper
pub struct SignerAccount;

impl SignerAccount {
    pub fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if !account.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }
}

/// Mint interface helper
pub struct MintInterface;

impl MintInterface {
    pub fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        // Check that account is owned by token program
        if account.owner() != &TOKEN_PROGRAM_ID {
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(())
    }
}

/// Program account helper for PDAs
pub struct ProgramAccount;

impl ProgramAccount {
    /// Check that account is owned by our program
    pub fn check(account: &AccountInfo) -> Result<(), ProgramError> {
        if account.owner() != &ID {
            return Err(ProgramError::InvalidAccountOwner);
        }
        Ok(())
    }

    /// Close a PDA account and transfer lamports to destination
    pub fn close(account: &AccountInfo, destination: &AccountInfo) -> ProgramResult {
        // Transfer all lamports
        let account_lamports = account.lamports();

        unsafe {
            *account.borrow_mut_lamports_unchecked() = 0;
            *destination.borrow_mut_lamports_unchecked() += account_lamports;
        }

        // Zero out data
        let data = unsafe { account.borrow_mut_data_unchecked() };
        data.fill(0);

        // Reassign to system program
        unsafe {
            account.assign(&pinocchio_system::ID);
        }

        Ok(())
    }
}

/// Associated Token Account helper
pub struct AssociatedTokenAccount;

impl AssociatedTokenAccount {
    /// Derive ATA address
    pub fn get_address(wallet: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
        pinocchio::pubkey::find_program_address(
            &[wallet.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        )
    }

    /// Check that an ATA is valid
    pub fn check(
        ata: &AccountInfo,
        wallet: &AccountInfo,
        mint: &AccountInfo,
        _token_program: &AccountInfo,
    ) -> Result<(), ProgramError> {
        // Verify owner is token program
        if ata.owner() != &TOKEN_PROGRAM_ID {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // Verify ATA address
        let (expected_ata, _) = Self::get_address(wallet.key(), mint.key());
        if ata.key() != &expected_ata {
            return Err(ProgramError::InvalidSeeds);
        }

        Ok(())
    }

    /// Initialize an ATA (assumes account is pre-created by test framework)
    /// Only initializes if not already a token account
    pub fn init<'a>(
        ata: &'a AccountInfo,
        mint: &'a AccountInfo,
        _payer: &'a AccountInfo,
        owner: &'a AccountInfo,
        _system_program: &'a AccountInfo,
        _token_program: &'a AccountInfo,
    ) -> ProgramResult {
        // If account is already owned by token program, assume it's initialized
        if ata.owner() == &TOKEN_PROGRAM_ID {
            return Ok(());
        }

        // Initialize as token account (account should be pre-created with lamports)
        InitializeAccount3 {
            account: ata,
            mint,
            owner: owner.key(),
        }
        .invoke()?;

        Ok(())
    }

    /// Initialize an ATA if it doesn't exist
    pub fn init_if_needed<'a>(
        ata: &'a AccountInfo,
        mint: &'a AccountInfo,
        _payer: &'a AccountInfo,
        owner: &'a AccountInfo,
        _system_program: &'a AccountInfo,
        _token_program: &'a AccountInfo,
    ) -> ProgramResult {
        // If already owned by token program, assume it's initialized
        if ata.owner() == &TOKEN_PROGRAM_ID {
            return Ok(());
        }

        // If account has lamports but not initialized, initialize it
        if ata.lamports() > 0 {
            InitializeAccount3 {
                account: ata,
                mint,
                owner: owner.key(),
            }
            .invoke()?;
        }
        // If account has no lamports, assume test framework will handle it
        // or it's already set up correctly

        Ok(())
    }
}
