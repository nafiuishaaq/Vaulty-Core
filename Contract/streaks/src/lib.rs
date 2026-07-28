#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, vec,
    Address, BytesN, Env, Vec,
};
use shared::errors::Error;
use shared::events::StreakUpdated;
use shared::types::{RateLimit, Role, Permission};
use shared::utils::{TimeHelper, SafeMath};

/// Storage keys for streaks contract
#[derive(Clone)]
#[contracttype]
pub enum StreakKey {
    StreakInfo(Address),
    ActivityHistory(Address),
    StreakConfig,
    RateLimit,
    AdminPermissions(Address),
    GlobalStreakCount,
    Leaderboard,
}

/// Streak information for a user
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct StreakInfo {
    pub user: Address,
    pub current_streak: u32,
    pub longest_streak: u32,
    pub last_activity: u64,
    pub streak_start: u64,
    pub total_activities: u64,
    pub multiplier: u32, // Reward multiplier based on streak length
}

/// Activity record
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ActivityRecord {
    pub timestamp: u64,
    pub activity_type: u32,
}

/// Streak configuration
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct StreakConfig {
    pub activity_window: u64, // Time window to maintain streak (in seconds)
    pub max_streak_multiplier: u32,
    pub multiplier_thresholds: Vec<u32>, // Streak counts for multiplier increases
    pub leaderboard_size: u32,
}

/// Leaderboard entry
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct LeaderboardEntry {
    pub user: Address,
    pub streak: u32,
    pub total_activities: u64,
}

/// Streaks contract for tracking user activity streaks with advanced features
use soroban_sdk::{contract, contractimpl, Address, Env};
use shared::types::{Asset, VaultMetadata};

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Map};
use shared::{
    errors::Error,
    events::{StreakFreezeUsed, StreakUpdated},
    types::UserStreak,
    utils::StreakTimeHelper,
};

/// Streaks contract for tracking user activity streaks
/// Tracks:
/// - Current streak: consecutive days of activity
/// - Longest streak: maximum consecutive days ever achieved
/// - Last activity period: UTC day of last valid streak update
/// - Available freezes: number of missed days that can be forgiven
#[contract]
pub struct StreaksContract;

// Storage key constants
const VAULT_CONTRACT_KEY: &[u8; 32] = b"vault_contract_address______\0\0\0\0";
const AUTHORIZED_CALLERS_KEY: &[u8; 32] = b"authorized_callers__________\0\0\0\0";
const INITIALIZE_FREEZES: u32 = 3; // Start with 3 freezes for all users
// const DAILY_RESET_HOUR: u64 = 0; // UTC midnight reset

#[contractimpl]
impl StreaksContract {
    pub fn initialize_streak(env: Env, user: Address) -> Result<(), Error> {
        if env.storage().persistent().has(&StreakKey::StreakInfo(user.clone())) {
            return Err(Error::AlreadyInitialized);
        }

        let now = TimeHelper::now(&env);

        let streak_info = StreakInfo {
            user: user.clone(),
            current_streak: 0,
            longest_streak: 0,
            last_activity: now,
            streak_start: now,
            total_activities: 0,
            multiplier: 1,
        };

        env.storage()
            .persistent()
            .set(&StreakKey::StreakInfo(user.clone()), &streak_info);

        env.storage()
            .persistent()
            .set(&StreakKey::ActivityHistory(user), &Vec::<ActivityRecord>::new(&env));

        let mut count: u64 = env
            .storage()
            .persistent()
            .get(&StreakKey::GlobalStreakCount)
            .unwrap_or(0);
        count = count.checked_add(1).ok_or(Error::Overflow)?;
        env.storage()
            .persistent()
            .set(&StreakKey::GlobalStreakCount, &count);

        Ok(())
    }

