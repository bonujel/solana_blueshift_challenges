#![no_std]

use pinocchio::{
    account_info::AccountInfo, entrypoint, nostd_panic_handler,
    program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

entrypoint!(process_instruction);
nostd_panic_handler!();

pub mod helpers;
pub mod instructions;
pub mod state;

pub use instructions::*;

/// Program ID specified by the challenge
pub const ID: Pubkey = [
    0x0f, 0x1e, 0x6b, 0x14, 0x21, 0xc0, 0x4a, 0x07,
    0x04, 0x31, 0x26, 0x5c, 0x19, 0xc5, 0xbb, 0xee,
    0x19, 0x92, 0xba, 0xe8, 0xaf, 0xd1, 0xcd, 0x07,
    0x8e, 0xf8, 0xaf, 0x70, 0x47, 0xdc, 0x11, 0xf7,
];

/// Escrow PDA seed prefix
pub const ESCROW_SEED: &[u8] = b"escrow";

/// Process program instruction
///
/// Instruction discriminators:
/// - 0: Make - Create an escrow offer
/// - 1: Take - Accept an escrow offer
/// - 2: Refund - Cancel an escrow offer
fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data.split_first() {
        Some((Make::DISCRIMINATOR, data)) => {
            Make::try_from((data, accounts))?.process()
        }
        Some((Take::DISCRIMINATOR, _)) => {
            Take::try_from(accounts)?.process()
        }
        Some((Refund::DISCRIMINATOR, _)) => {
            Refund::try_from(accounts)?.process()
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
