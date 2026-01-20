use constant_product_curve::ConstantProduct;
use pinocchio::{
    AccountView,
    Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_token::{
    instructions::{MintTo, Transfer},
    state::{Mint, TokenAccount},
};

use crate::{AmmState, Config};

// ==================== Accounts ====================

pub struct DepositAccounts<'a> {
    pub user: &'a AccountView,
    pub mint_lp: &'a AccountView,
    pub vault_x: &'a AccountView,
    pub vault_y: &'a AccountView,
    pub user_x_ata: &'a AccountView,
    pub user_y_ata: &'a AccountView,
    pub user_lp_ata: &'a AccountView,
    pub config: &'a AccountView,
    pub token_program: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for DepositAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [user, mint_lp, vault_x, vault_y, user_x_ata, user_y_ata, user_lp_ata, config, token_program] =
            accounts
        else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };

        Ok(Self {
            user,
            mint_lp,
            vault_x,
            vault_y,
            user_x_ata,
            user_y_ata,
            user_lp_ata,
            config,
            token_program,
        })
    }
}

// ==================== Instruction Data ====================

#[repr(C, packed)]
pub struct DepositInstructionData {
    pub amount: u64,
    pub max_x: u64,
    pub max_y: u64,
    pub expiration: i64,
}

impl TryFrom<&[u8]> for DepositInstructionData {
    type Error = ProgramError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() != core::mem::size_of::<Self>() {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(unsafe { (data.as_ptr() as *const Self).read_unaligned() })
    }
}

// ==================== Deposit Instruction ====================

pub struct Deposit<'a> {
    pub accounts: DepositAccounts<'a>,
    pub instruction_data: DepositInstructionData,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountView])> for Deposit<'a> {
    type Error = ProgramError;

    fn try_from((data, accounts): (&'a [u8], &'a [AccountView])) -> Result<Self, Self::Error> {
        let accounts = DepositAccounts::try_from(accounts)?;
        let instruction_data = DepositInstructionData::try_from(data)?;

        // Validate amounts are greater than zero
        if instruction_data.amount == 0
            || instruction_data.max_x == 0
            || instruction_data.max_y == 0
        {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(Self {
            accounts,
            instruction_data,
        })
    }
}

impl<'a> Deposit<'a> {
    pub const DISCRIMINATOR: &'a u8 = &1;

    pub fn process(&mut self) -> ProgramResult {
        // 1. Check expiration using Clock sysvar
        let clock = Clock::get()?;
        if clock.unix_timestamp >= self.instruction_data.expiration {
            return Err(ProgramError::Custom(1)); // Order expired
        }

        // 2. Load and validate config
        let config = Config::load(self.accounts.config)?;

        // Verify pool state allows deposits
        if config.state() != AmmState::Initialized as u8 {
            return Err(ProgramError::InvalidAccountData);
        }

        // 3. Verify vault_x is valid ATA (only on-chain, syscall not available off-chain)
        // ATA seeds: [wallet, token_program_id, mint]
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
        let mint_lp = unsafe { Mint::from_account_view_unchecked(self.accounts.mint_lp)? };
        let vault_x_account =
            unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault_x)? };
        let vault_y_account =
            unsafe { TokenAccount::from_account_view_unchecked(self.accounts.vault_y)? };

        // 6. Calculate deposit amounts
        let (x, y) = match mint_lp.supply() == 0
            && vault_x_account.amount() == 0
            && vault_y_account.amount() == 0
        {
            // First deposit: use user's max amounts directly
            true => (self.instruction_data.max_x, self.instruction_data.max_y),
            // Subsequent deposits: calculate required amounts based on desired LP
            false => {
                let amounts = ConstantProduct::xy_deposit_amounts_from_l(
                    vault_x_account.amount(),
                    vault_y_account.amount(),
                    mint_lp.supply(),
                    self.instruction_data.amount,
                    6, // LP token decimals
                )
                .map_err(|_| ProgramError::InvalidArgument)?;
                (amounts.x, amounts.y)
            }
        };

        // 7. Check for slippage
        if !(x <= self.instruction_data.max_x && y <= self.instruction_data.max_y) {
            return Err(ProgramError::InvalidArgument);
        }

        // 8. Transfer token X from user to vault
        Transfer {
            from: self.accounts.user_x_ata,
            to: self.accounts.vault_x,
            authority: self.accounts.user,
            amount: x,
        }
        .invoke()?;

        // 9. Transfer token Y from user to vault
        Transfer {
            from: self.accounts.user_y_ata,
            to: self.accounts.vault_y,
            authority: self.accounts.user,
            amount: y,
        }
        .invoke()?;

        // 10. Mint LP tokens to user
        // Config PDA is the mint authority, so we need to sign with config seeds
        let seed_binding = config.seed().to_le_bytes();
        let bump_binding = config.config_bump();
        let config_seeds = [
            Seed::from(b"config"),
            Seed::from(&seed_binding),
            Seed::from(config.mint_x()),
            Seed::from(config.mint_y()),
            Seed::from(&bump_binding),
        ];
        let config_signer = Signer::from(&config_seeds);

        MintTo {
            mint: self.accounts.mint_lp,
            account: self.accounts.user_lp_ata,
            mint_authority: self.accounts.config,
            amount: self.instruction_data.amount,
        }
        .invoke_signed(&[config_signer])?;

        Ok(())
    }
}
