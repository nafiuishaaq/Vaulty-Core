use shared::errors::Error;
use soroban_sdk::{Address, Env};

use crate::state::State;

pub struct Activity;

impl Activity {
    /// Record a new activity period for a user.
    pub fn record(env: &Env, user: &Address, period: u64) -> Result<(), Error> {
        let mut history = State::get_activity_history(env, user)?;
        history.push_back(period);
        State::set_activity_history(env, user, &history)
    }

    /// Check whether the user already has activity in the given UTC day period.
    pub fn has_activity_in_period(env: &Env, user: &Address, period: u64) -> Result<bool, Error> {
        let history = State::get_activity_history(env, user)?;
        Ok(history.contains(&period))
    }

    /// Validate that no duplicate activity exists for the current UTC day.
    ///
    /// # Errors
    /// Returns `Error::DuplicateActivity` if the user already has activity recorded
    /// in the same UTC day period.
    pub fn reject_duplicate(env: &Env, user: &Address, period: u64) -> Result<(), Error> {
        if Self::has_activity_in_period(env, user, period)? {
            return Err(Error::DuplicateActivity);
        }
        Ok(())
    }
}
