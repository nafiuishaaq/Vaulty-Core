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

    /// Pool not found - the specified lending pool does not exist
    PoolNotFound = 19,

    /// Pool already exists - a pool for this asset already exists
    PoolAlreadyExists = 20,

    /// Insufficient liquidity - not enough available liquidity for operation
    InsufficientLiquidity = 21,

    /// Invalid interest rate - interest rate outside allowed bounds
    InvalidInterestRate = 22,

    /// Invalid share amount - share amount must be positive
    InvalidShareAmount = 23,

    /// Pool paused - operation not allowed while pool is paused
    PoolPaused = 24,

    /// Pool closed - operation not allowed on closed pool
    PoolClosed = 25,

    /// Zero shares - cannot withdraw zero shares
    ZeroShares = 26,

    /// Insufficient shares - not enough shares for withdrawal
    InsufficientShares = 27,

    /// Invalid reserve factor - reserve factor outside allowed bounds
    InvalidReserveFactor = 28,

    /// Rate limit exceeded - operation rate limit reached
    RateLimitExceeded = 29,

    /// Emergency stop active - operation blocked due to emergency stop
    EmergencyStopActive = 30,

    /// Permission denied - insufficient permissions for operation
    PermissionDenied = 31,

    /// Loan already exists - loan ID already in use
    LoanAlreadyExists = 32,

    /// Invalid collateral - collateral asset not supported or insufficient
    InvalidCollateral = 33,

    /// Collateral ratio too low - position undercollateralized
    CollateralRatioTooLow = 34,

    /// Flash loan not allowed - flash loans disabled for this pool
    FlashLoanNotAllowed = 35,

    /// Cooldown period not met - operation blocked by cooldown
    CooldownPeriodNotMet = 36,

    /// Invalid timestamp - timestamp outside acceptable range
    InvalidTimestampRange = 37,

    /// Contract paused - operation blocked due to paused state
    ContractPaused = 38,

    /// Reentrancy detected - potential reentrancy attack blocked
    ReentrancyDetected = 39,

    /// Invalid signature - signature verification failed
    InvalidSignature = 40,

    /// Expired deadline - operation deadline has passed
    ExpiredDeadline = 41,
}
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
