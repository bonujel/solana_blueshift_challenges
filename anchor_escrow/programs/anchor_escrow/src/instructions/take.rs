use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{close_account, transfer_checked, CloseAccount, Mint, Token, TokenAccount, TransferChecked},
};

use crate::state::Escrow;

#[derive(Accounts)]
pub struct Take<'info> {
    /// The taker who accepts the exchange terms
    #[account(mut)]
    pub taker: Signer<'info>,

    /// The original maker who created the escrow
    #[account(mut)]
    pub maker: SystemAccount<'info>,

    /// Escrow account storing exchange terms (will be closed)
    #[account(
        mut,
        close = maker,
        has_one = maker,
        has_one = mint_a,
        has_one = mint_b,
        seeds = [b"escrow", maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Box<Account<'info, Escrow>>,

    /// Token A mint
    pub mint_a: Box<Account<'info, Mint>>,

    /// Token B mint
    pub mint_b: Box<Account<'info, Mint>>,

    /// Vault holding Token A (owned by escrow)
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,

    /// Taker's associated token account for Token A (receives Token A)
    #[account(
        init_if_needed,
        payer = taker,
        associated_token::mint = mint_a,
        associated_token::authority = taker,
    )]
    pub taker_ata_a: Box<Account<'info, TokenAccount>>,

    /// Taker's associated token account for Token B (source of Token B)
    #[account(
        mut,
        associated_token::mint = mint_b,
        associated_token::authority = taker,
    )]
    pub taker_ata_b: Box<Account<'info, TokenAccount>>,

    /// Maker's associated token account for Token B (receives Token B)
    #[account(
        init_if_needed,
        payer = taker,
        associated_token::mint = mint_b,
        associated_token::authority = maker,
    )]
    pub maker_ata_b: Box<Account<'info, TokenAccount>>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> Take<'info> {
    /// Transfer Token B from taker to maker
    pub fn transfer_to_maker(&mut self) -> Result<()> {
        let cpi_accounts = TransferChecked {
            from: self.taker_ata_b.to_account_info(),
            mint: self.mint_b.to_account_info(),
            to: self.maker_ata_b.to_account_info(),
            authority: self.taker.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        transfer_checked(cpi_ctx, self.escrow.receive, self.mint_b.decimals)
    }

    /// Withdraw Token A from vault to taker, then close the vault
    pub fn withdraw_and_close_vault(&mut self) -> Result<()> {
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"escrow",
            self.maker.key.as_ref(),
            &self.escrow.seed.to_le_bytes(),
            &[self.escrow.bump],
        ]];

        // Transfer all Token A from vault to taker
        let cpi_accounts = TransferChecked {
            from: self.vault.to_account_info(),
            mint: self.mint_a.to_account_info(),
            to: self.taker_ata_a.to_account_info(),
            authority: self.escrow.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        transfer_checked(cpi_ctx, self.vault.amount, self.mint_a.decimals)?;

        // Close the vault account and return rent to maker
        let cpi_accounts = CloseAccount {
            account: self.vault.to_account_info(),
            destination: self.maker.to_account_info(),
            authority: self.escrow.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        close_account(cpi_ctx)
    }
}

/// Handler for the take instruction
pub fn handler(ctx: Context<Take>) -> Result<()> {
    // First, transfer Token B from taker to maker
    ctx.accounts.transfer_to_maker()?;

    // Then, withdraw Token A from vault to taker and close vault
    ctx.accounts.withdraw_and_close_vault()?;

    Ok(())
}
