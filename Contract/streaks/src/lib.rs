#![no_std]

mod activity;
mod authorization;
mod events;
mod leaderboard;
mod state;
mod ttl;

use shared::{errors::Error, types::UserStreak, utils::StreakTimeHelper};
use soroban_sdk::{contract, contractimpl, Address, Env, Vec};

use crate::activity::Activity;
use crate::authorization::Authorization;
use crate::events::Events;
use crate::leaderboard::Leaderboard;
use crate::state::{State, INITIAL_FREEZES};
use crate::ttl::Ttl;

pub use crate::state::StreakKey;

/// Streak state machine for vault-integrated daily activity tracking.
///
/// This contract exposes exactly one authorized implementation for each streak
/// operation. Deposit-triggered updates require vault-contract authorization,
/// while user-triggered actions require the user's own authorization.
///
/// ## Behavioral rules (preserved)
/// - **Consecutive day**: streak increments by 1.
/// - **One missed day with freeze**: a freeze is consumed and streak continues.
/// - **One missed day without freeze**: streak resets to 1.
/// - **Two or more missed days**: streak resets to 1.
/// - **Duplicate activity in the same UTC day**: rejected with `Error::DuplicateActivity`.
///
/// ## Authorization model
/// - `initialize`, `add_authorized_caller`, `update_streak`, `add_freezes`
///   require the caller to be in the authorized callers list.
/// - `use_freeze` requires the user's own authorization via `require_auth`.
///
/// ## UTC day boundaries
/// Activity is tracked in UTC day periods (timestamps floored to midnight UTC).
/// All boundary decisions are deterministic on-chain.
#[contract]
pub struct StreaksContract;

#[contractimpl]
impl StreaksContract {
    /// Initialize the streaks contract with the vault contract as the first authorized caller.
    ///
    /// # Authorization
    /// May only be called once; subsequent calls return `Error::AlreadyInitialized`.
    ///
    /// # Errors
    /// - `Error::AlreadyInitialized` if the contract has already been initialized.
    pub fn initialize(env: Env, vault_contract: Address) -> Result<(), Error> {
        if State::is_initialized(&env) {
            return Err(Error::AlreadyInitialized);
        }

        let mut authorized = Vec::new(&env);
        authorized.push_back(vault_contract);
        State::set_authorized_callers(&env, &authorized);
        Ttl::refresh_authorized_callers(&env);

        Ok(())
    }

    /// Add an additional authorized caller.
    ///
    /// # Authorization
    /// Requires vault authorization.
    ///
    /// # Errors
    /// - `Error::NotInitialized` if the contract has not been initialized.
    /// - `Error::Unauthorized` if the caller is not authorized.
    pub fn add_authorized_caller(env: Env, caller: Address) -> Result<(), Error> {
        Authorization::require_vault_authorization(&env)?;
        Authorization::ensure_initialized(&env)?;

        let mut authorized = State::get_authorized_callers(&env)?;

        if authorized.contains(&caller) {
            return Ok(());
        }

        authorized.push_back(caller);
        State::set_authorized_callers(&env, &authorized);
        Ttl::refresh_authorized_callers(&env);

        Ok(())
    }

    /// Initialize a user's streak if one does not already exist.
    ///
    /// # Authorization
    /// Requires vault authorization.
    ///
    /// # Errors
    /// - `Error::NotInitialized` if the contract has not been initialized.
    /// - `Error::Unauthorized` if the caller is not authorized.
    pub fn initialize_streak(env: Env, user: Address) -> Result<(), Error> {
        Authorization::require_vault_authorization(&env)?;
        Authorization::ensure_initialized(&env)?;

        if State::streak_exists(&env, &user) {
            return Ok(());
        }

        let current_period = StreakTimeHelper::get_current_period(&env);
        let streak = UserStreak {
            current_streak: 1,
            longest_streak: 1,
            last_activity_period: current_period,
            available_freezes: INITIAL_FREEZES,
        };

        State::set_streak(&env, &user, &streak)?;
        let empty_history = Vec::new(&env);
        State::set_activity_history(&env, &user, &empty_history)?;

        Events::streak_updated(&env, &user, 1, current_period);

        Ok(())
    }

