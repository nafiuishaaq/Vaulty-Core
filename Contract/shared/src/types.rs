use soroban_sdk::{Address, BytesN, contracttype};

/// Vault status enum representing the current state of a vault
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum VaultStatus {
    Active = 0,
    Locked = 1,
    /// The lock period has completed and the vault has been explicitly
    /// marked unlocked. This state is distinct from `Active` because it
    /// indicates maturity has been reached.
    Unlocked = 2,
    Closed = 3,
    Liquidated = 4,
    Frozen = 5,
}

/// Lending pool status with additional states
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum PoolStatus {
    Active = 0,
    Paused = 1,
    Closed = 2,
    Emergency = 3,
    RateLimited = 4,
}

/// Loan status shared by the lending/borrowing contracts.
///
/// NOTE: the uploaded repo defined this enum twice (once here with a
/// `Defaulted` variant used by the older `LoanInfo`-based borrowing draft,
/// and once later with only `Active`/`Repaid`/`Liquidated` used by the
/// newer `Loan`-based draft). Kept as one definition, since both drafts
/// only ever reference `Active`, `Repaid`, and `Liquidated` in practice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum LoanStatus {
    Active = 0,
    Repaid = 1,
    Liquidated = 2,
    Defaulted = 3,
}

/// Rate limit configuration for operations
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct RateLimit {
    pub max_operations_per_period: u64,
    pub period_seconds: u64,
    pub current_count: u64,
    pub period_start: u64,
}

impl RateLimit {
    pub fn new(max_ops: u64, period: u64) -> Self {
        Self {
            max_operations_per_period: max_ops,
            period_seconds: period,
            current_count: 0,
            period_start: 0,
        }
    }
}

/// Access control roles
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum Role {
    Admin = 0,
    Operator = 1,
    User = 2,
    None = 3,
}

/// Permission configuration
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct Permission {
    pub role: Role,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
}

/// Collateral ratio configuration
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct CollateralConfig {
    pub asset: BytesN<32>,
    pub liquidation_threshold: i128, // Basis points
    pub loan_to_value: i128,         // Basis points
    pub safety_factor: i128,         // Basis points
}

/// Loan information (used by the older `LoanInfo`-based borrowing draft)
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct LoanInfo {
    pub loan_id: BytesN<32>,
    pub borrower: Address,
    pub collateral_asset: BytesN<32>,
    pub collateral_amount: i128,
    pub borrow_asset: BytesN<32>,
    pub borrow_amount: i128,
    pub status: LoanStatus,
    pub created_at: u64,
    pub last_updated: u64,
    pub interest_accrued: i128,
}

/// Asset identifier using a Soroban token contract address
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct Asset {
    /// Token contract address
    pub token: Address,
    /// Human-readable asset symbol for indexing
    pub symbol: BytesN<32>,
    pub code: BytesN<32>,
    pub issuer: Address,
}

/// Vault metadata containing lock period and token identity
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct VaultMetadata {
    pub owner: Address,
    pub asset: Asset,
    pub lock_period: u64,
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

/// Collateralized loan state (used by the newer `Loan`-based borrowing draft).
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

/// Fixed-point number for precise financial calculations
/// Uses 18 decimal places (similar to Ethereum's WAD)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fixed {
    pub value: i128,
}

impl Fixed {
    pub const SCALE: i128 = 1_000_000_000_000_000_000;
    pub const ZERO: Fixed = Fixed { value: 0 };
    pub const ONE: Fixed = Fixed { value: Self::SCALE };

    pub const fn from_int(value: i128) -> Self {
        Fixed { value: value * Self::SCALE }
    }

    pub fn to_int(self) -> i128 {
        self.value / Self::SCALE
    }

    pub fn checked_add(self, other: Fixed) -> Option<Fixed> {
        self.value.checked_add(other.value).map(Fixed::new)
    }

    pub fn checked_sub(self, other: Fixed) -> Option<Fixed> {
        self.value.checked_sub(other.value).map(Fixed::new)
    }

    pub fn checked_mul(self, other: Fixed) -> Option<Fixed> {
        self.value
            .checked_mul(other.value)
            .and_then(|v| v.checked_div(Self::SCALE))
            .map(Fixed::new)
    }

    pub fn checked_div(self, other: Fixed) -> Option<Fixed> {
        self.value
            .checked_mul(Self::SCALE)
            .and_then(|v| v.checked_div(other.value))
            .map(Fixed::new)
    }

    pub const fn new(value: i128) -> Self {
        Self { value }
    }

    pub fn is_zero(self) -> bool {
        self.value == 0
    }
}

/// Lending pool configuration
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct PoolConfig {
    pub asset: BytesN<32>,
    pub admin: Address,
    pub interest_rate: i128, // Annual interest rate in basis points (10000 = 100%)
    pub last_update: u64,
    pub reserve_factor: i128, // Portion of interest sent to reserves (basis points)
}

/// Lending pool accounting state
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct PoolAccounting {
    pub total_assets: i128,      // Total tokens held by the pool
    pub total_shares: i128,     // Total supplier shares minted
    pub available_liquidity: i128, // Tokens available for withdrawal/borrowing
    pub outstanding_debt: i128,  // Tokens currently borrowed
    pub accrued_interest: i128, // Interest accumulated but not distributed
    pub interest_index: i128,    // Cumulative interest index (scaled by 1e18)
}

/// Supplier share balance
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ShareBalance {
    pub shares: i128,
    pub last_interest_index: i128,
}

/// Interest accrual parameters with advanced features
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct InterestParams {
    pub rate_per_second: i128, // Interest rate per second (scaled by 1e18)
    pub last_accrual_time: u64,
    pub utilization_multiplier: i128, // Multiplier based on utilization
    pub base_rate: i128, // Base interest rate
}

/// Flash loan protection
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct FlashLoanProtection {
    pub enabled: bool,
    pub max_fee: i128, // Maximum flash loan fee in basis points
    pub cooldown_period: u64,
}

/// Emergency stop configuration
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct EmergencyStop {
    pub active: bool,
    pub triggered_by: Address,
    pub triggered_at: u64,
    pub reason: BytesN<32>,
}