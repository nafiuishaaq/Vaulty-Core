use soroban_sdk::contracterror;

/// Shared error codes used across all Vaulty contracts.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 1,
    VaultNotFound = 2,
    InvalidAmount = 3,
    InsufficientBalance = 4,
    VaultLocked = 5,
    InvalidLockPeriod = 6,
    Overflow = 7,
    Underflow = 8,
    InvalidAsset = 9,
    AlreadyInitialized = 10,
    NotInitialized = 11,
    InvalidTimestamp = 12,
    StreakNotFound = 13,
    LoanNotFound = 14,
    InsufficientCollateral = 15,
    LiquidationThreshold = 16,
    RewardAlreadyClaimed = 17,
    InvalidParameters = 18,
    DuplicateActivity = 19,
    NoFreezesAvailable = 20,
    MilestoneAlreadyReached = 21,
    RewardsPoolNotInitialized = 22,
    RewardsPoolAlreadyInitialized = 23,
    InsufficientRewardLiquidity = 24,
    UnauthorizedAdmin = 25,
    RewardNotEligible = 26,
    InvalidInterestRate = 27,
    PoolNotFound = 28,
    PoolAlreadyInitialized = 29,
    LoanAlreadyExists = 30,
    LoanAlreadyRepaid = 31,
    LiquidationNotAllowed = 32,
    NoPositionFound = 33,
    InsufficientShares = 34,
}