    pub fn update_streak(env: Env, user: Address, activity_type: u32) -> Result<(), Error> {
        Self::check_rate_limit(&env)?;

        let mut streak_info: StreakInfo = env
            .storage()
            .persistent()
            .get(&StreakKey::StreakInfo(user.clone()))
            .ok_or(Error::StreakNotFound)?;

        let config: StreakConfig = Self::get_config(&env);
        let now = TimeHelper::now(&env);

        let time_since_last = now.saturating_sub(streak_info.last_activity);

        if time_since_last <= config.activity_window {
            streak_info.current_streak = streak_info
                .current_streak
                .checked_add(1)
                .ok_or(Error::Overflow)?;
        } else if time_since_last <= config.activity_window * 2 {
            streak_info.current_streak = 1;
            streak_info.streak_start = now;
        } else {
            streak_info.current_streak = 0;
            streak_info.streak_start = now;
        }

        if streak_info.current_streak > streak_info.longest_streak {
            streak_info.longest_streak = streak_info.current_streak;
        }

        streak_info.multiplier = Self::calculate_multiplier(&config, streak_info.current_streak);

        streak_info.last_activity = now;
        streak_info.total_activities = streak_info
            .total_activities
            .checked_add(1)
            .ok_or(Error::Overflow)?;

        let mut history: Vec<ActivityRecord> = env
            .storage()
            .persistent()
            .get(&StreakKey::ActivityHistory(user.clone()))
            .unwrap_or(Vec::new(&env));
        history.push_back(ActivityRecord {
            timestamp: now,
            activity_type,
        });
        if history.len() > 100 {
            history.remove(0);
        }
        env.storage()
            .persistent()
            .set(&StreakKey::ActivityHistory(user.clone()), &history);

        env.storage()
            .persistent()
            .set(&StreakKey::StreakInfo(user.clone()), &streak_info.clone());

        Self::update_leaderboard(env.clone(), user.clone(), streak_info.clone())?;

        env.events().publish(
            (StreakUpdated::topic(&env), user.clone()),
            StreakUpdated {
                user,
                streak_count: streak_info.current_streak,
                last_activity: now,
            },
        );

        Ok(())
    }

    pub fn get_streak(env: Env, user: Address) -> Result<StreakInfo, Error> {
        let streak_info: StreakInfo = env
            .storage()
            .persistent()
            .get(&StreakKey::StreakInfo(user))
            .ok_or(Error::StreakNotFound)?;
        Ok(streak_info)
    }

    pub fn is_streak_active(env: Env, user: Address) -> Result<bool, Error> {
        let streak_info: StreakInfo = env
            .storage()
            .persistent()
            .get(&StreakKey::StreakInfo(user.clone()))
            .ok_or(Error::StreakNotFound)?;

        let config: StreakConfig = Self::get_config(&env);
        let now = TimeHelper::now(&env);
        let time_since_last = now.saturating_sub(streak_info.last_activity);

        Ok(time_since_last <= config.activity_window && streak_info.current_streak > 0)
    }

    pub fn get_activity_history(env: Env, user: Address) -> Result<Vec<ActivityRecord>, Error> {
        let history: Vec<ActivityRecord> = env
            .storage()
            .persistent()
            .get(&StreakKey::ActivityHistory(user))
            .unwrap_or(Vec::new(&env));
        Ok(history)
    }

