use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("22222222222222222222222222222222222222222222");

#[program]
pub mod blueshift_anchor_vault {
    use super::*;

    /// Deposit lamports into the vault
    ///
    /// Requirements:
    /// 1. Vault must be empty (no duplicate deposits)
    /// 2. Amount must exceed rent-exempt minimum for SystemAccount
    /// 3. Transfer via CPI from signer to vault
    pub fn deposit(ctx: Context<VaultAction>, amount: u64) -> Result<()> {
        // Verify vault is empty (prevent duplicate deposits)
        require_eq!(
            ctx.accounts.vault.lamports(),
            0,
            VaultError::VaultAlreadyExists
        );

        // Check amount exceeds rent minimum for a 0-byte account
        let rent_minimum = Rent::get()?.minimum_balance(0);
        require_gt!(amount, rent_minimum, VaultError::InvalidAmount);

        // Transfer lamports from signer to vault via CPI
        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.signer.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        );

        transfer(cpi_context, amount)?;

        msg!("Deposited {} lamports to vault", amount);
        Ok(())
    }

    /// Withdraw all lamports from the vault
    ///
    /// Requirements:
    /// 1. Vault must contain lamports
    /// 2. Use PDA signing to authorize transfer
    /// 3. Return all lamports to the original signer
    pub fn withdraw(ctx: Context<VaultAction>) -> Result<()> {
        let vault_balance = ctx.accounts.vault.lamports();

        // Verify vault has lamports to withdraw
        require_neq!(vault_balance, 0, VaultError::InvalidAmount);

        // Create PDA signer seeds for CPI
        let signer_key = ctx.accounts.signer.key();
        let bump = ctx.bumps.vault;
        let signer_seeds: &[&[&[u8]]] = &[&[b"vault", signer_key.as_ref(), &[bump]]];

        // Transfer all lamports from vault back to signer via CPI with PDA signing
        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.signer.to_account_info(),
            },
            signer_seeds,
        );

        transfer(cpi_context, vault_balance)?;

        msg!("Withdrew {} lamports from vault", vault_balance);
        Ok(())
    }
}

// ============================================================
// Account Structures
// ============================================================

#[derive(Accounts)]
pub struct VaultAction<'info> {
    /// The signer who owns this vault
    /// Must be mutable because lamports will be transferred
    #[account(mut)]
    pub signer: Signer<'info>,

    /// The vault PDA derived from ["vault", signer.key()]
    /// Must be mutable because lamports will be updated
    #[account(
        mut,
        seeds = [b"vault", signer.key().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    /// System program for CPI transfers
    pub system_program: Program<'info, System>,
}

// ============================================================
// Error Definitions
// ============================================================

#[error_code]
pub enum VaultError {
    #[msg("Vault already exists")]
    VaultAlreadyExists,
    #[msg("Invalid amount")]
    InvalidAmount,
}