    /// Update a user's streak after verified vault deposit activity.
    ///
    /// Handles streak continuation, missed-day freeze usage, and multi-day resets.
    /// Rejects duplicate activity in the same UTC day.
    ///
    /// # Authorization
    /// Requires vault authorization.
    ///
    /// # Errors
    /// - `Error::NotInitialized` if the contract has not been initialized.
    /// - `Error::Unauthorized` if the caller is not authorized.
    /// - `Error::DuplicateActivity` if the user already has activity in the current UTC day.
    pub fn update_streak(env: Env, user: Address) -> Result<(), Error> {
        Authorization::require_vault_authorization(&env)?;
        Authorization::ensure_initialized(&env)?;

        let current_period = StreakTimeHelper::get_current_period(&env);

        let mut streak = if State::streak_exists(&env, &user) {
            State::get_streak(&env, &user)?
        } else {
            let new_streak = UserStreak {
                current_streak: 1,
                longest_streak: 1,
                last_activity_period: current_period,
                available_freezes: INITIAL_FREEZES,
            };
            State::set_streak(&env, &user, &new_streak)?;
            let empty_history = Vec::new(&env);
            State::set_activity_history(&env, &user, &empty_history)?;
            Events::streak_updated(&env, &user, 1, current_period);
            return Ok(());
        };

        Activity::reject_duplicate(&env, &user, current_period)?;

        let days_since_last =
            StreakTimeHelper::days_between(streak.last_activity_period, current_period);

        if days_since_last == 1 {
            streak.current_streak = streak
                .current_streak
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            if streak.current_streak > streak.longest_streak {
                streak.longest_streak = streak.current_streak;
            }
            streak.last_activity_period = current_period;
        } else if days_since_last == 2 && streak.available_freezes > 0 {
            streak.available_freezes -= 1;
            streak.current_streak = streak
                .current_streak
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            if streak.current_streak > streak.longest_streak {
                streak.longest_streak = streak.current_streak;
            }
            streak.last_activity_period = current_period;

            Events::freeze_used(&env, &user, streak.available_freezes);
        } else {
            streak.current_streak = 1;
            streak.last_activity_period = current_period;
        }

        State::set_streak(&env, &user, &streak)?;
        Activity::record(&env, &user, current_period)?;

        Leaderboard::update_leaderboard(&env, &user, streak.current_streak)?;

        Events::streak_updated(&env, &user, streak.current_streak, streak.last_activity_period);

        Ok(())
    }

    /// Manually consume one of the user's available freezes.
    ///
    /// # Authorization
    /// Requires the user's own authorization.
    ///
    /// # Errors
    /// - `Error::StreakNotFound` if the user has no streak.
    /// - `Error::NoFreezesAvailable` if the user has no freezes left.
    pub fn use_freeze(env: Env, user: Address) -> Result<(), Error> {
        Authorization::require_user_authorization(&user)?;

        let mut streak = State::get_streak(&env, &user)?;

        if streak.available_freezes == 0 {
            return Err(Error::NoFreezesAvailable);
        }

        streak.available_freezes -= 1;
        State::set_streak(&env, &user, &streak)?;

        Events::freeze_used(&env, &user, streak.available_freezes);

        Ok(())
    }

    /// Add freezes to a user's account.
    ///
    /// # Authorization
    /// Requires vault authorization.
    ///
    /// # Errors
    /// - `Error::NotInitialized` if the contract has not been initialized.
    /// - `Error::Unauthorized` if the caller is not authorized.
    /// - `Error::StreakNotFound` if the user has no streak.
    pub fn add_freezes(env: Env, user: Address, amount: u32) -> Result<(), Error> {
        Authorization::require_vault_authorization(&env)?;
        Authorization::ensure_initialized(&env)?;

        let mut streak = State::get_streak(&env, &user)?;

        streak.available_freezes = streak
            .available_freezes
            .checked_add(amount)
            .ok_or(Error::Overflow)?;

        State::set_streak(&env, &user, &streak)?;

        Ok(())
    }

    /// Get the user's full streak data.
    ///
    /// # Errors
    /// - `Error::StreakNotFound` if the user has no streak.
    pub fn get_user_streak(env: Env, user: Address) -> Result<UserStreak, Error> {
        State::get_streak(&env, &user)
    }

    /// Get the user's current streak count.
    ///
    /// Returns `0` if the user has no streak.
    pub fn get_streak(env: Env, user: Address) -> u32 {
        State::get_streak(&env, &user)
            .map(|s| s.current_streak)
            .unwrap_or(0)
    }

    /// Check whether the user's streak is active (activity within last 48 hours).
    pub fn is_streak_active(env: Env, user: Address) -> bool {
        let streak = match State::get_streak(&env, &user) {
            Ok(s) => s,
            Err(_) => return false,
        };

        let current_period = StreakTimeHelper::get_current_period(&env);
        current_period.saturating_sub(streak.last_activity_period) <= 86400 * 2
    }

    /// Get the UTC day periods in which the user recorded streak activity.
    pub fn get_activity_history(env: Env, user: Address) -> Result<Vec<u64>, Error> {
        State::get_activity_history(&env, &user)
    }
}
