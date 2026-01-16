use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer_checked, Mint, Token, TokenAccount, TransferChecked},
};

use crate::state::Escrow;

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Make<'info> {
    /// The maker who sets exchange terms and deposits Token A
    #[account(mut)]
    pub maker: Signer<'info>,

    /// Escrow account that stores all exchange conditions
    #[account(
        init,
        payer = maker,
        space = 8 + Escrow::INIT_SPACE,
        seeds = [b"escrow", maker.key().as_ref(), seed.to_le_bytes().as_ref()],
        bump,
    )]
    pub escrow: Account<'info, Escrow>,

    /// Token A mint (the token the maker will deposit)
    pub mint_a: Account<'info, Mint>,

    /// Token B mint (the token the maker wants to receive)
    pub mint_b: Account<'info, Mint>,

    /// Maker's associated token account for Token A (source of deposit)
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
    )]
    pub maker_ata_a: Account<'info, TokenAccount>,

    /// Vault account owned by escrow to hold Token A
    #[account(
        init,
        payer = maker,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
    )]
    pub vault: Account<'info, TokenAccount>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> Make<'info> {
    /// Initialize the escrow account with exchange terms
    pub fn init_escrow(&mut self, seed: u64, receive: u64, bumps: &MakeBumps) -> Result<()> {
        self.escrow.set_inner(Escrow {
            seed,
            maker: self.maker.key(),
            mint_a: self.mint_a.key(),
            mint_b: self.mint_b.key(),
            receive,
            bump: bumps.escrow,
        });
        Ok(())
    }

    /// Transfer Token A from maker to vault
    pub fn deposit(&mut self, amount: u64) -> Result<()> {
        let cpi_accounts = TransferChecked {
            from: self.maker_ata_a.to_account_info(),
            mint: self.mint_a.to_account_info(),
            to: self.vault.to_account_info(),
            authority: self.maker.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        transfer_checked(cpi_ctx, amount, self.mint_a.decimals)
    }
}

/// Handler for the make instruction
pub fn handler(ctx: Context<Make>, seed: u64, receive: u64, amount: u64) -> Result<()> {
    // Validate that receive amount is greater than zero
    require_gt!(receive, 0, crate::errors::EscrowError::InvalidAmount);
    // Validate that deposit amount is greater than zero
    require_gt!(amount, 0, crate::errors::EscrowError::InvalidAmount);

    // Initialize escrow with exchange terms
    ctx.accounts.init_escrow(seed, receive, &ctx.bumps)?;

    // Deposit Token A into vault
    ctx.accounts.deposit(amount)?;

    Ok(())
}
