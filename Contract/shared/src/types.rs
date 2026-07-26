use soroban_sdk::{contracttype, Address, BytesN};

/// Vault status enum representing the current state of a vault
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum VaultStatus {
    Active = 0,
    Locked = 1,
    Unlocked = 2,
    Closed = 3,
}

/// Asset identifier for supported tokens in the protocol
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct Asset {
    pub code: BytesN<32>,
    pub issuer: Option<Address>,
}

/// Vault metadata containing lock period and other configuration
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct VaultMetadata {
    pub owner: Address,
    pub asset: Asset,
    pub lock_period: u64, // in seconds
    pub created_at: u64,
    pub unlock_time: u64,
    pub status: VaultStatus,
}

/// Streak state for tracking user's consecutive activity
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct UserStreak {
    pub current_streak: u32,
    pub longest_streak: u32,
    pub last_activity_period: u64, // UTC day timestamp (seconds since epoch / 86400 * 86400)
    pub available_freezes: u32,     // Number of streak freezes available to use
}

/// Milestone definition for reward eligibility
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct Milestone {
    pub streak_threshold: u32,
    pub reward_amount: i128,
    pub reward_type: u32, // 0 = one-time streak milestone
}

/// User reward entitlement tracking
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct UserReward {
    pub milestone_id: BytesN<32>,
    pub amount: i128,
    pub claimed: bool,
    pub claimed_at: Option<u64>,
}

/// Rewards pool state
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct RewardsPool {
    pub total_funded: i128,
    pub available_liquidity: i128,
    pub reward_asset: BytesN<32>,
    pub initialized: bool,
    pub admin: Address,
}

/// Lending pool state with interest configuration and liquidity accounting.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct LendingPool {
    pub pool_id: BytesN<32>,
    pub asset: Asset,
    pub interest_rate_bps: u32,
    pub total_deposits: i128,
    pub total_shares: i128,
    pub initialized: bool,
}

/// A lender position inside a pool.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct PoolPosition {
    pub user: Address,
    pub shares: i128,
    pub last_accrued_at: u64,
}

/// Loan status for borrowing flows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum LoanStatus {
    Active = 0,
    Repaid = 1,
    Liquidated = 2,
}

/// Collateralized loan state.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct Loan {
    pub loan_id: BytesN<32>,
    pub borrower: Address,
    pub collateral_asset: Asset,
    pub collateral_amount: i128,
    pub borrow_asset: Asset,
    pub borrow_amount: i128,
    pub outstanding_amount: i128,
    pub interest_rate_bps: u32,
    pub created_at: u64,
    pub status: LoanStatus,
}

/// Amount wrapper for safe arithmetic operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Amount {
    pub value: i128,
}

impl Amount {
    pub const fn new(value: i128) -> Self {
        Self { value }
    }

    pub fn checked_add(self, other: Amount) -> Option<Amount> {
        self.value.checked_add(other.value).map(Amount::new)
    }

    pub fn checked_sub(self, other: Amount) -> Option<Amount> {
        self.value.checked_sub(other.value).map(Amount::new)
    }
}