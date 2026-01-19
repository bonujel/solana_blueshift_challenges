use pinocchio::{
    account_info::AccountInfo,
    instruction::Signer,
    program_error::ProgramError,
    pubkey::find_program_address,
    seeds, ProgramResult,
};
use pinocchio_system::instructions::Transfer;

use crate::{ID, VAULT_SEED};

/// Withdraw instruction - transfers all lamports from vault PDA back to owner
pub struct Withdraw<'a> {
    /// Owner account (must be signer)
    pub owner: &'a AccountInfo,
    /// Vault PDA account
    pub vault: &'a AccountInfo,
    /// PDA bump seed
    pub bump: u8,
}

impl Withdraw<'_> {
    /// Instruction discriminator
    pub const DISCRIMINATOR: &'static u8 = &1;

    /// Process the withdraw instruction
    pub fn process(&self) -> ProgramResult {
        // Verify owner is a signer
        if !self.owner.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // Verify vault is owned by System Program
        if self.vault.owner() != &pinocchio_system::ID {
            return Err(ProgramError::InvalidAccountOwner);
        }

        // Verify vault has lamports (cannot withdraw from empty vault)
        let lamports = self.vault.lamports();
        if lamports == 0 {
            return Err(ProgramError::InsufficientFunds);
        }

        // Verify vault PDA derivation
        let (expected_vault, _) = find_program_address(
            &[VAULT_SEED, self.owner.key().as_ref()],
            &ID,
        );

        if self.vault.key() != &expected_vault {
            return Err(ProgramError::InvalidSeeds);
        }

        // Prepare PDA signer seeds
        let bump_bytes = [self.bump];
        let signer_seeds = seeds!(VAULT_SEED, self.owner.key().as_ref(), &bump_bytes);
        let signer = Signer::from(&signer_seeds);

        // Transfer all lamports from vault to owner using signed CPI
        Transfer {
            from: self.vault,
            to: self.owner,
            lamports,
        }
        .invoke_signed(&[signer])?;

        Ok(())
    }
}

impl<'a> TryFrom<&'a [AccountInfo]> for Withdraw<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        // Parse accounts
        let [owner, vault, _system_program] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        // Derive PDA and get bump seed
        let (_, bump) = find_program_address(
            &[VAULT_SEED, owner.key().as_ref()],
            &ID,
        );

        Ok(Self {
            owner,
            vault,
            bump,
        })
    }
}