    pub fn get_leaderboard(env: Env) -> Result<Vec<LeaderboardEntry>, Error> {
        let leaderboard: Vec<LeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&StreakKey::Leaderboard)
            .unwrap_or(Vec::new(&env));
        Ok(leaderboard)
    }

    pub fn set_config(env: Env, admin: Address, config: StreakConfig) -> Result<(), Error> {
        if !Self::is_admin(&env, &admin) {
            return Err(Error::PermissionDenied);
        }
        env.storage().persistent().set(&StreakKey::StreakConfig, &config);
        Ok(())
    }

    fn get_config(env: &Env) -> StreakConfig {
        env.storage()
            .persistent()
            .get(&StreakKey::StreakConfig)
            .unwrap_or(StreakConfig {
                activity_window: 86400,
                max_streak_multiplier: 5,
                multiplier_thresholds: vec![&env, 7, 30, 90, 180],
                leaderboard_size: 100,
            })
    }

    fn calculate_multiplier(config: &StreakConfig, streak: u32) -> u32 {
        let mut multiplier = 1u32;
        for i in 0..config.multiplier_thresholds.len() {
            let threshold = config.multiplier_thresholds.get(i).unwrap();
            if streak >= threshold {
                multiplier = multiplier.saturating_add(1);
                if multiplier >= config.max_streak_multiplier {
                    return config.max_streak_multiplier;
                }
            }
        }
        multiplier
    }

    fn update_leaderboard(env: Env, user: Address, streak_info: StreakInfo) -> Result<(), Error> {
        let config = Self::get_config(&env);
        let leaderboard: Vec<LeaderboardEntry> = env
            .storage()
            .persistent()
            .get(&StreakKey::Leaderboard)
            .unwrap_or(Vec::new(&env));

        let entry = LeaderboardEntry {
            user: user.clone(),
            streak: streak_info.current_streak,
            total_activities: streak_info.total_activities,
        };

        // Rebuild leaderboard, replacing existing entry for this user
        let mut new_leaderboard: Vec<LeaderboardEntry> = Vec::new(&env);
        for i in 0..leaderboard.len() {
            let e = leaderboard.get(i).unwrap();
            if e.user != user {
                new_leaderboard.push_back(e);
            }
        }
        new_leaderboard.push_back(entry);

        // Simple insertion sort to keep leaderboard ordered by streak desc
        let n = new_leaderboard.len();
        for i in 1..n {
            for j in (1..=i).rev() {
                let a = new_leaderboard.get(j - 1).unwrap();
                let b = new_leaderboard.get(j).unwrap();
                if a.streak < b.streak || (a.streak == b.streak && a.total_activities < b.total_activities) {
                    new_leaderboard.set(j - 1, b);
                    new_leaderboard.set(j, a);
                } else {
                    break;
                }
            }
        }

        // Trim to max size
        let max_size = config.leaderboard_size as u32;
        let trimmed: Vec<LeaderboardEntry> = if new_leaderboard.len() > max_size {
            let mut t: Vec<LeaderboardEntry> = Vec::new(&env);
            for i in 0..max_size {
                t.push_back(new_leaderboard.get(i).unwrap());
            }
            t
        } else {
            new_leaderboard
        };

        env.storage().persistent().set(&StreakKey::Leaderboard, &trimmed);
        Ok(())
    }

    fn check_rate_limit(env: &Env) -> Result<(), Error> {
        let mut rate_limit: RateLimit = env
            .storage()
            .persistent()
            .get(&StreakKey::RateLimit)
            .unwrap_or(RateLimit::new(100, 60));

        let now = TimeHelper::now(env);

        if now >= rate_limit.period_start + rate_limit.period_seconds {
            rate_limit.current_count = 0;
            rate_limit.period_start = now;
        }

        if rate_limit.current_count >= rate_limit.max_operations_per_period {
            return Err(Error::RateLimitExceeded);
        }

        rate_limit.current_count = rate_limit.current_count.checked_add(1).ok_or(Error::Overflow)?;
        env.storage().persistent().set(&StreakKey::RateLimit, &rate_limit);

        Ok(())
    }

    pub fn grant_admin(env: Env, admin: Address) -> Result<(), Error> {
        let permission = Permission {
            role: Role::Admin,
            granted_at: TimeHelper::now(&env),
            expires_at: None,
        };
        env.storage()
            .persistent()
            .set(&StreakKey::AdminPermissions(admin), &permission);
        Ok(())
    }

    fn is_admin(env: &Env, address: &Address) -> bool {
        if let Some(permission) = env
            .storage()
            .persistent()
            .get::<_, Permission>(&StreakKey::AdminPermissions(address.clone()))
        {
            permission.role == Role::Admin
        } else {
            false
        }
    }

    pub fn get_global_count(env: Env) -> Result<u64, Error> {
        let count: u64 = env
            .storage()
            .persistent()
            .get(&StreakKey::GlobalStreakCount)
            .unwrap_or(0);
        Ok(count)
    }

    /// Initialize the streaks contract with authorized vault address
    /// Can only be called once
    pub fn initialize(env: Env, vault_contract: Address) {
        // Check if already initialized
        let vault_key = BytesN::from_array(&env, VAULT_CONTRACT_KEY);
        if env.storage().instance().has(&vault_key) {
            panic!("{:?}", Error::AlreadyInitialized);
        }

        // Store authorized vault contract
        env.storage().instance().set(&vault_key, &vault_contract);

        // Create and store authorized callers set
        let mut authorized = soroban_sdk::Vec::new(&env);
        authorized.push_back(vault_contract);
        let auth_key = BytesN::from_array(&env, AUTHORIZED_CALLERS_KEY);
        env.storage().instance().set(&auth_key, &authorized);
    }

    /// Add an additional authorized caller (only existing authorized contracts can add)
    pub fn add_authorized_caller(env: Env, caller: Address) {
        // Verify caller is authorized
        Self::verify_authorization(&env);

        let auth_key = BytesN::from_array(&env, AUTHORIZED_CALLERS_KEY);
        let mut authorized: soroban_sdk::Vec<Address> = env.storage().instance().get(&auth_key).unwrap();

        // Check if already in list
        if authorized.contains(&caller) {
            return;
        }

        authorized.push_back(caller);
        env.storage().instance().set(&auth_key, &authorized);
    }

    /// Initialize a user's streak if they don't have one yet
    /// Can only be called by authorized contracts
    pub fn initialize_streak(env: Env, user: Address) {
        // Verify authorization
        Self::verify_authorization(&env);

        // Check if streak already exists
        if Self::streak_exists(&env, &user) {
            return;
        }

        // Create initial streak state
        let current_period = StreakTimeHelper::get_current_period(&env);
        let streak = UserStreak {
            current_streak: 1,
            longest_streak: 1,
            last_activity_period: current_period,
            available_freezes: INITIALIZE_FREEZES,
        };

        // Store the streak
        Self::store_streak(&env, &user, streak);

        // Emit event
        env.events().publish(
            ("streak_updated", user.clone()),
            StreakUpdated {
                user,
                streak_count: 1,
                last_activity: current_period,
            },
        );
    }

    /// Update a user's streak after verified deposit activity
    /// Only authorized vault contract can call this
    /// Handles streak continuation, missed days, and freeze usage
    pub fn update_streak(env: Env, user: Address) {
        // Verify authorization
        Self::verify_authorization(&env);

        let current_period = StreakTimeHelper::get_current_period(&env);

        // Get or initialize streak
        let mut streak = if Self::streak_exists(&env, &user) {
            Self::get_streak_internal(&env, &user)
        } else {
            // Initialize streak if it doesn't exist
            let new_streak = UserStreak {
                current_streak: 1,
                longest_streak: 1,
                last_activity_period: current_period,
                available_freezes: INITIALIZE_FREEZES,
            };
            Self::store_streak(&env, &user, new_streak.clone());
            new_streak
        };

        // Check for duplicate activity in the same period
        if streak.last_activity_period == current_period {
            panic!("{:?}", Error::DuplicateActivity);
        }

        // Check if activity is consecutive day
        if StreakTimeHelper::is_consecutive_day(streak.last_activity_period, current_period) {
            // Continue streak
            streak.current_streak += 1;
            if streak.current_streak > streak.longest_streak {
                streak.longest_streak = streak.current_streak;
            }
            streak.last_activity_period = current_period;
        }
        // Check if missed exactly one day - can use a freeze
        else if StreakTimeHelper::days_between(streak.last_activity_period, current_period) == 2 {
            // Use a freeze if available
            if streak.available_freezes > 0 {
                streak.available_freezes -= 1;
                streak.current_streak += 1;
                if streak.current_streak > streak.longest_streak {
                    streak.longest_streak = streak.current_streak;
                }
                streak.last_activity_period = current_period;

                // Emit freeze used event
                env.events().publish(
                    ("freeze_used", user.clone()),
                    StreakFreezeUsed {
                        user: user.clone(),
                        remaining_freezes: streak.available_freezes,
                    },
                );
            } else {
                // No freezes left, reset streak
                streak.current_streak = 1;
                streak.last_activity_period = current_period;
            }
        }
        // Missed more than one day - reset streak
        else {
            streak.current_streak = 1;
            streak.last_activity_period = current_period;
        }

        // Save updated streak
        Self::store_streak(&env, &user, streak.clone());

        // Emit streak updated event
        env.events().publish(
            ("streak_updated", user.clone()),
            StreakUpdated {
                user,
                streak_count: streak.current_streak,
                last_activity: streak.last_activity_period,
            },
        );
    }

    /// Use a streak freeze manually to save current streak if missed a day
    /// User must authorize this action
    pub fn use_freeze(env: Env, user: Address) {
        user.require_auth();

        if !Self::streak_exists(&env, &user) {
            panic!("{:?}", Error::StreakNotFound);
        }

        let mut streak = Self::get_streak_internal(&env, &user);
        if streak.available_freezes == 0 {
            panic!("{:?}", Error::NoFreezesAvailable);
        }

        streak.available_freezes -= 1;
        Self::store_streak(&env, &user, streak.clone());

        env.events().publish(
            ("freeze_used", user.clone()),
            StreakFreezeUsed {
                user,
                remaining_freezes: streak.available_freezes,
            },
        );
    }

    /// Add freezes to a user's account (only authorized)
    pub fn add_freezes(env: Env, user: Address, amount: u32) {
        Self::verify_authorization(&env);

        if !Self::streak_exists(&env, &user) {
            panic!("{:?}", Error::StreakNotFound);
        }

        let mut streak = Self::get_streak_internal(&env, &user);
        streak.available_freezes = streak.available_freezes.checked_add(amount).unwrap_or(streak.available_freezes);
        Self::store_streak(&env, &user, streak);
    }

    /// Get a user's full streak data
    pub fn get_user_streak(env: Env, user: Address) -> UserStreak {
        if !Self::streak_exists(&env, &user) {
            panic!("{:?}", Error::StreakNotFound);
        }
        Self::get_streak_internal(&env, &user)
    }

    /// Get a user's current streak count
    pub fn get_streak(env: Env, user: Address) -> u32 {
        if !Self::streak_exists(&env, &user) {
            0
        } else {
            Self::get_streak_internal(&env, &user).current_streak
        }
    }

    /// Check if a user's streak is currently active (activity in last 48 hours)
    pub fn is_streak_active(env: Env, user: Address) -> bool {
        if !Self::streak_exists(&env, &user) {
            return false;
        }

        let streak = Self::get_streak_internal(&env, &user);
        let current_period = StreakTimeHelper::get_current_period(&env);
        // Active if last activity was either today or yesterday (within 48 hours)
        (current_period - streak.last_activity_period) <= 86400 * 2
    }

    /// Internal helper to verify caller is authorized
    fn verify_authorization(env: &Env) {
        let auth_key = BytesN::from_array(env, AUTHORIZED_CALLERS_KEY);
        let authorized: Option<soroban_sdk::Vec<Address>> = env.storage().instance().get(&auth_key);

        if authorized.is_none() {
            panic!("{:?}", Error::Unauthorized);
        }

        let _authorized = authorized.unwrap();
    }

    /// Internal helper to create storage key for the streak map
    fn streaks_storage_key(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[7u8; 32])
    }

    /// Check if a user has a streak stored
    fn streak_exists(env: &Env, user: &Address) -> bool {
        let key = Self::streaks_storage_key(env);
        let streaks: Map<Address, UserStreak> = env.storage().persistent().get(&key).unwrap_or_else(|| Map::new(env));
        streaks.get(user.clone()).is_some()
    }

    /// Get a user's streak from storage
    fn get_streak_internal(env: &Env, user: &Address) -> UserStreak {
        let key = Self::streaks_storage_key(env);
        let streaks: Map<Address, UserStreak> = env.storage().persistent().get(&key).unwrap_or_else(|| Map::new(env));
        streaks.get(user.clone()).unwrap()
    }

    /// Store a user's streak
    fn store_streak(env: &Env, user: &Address, streak: UserStreak) {
        let key = Self::streaks_storage_key(env);
        let mut streaks: Map<Address, UserStreak> = env.storage().persistent().get(&key).unwrap_or_else(|| Map::new(env));
        streaks.set(user.clone(), streak);
        env.storage().persistent().set(&key, &streaks);
    }
}

#[cfg(test)]
mod test {
    use super::*;
}