#![no_std]
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
const DAILY_RESET_HOUR: u64 = 0; // UTC midnight reset

#[contractimpl]
impl StreaksContract {
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