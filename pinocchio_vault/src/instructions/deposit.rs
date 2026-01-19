use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::find_program_address,
    ProgramResult,
};
use pinocchio_system::instructions::Transfer;

use crate::{ID, VAULT_SEED};

/// Deposit instruction - transfers lamports from owner to vault PDA
pub struct Deposit<'a> {
    /// Owner account (must be signer)
    pub owner: &'a AccountInfo,
    /// Vault PDA account
    pub vault: &'a AccountInfo,
    /// Amount to deposit
    pub amount: u64,
}

impl Deposit<'_> {
    /// Instruction discriminator
    pub const DISCRIMINATOR: &'static u8 = &0;

    /// Process the deposit instruction
    pub fn process(&self) -> ProgramResult {
        // Verify owner is a signer
        if !self.owner.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // Verify vault is owned by System Program (uninitialized account)
        if self.vault.owner() != &pinocchio_system::ID {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // Verify vault has zero lamports (prevents duplicate deposits)
        if self.vault.lamports() != 0 {
            return Err(ProgramError::AccountAlreadyInitialized);
        }

        // Verify vault PDA derivation
        let (expected_vault, _bump) = find_program_address(
            &[VAULT_SEED, self.owner.key().as_ref()],
            &ID,
        );

        if self.vault.key() != &expected_vault {
            return Err(ProgramError::InvalidSeeds);
        }

        // Verify amount is greater than zero
        if self.amount == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

        // Transfer lamports from owner to vault via CPI
        Transfer {
            from: self.owner,
            to: self.vault,
            lamports: self.amount,
        }
        .invoke()?;

        Ok(())
    }
}

impl<'a> TryFrom<(&[u8], &'a [AccountInfo])> for Deposit<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&[u8], &'a [AccountInfo])) -> Result<Self, Self::Error> {
        // Parse accounts
        let [owner, vault, _system_program] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // Parse instruction data (8 bytes for u64 amount in little-endian)
        if data.len() < 8 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let amount = u64::from_le_bytes(
            data[..8]
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?,
        );

        Ok(Self {
            owner,
            vault,
            amount,
        })
    }
}
