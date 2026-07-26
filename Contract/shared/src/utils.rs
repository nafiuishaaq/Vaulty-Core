use soroban_sdk::Env;

/// Safe arithmetic operations with overflow/underflow checking
pub struct SafeMath;

impl SafeMath {
    /// Safe addition that returns None on overflow
    pub fn add(a: i128, b: i128) -> Option<i128> {
        a.checked_add(b)
    }

    /// Safe subtraction that returns None on underflow
    pub fn sub(a: i128, b: i128) -> Option<i128> {
        a.checked_sub(b)
    }

    /// Safe multiplication that returns None on overflow
    pub fn mul(a: i128, b: i128) -> Option<i128> {
        a.checked_mul(b)
    }

    /// Safe division that returns None on division by zero
    pub fn div(a: i128, b: i128) -> Option<i128> {
        if b == 0 {
            None
        } else {
            Some(a / b)
        }
    }
}

/// Timestamp helpers for time-based operations
pub struct TimeHelper;

impl TimeHelper {
    /// Get the current ledger timestamp in seconds
    pub fn now(env: &Env) -> u64 {
        env.ledger().timestamp()
    }

    /// Check if a timestamp is in the past
    pub fn is_past(env: &Env, timestamp: u64) -> bool {
        TimeHelper::now(env) >= timestamp
    }

    /// Check if a timestamp is in the future
    pub fn is_future(env: &Env, timestamp: u64) -> bool {
        TimeHelper::now(env) < timestamp
    }

    /// Calculate seconds until a timestamp
    pub fn seconds_until(env: &Env, timestamp: u64) -> u64 {
        let now = TimeHelper::now(env);
        if timestamp > now {
            timestamp - now
        } else {
            0
        }
    }
}

/// Validation helpers for common checks
pub struct ValidationHelper;

impl ValidationHelper {
    pub fn validate_positive_amount(amount: i128) -> bool {
        amount > 0
    }

    pub fn validate_non_negative_amount(amount: i128) -> bool {
        amount >= 0
    }

    pub fn validate_lock_period(lock_period: u64) -> bool {
        lock_period >= 1 && lock_period <= 157_788_000
    }

    pub fn validate_interest_rate(rate_bps: u32) -> bool {
        rate_bps <= 10_000
    }

    pub fn required_collateral_for_borrow(borrow_amount: i128, collateral_ratio_bps: u32) -> Option<i128> {
        if borrow_amount <= 0 || collateral_ratio_bps == 0 {
            return Some(0);
        }
        let scaled = SafeMath::mul(borrow_amount, i128::from(collateral_ratio_bps))?;
        SafeMath::div(scaled, 10_000)
    }

    pub fn calculate_interest(principal: i128, rate_bps: u32, elapsed_seconds: u64) -> Option<i128> {
        if principal <= 0 || rate_bps == 0 || elapsed_seconds == 0 {
            return Some(0);
        }
        let annualized = SafeMath::mul(principal, i128::from(rate_bps))?;
        let elapsed_units = SafeMath::mul(annualized, i128::from(elapsed_seconds))?;
        let seconds_per_year = 365_i128 * 24 * 60 * 60 * 10_000;
        SafeMath::div(elapsed_units, seconds_per_year)
    }
}

/// Additional time helpers specifically for streak calculations
pub struct StreakTimeHelper;

impl StreakTimeHelper {
    /// Seconds in a day (86400)
    const DAY_SECONDS: u64 = 86400;

    /// Get the current UTC day period (timestamp floored to midnight UTC)
    pub fn get_current_period(env: &Env) -> u64 {
        let now = TimeHelper::now(env);
        (now / Self::DAY_SECONDS) * Self::DAY_SECONDS
    }

    /// Calculate the number of days between two timestamps
    pub fn days_between(start: u64, end: u64) -> u64 {
        if end <= start {
            return 0;
        }
        (end - start) / Self::DAY_SECONDS
    }

    /// Check if two timestamps are within the same day period
    pub fn is_same_period(ts1: u64, ts2: u64) -> bool {
        (ts1 / Self::DAY_SECONDS) == (ts2 / Self::DAY_SECONDS)
    }

    /// Check if timestamp is the immediate consecutive day after the previous
    pub fn is_consecutive_day(previous_ts: u64, current_ts: u64) -> bool {
        let previous_period = (previous_ts / Self::DAY_SECONDS) * Self::DAY_SECONDS;
        let next_expected = previous_period + Self::DAY_SECONDS;
        let current_period = (current_ts / Self::DAY_SECONDS) * Self::DAY_SECONDS;
        current_period == next_expected
    }

    /// Check if more than one day has passed (missed a day)
    pub fn missed_day(previous_ts: u64, current_ts: u64) -> bool {
        let previous_period = (previous_ts / Self::DAY_SECONDS) * Self::DAY_SECONDS;
        let current_period = (current_ts / Self::DAY_SECONDS) * Self::DAY_SECONDS;
        current_period > previous_period + Self::DAY_SECONDS
    }
}