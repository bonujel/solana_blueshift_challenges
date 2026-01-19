use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

/// Escrow account state - stores all transaction terms
/// Memory layout: #[repr(C)] ensures predictable field ordering
#[repr(C)]
pub struct Escrow {
    /// Random identifier allowing multiple escrows per token pair
    pub seed: u64,
    /// Creator's wallet address
    pub maker: Pubkey,
    /// Deposited token's mint (Token A)
    pub mint_a: Pubkey,
    /// Requested token's mint (Token B)
    pub mint_b: Pubkey,
    /// Desired amount of Token B
    pub receive: u64,
    /// PDA derivation bump seed (stored as array for easy use in signer seeds)
    pub bump: [u8; 1],
}

impl Escrow {
    /// Size of the Escrow account in bytes
    /// 8 (seed) + 32 (maker) + 32 (mint_a) + 32 (mint_b) + 8 (receive) + 1 (bump) = 113
    pub const LEN: usize = 8 + 32 + 32 + 32 + 8 + 1;

    /// Safely load Escrow from account data
    #[inline(always)]
    pub fn from_account_info(account: &AccountInfo) -> Result<&Self, ProgramError> {
        // Verify account data length
        if account.data_len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        // Safety: We verified the data length above
        // The account data is properly aligned for our struct
        unsafe {
            let ptr = account.borrow_data_unchecked().as_ptr() as *const Self;
            Ok(&*ptr)
        }
    }

    /// Safely load mutable Escrow from account data
    #[inline(always)]
    pub fn from_account_info_mut(account: &AccountInfo) -> Result<&mut Self, ProgramError> {
        // Verify account data length
        if account.data_len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        // Safety: We verified the data length above
        unsafe {
            let ptr = account.borrow_mut_data_unchecked().as_mut_ptr() as *mut Self;
            Ok(&mut *ptr)
        }
    }

    /// Initialize escrow with all fields
    #[inline(always)]
    pub fn init(
        &mut self,
        seed: u64,
        maker: Pubkey,
        mint_a: Pubkey,
        mint_b: Pubkey,
        receive: u64,
        bump: u8,
    ) {
        self.seed = seed;
        self.maker = maker;
        self.mint_a = mint_a;
        self.mint_b = mint_b;
        self.receive = receive;
        self.bump = [bump];
    }

    /// Set inner values (alias for init, matches reference code)
    #[inline(always)]
    pub fn set_inner(
        &mut self,
        seed: u64,
        maker: Pubkey,
        mint_a: Pubkey,
        mint_b: Pubkey,
        receive: u64,
        bump: [u8; 1],
    ) {
        self.seed = seed;
        self.maker = maker;
        self.mint_a = mint_a;
        self.mint_b = mint_b;
        self.receive = receive;
        self.bump = bump;
    }

    /// Load escrow from raw data slice
    #[inline(always)]
    pub fn load(data: &[u8]) -> Result<&Self, ProgramError> {
        if data.len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        unsafe {
            let ptr = data.as_ptr() as *const Self;
            Ok(&*ptr)
        }
    }

    /// Load mutable escrow from raw data slice
    #[inline(always)]
    pub fn load_mut(data: &mut [u8]) -> Result<&mut Self, ProgramError> {
        if data.len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        unsafe {
            let ptr = data.as_mut_ptr() as *mut Self;
            Ok(&mut *ptr)
        }
    }
}
