use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Invalid amount: amount must be greater than zero")]
    InvalidAmount,
    #[msg("Invalid maker: maker does not match escrow maker")]
    InvalidMaker,
    #[msg("Invalid mint A: mint_a does not match escrow mint_a")]
    InvalidMintA,
    #[msg("Invalid mint B: mint_b does not match escrow mint_b")]
    InvalidMintB,
}
