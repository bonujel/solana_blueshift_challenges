use constant_product_curve::{ConstantProduct, LiquidityPair};
use pinocchio::{
    AccountView,
    Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_token::{
    instructions::Transfer,
    state::TokenAccount,
};

use crate::{AmmState, Config};

// ==================== Accounts ====================

pub struct SwapAccounts<'a> {
    pub user: &'a AccountView,
    pub user_x_ata: &'a AccountView,
    pub user_y_ata: &'a AccountView,
    pub vault_x: &'a AccountView,
    pub vault_y: &'a AccountView,
    pub config: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for SwapAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [user, user_x_ata, user_y_ata, vault_x, vault_y, config, token_program] = accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        Ok(Self {
            user,
            user_x_ata,
            user_y_ata,
            vault_x,
            vault_y,
            config,
            token_program,
        })
    }
}

// ==================== Instruction Data ====================

#[repr(C, packed)]
pub struct SwapInstructionData {
    pub is_x: u8, // bool as u8 for packed struct
    pub amount: u64,
    pub min: u64,
    pub expiration: i64,
}

impl TryFrom<&[u8]> for SwapInstructionData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<Self>() {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(unsafe { (data.as_ptr() as *const Self).read_unaligned() })
    }
}

impl SwapInstructionData {
    #[inline]
    pub fn is_x(&self) -> bool {
        self.is_x != 0
    }
}

// ==================== Swap Instruction ====================

pub struct Swap<'a> {
    pub accounts: SwapAccounts<'a>,
    pub instruction_data: SwapInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountView])> for Swap<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountView])) -> Result<Self, Self::Error> {
        let accounts = SwapAccounts::try_from(accounts)?;
        let instruction_data = SwapInstructionData::try_from(data)?;

        // Validate amounts are greater than zero
        if instruction_data.amount == 0 || instruction_data.min == 0 {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> Swap<'a> {
    pub const DISCRIMINATOR: &'a u8 = &3;

    pub fn process(&mut self) -> ProgramResult {
        // 1. Check expiration using Clock sysvar
        let clock = Clock::get()?;
        if clock.unix_timestamp >= self.instruction_data.expiration {
            return Err(ProgramError::Custom(1)); // Order expired
        }

        // 2. Load and validate config
        let config = Config::load(self.accounts.config)?;

        // Verify pool state allows swaps (must be initialized)
        if config.state() != AmmState::Initialized as u8 {
            return Err(ProgramError::InvalidAccountData);
        }

        // 3. Verify vault_x is valid ATA (only on-chain)
        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        {
            let (vault_x_addr, _) = Address::find_program_address(
                &[
                    self.accounts.config.address().as_ref(),
                    self.accounts.token_program.address().as_ref(),
                    config.mint_x(),
                ],
                &pinocchio_associated_token_account::ID,
            );
            if vault_x_addr.ne(self.accounts.vault_x.address()) {
                return Err(ProgramError::InvalidAccountData);
            }
        }

        // 4. Verify vault_y is valid ATA
        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        {
            let (vault_y_addr, _) = Address::find_program_address(
                &[
                    self.accounts.config.address().as_ref(),
                    self.accounts.token_program.address().as_ref(),
                    config.mint_y(),
                ],
                &pinocchio_associated_token_account::ID,
            );
            if vault_y_addr.ne(self.accounts.vault_y.address()) {
                return Err(ProgramError::InvalidAccountData);
            }
        }

        // 5. Deserialize the token accounts
        let vault_x_account =
            unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault_x)? };
        let vault_y_account =
            unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault_y)? };

        // 6. Calculate swap using constant product curve
        let mut curve = ConstantProduct::init(
            vault_x_account.amount(),
            vault_y_account.amount(),
            vault_x_account.amount(), // l parameter (not used for swap)
            config.fee(),
            None,
        )
        .map_err(|_| ProgramError::Custom(1))?;

        let pair = match self.instruction_data.is_x() {
            true => LiquidityPair::X,
            false => LiquidityPair::Y,
        };

        let swap_result = curve
            .swap(pair, self.instruction_data.amount, self.instruction_data.min)
            .map_err(|_| ProgramError::Custom(1))?;

        // 7. Validate swap result
        if swap_result.deposit == 0 || swap_result.withdraw == 0 {
            return Err(ProgramError::InvalidArgument);
        }

        // 8. Prepare config PDA signer for vault transfers
        let seed_binding = config.seed().to_le_bytes();
        let bump_binding = config.config_bump();
        let config_seeds = [
            Seed::from(b"config"),
            Seed::from(&seed_binding),
            Seed::from(config.mint_x()),
            Seed::from(config.mint_y()),
            Seed::from(&bump_binding),
        ];

        // 9. Execute transfers based on swap direction
        if self.instruction_data.is_x() {
            // User sends X, receives Y
            // Transfer X from user to vault_x (user signs)
            Transfer {
                from: self.accounts.user_x_ata,
                to: self.accounts.vault_x,
                authority: self.accounts.user,
                amount: swap_result.deposit,
            }
            .invoke()?;

            // Transfer Y from vault_y to user (config PDA signs)
            let config_signer = Signer::from(&config_seeds);
            Transfer {
                from: self.accounts.vault_y,
                to: self.accounts.user_y_ata,
                authority: self.accounts.config,
                amount: swap_result.withdraw,
            }
            .invoke_signed(&[config_signer])?;
        } else {
            // User sends Y, receives X
            // Transfer Y from user to vault_y (user signs)
            Transfer {
                from: self.accounts.user_y_ata,
                to: self.accounts.vault_y,
                authority: self.accounts.user,
                amount: swap_result.deposit,
            }
            .invoke()?;

            // Transfer X from vault_x to user (config PDA signs)
            let config_signer = Signer::from(&config_seeds);
            Transfer {
                from: self.accounts.vault_x,
                to: self.accounts.user_x_ata,
                authority: self.accounts.config,
                amount: swap_result.withdraw,
            }
            .invoke_signed(&[config_signer])?;
        }

        Ok(())
    }
}
