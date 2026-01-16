use anchor_lang::prelude::*;

/// Escrow account that stores all the exchange terms
#[account(discriminator = 1)]
#[derive(InitSpace)]
pub struct Escrow {
    /// Seed used for PDA derivation
    pub seed: u64,
    /// The maker's wallet address (creator of the escrow)
    pub maker: Pubkey,
    /// Token A mint address (the token maker deposits)
    pub mint_a: Pubkey,
    /// Token B mint address (the token maker wants to receive)
    pub mint_b: Pubkey,
    /// Amount of Token B the maker wants to receive
    pub receive: u64,
    /// Bump seed for PDA derivation (cached for efficiency)
    pub bump: u8,
}
