use core::mem::{size_of, MaybeUninit};
use pinocchio::{
    AccountView,
    Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    ProgramResult,
};
use pinocchio_system::create_account_with_minimum_balance_signed;
use pinocchio_token::instructions::InitializeMint2;

use crate::Config;

// ==================== Accounts ====================

pub struct InitializeAccounts<'a> {
    pub initializer: &'a AccountView,
    pub mint_lp: &'a AccountView,
    pub config: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for InitializeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [initializer, mint_lp, config, _system_program, _token_program] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        Ok(Self {
            initializer,
            mint_lp,
            config,
        })
    }
}

// ==================== Instruction Data ====================

#[repr(C, packed)]
pub struct InitializeInstructionData {
    pub seed: u64,
    pub fee: u16,
    pub mint_x: [u8; 32],
    pub mint_y: [u8; 32],
    pub config_bump: [u8; 1],
    pub lp_bump: [u8; 1],
    pub authority: [u8; 32],
}

impl TryFrom<&[u8]> for InitializeInstructionData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        const INITIALIZE_DATA_LEN_WITH_AUTHORITY: usize = size_of::<InitializeInstructionData>();
        const INITIALIZE_DATA_LEN: usize =
            INITIALIZE_DATA_LEN_WITH_AUTHORITY - size_of::<[u8; 32]>();

        match data.len() {
            INITIALIZE_DATA_LEN_WITH_AUTHORITY => {
                // Full data with authority
                Ok(unsafe { (data.as_ptr() as *const Self).read_unaligned() })
            }
            INITIALIZE_DATA_LEN => {
                // Without authority - create immutable pool with zero authority
                let mut raw: MaybeUninit<[u8; INITIALIZE_DATA_LEN_WITH_AUTHORITY]> =
                    MaybeUninit::uninit();
                let raw_ptr = raw.as_mut_ptr() as *mut u8;
                unsafe {
                    // Copy the provided data
                    core::ptr::copy_nonoverlapping(data.as_ptr(), raw_ptr, INITIALIZE_DATA_LEN);
                    // Add zero authority to the end of the buffer
                    core::ptr::write_bytes(raw_ptr.add(INITIALIZE_DATA_LEN), 0, 32);
                    // Transmute to the struct
                    Ok((raw.as_ptr() as *const Self).read_unaligned())
                }
            }
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

// ==================== Initialize Instruction ====================

pub struct Initialize<'a> {
    pub accounts: InitializeAccounts<'a>,
    pub instruction_data: InitializeInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountView])> for Initialize<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountView])) -> Result<Self, Self::Error> {
        let accounts = InitializeAccounts::try_from(accounts)?;
        let instruction_data = InitializeInstructionData::try_from(data)?;
        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> Initialize<'a> {
    pub const DISCRIMINATOR: &'a u8 = &0;

    pub fn process(&mut self) -> ProgramResult {
        // 1. Create Config account
        let seed_binding = self.instruction_data.seed.to_le_bytes();
        let config_seeds = [
            Seed::from(b"config"),
            Seed::from(&seed_binding),
            Seed::from(&self.instruction_data.mint_x),
            Seed::from(&self.instruction_data.mint_y),
            Seed::from(&self.instruction_data.config_bump),
        ];
        let config_signer = Signer::from(&config_seeds);

        create_account_with_minimum_balance_signed(
            self.accounts.config,
            Config::LEN,
            &crate::ID,
            self.accounts.initializer,
            None,  // rent_sysvar - use syscall
            &[config_signer],
        )?;

        // 2. Fill Config data
        let config = unsafe { Config::load_mut_unchecked(self.accounts.config)? };
        config.set_inner(
            self.instruction_data.seed,
            self.instruction_data.authority,
            self.instruction_data.mint_x,
            self.instruction_data.mint_y,
            self.instruction_data.fee,
            self.instruction_data.config_bump,
        )?;

        // 3. Create mint_lp account
        let mint_lp_seeds = [
            Seed::from(b"mint_lp"),
            Seed::from(self.accounts.config.address().as_ref()),
            Seed::from(&self.instruction_data.lp_bump),
        ];
        let mint_lp_signer = Signer::from(&mint_lp_seeds);

        // Mint account size is 82 bytes
        const MINT_SIZE: usize = 82;

        create_account_with_minimum_balance_signed(
            self.accounts.mint_lp,
            MINT_SIZE,
            &pinocchio_token::ID,
            self.accounts.initializer,
            None,  // rent_sysvar - use syscall
            &[mint_lp_signer],
        )?;

        // 4. Initialize mint_lp with config as mint_authority
        // LP token has 6 decimals (standard for LP tokens)
        InitializeMint2 {
            mint: self.accounts.mint_lp,
            decimals: 6,
            mint_authority: self.accounts.config.address(),
            freeze_authority: None,
        }
        .invoke()?;

        Ok(())
    }
}
