use shared::events::{StreakFreezeUsed, StreakUpdated};
use soroban_sdk::{Address, Env};

pub struct Events;

impl Events {
    /// Emit a `StreakUpdated` event after a streak state change.
    pub fn streak_updated(env: &Env, user: &Address, streak_count: u32, last_activity: u64) {
        env.events().publish(
            ("streak_updated", user.clone()),
            StreakUpdated {
                user: user.clone(),
                streak_count,
                last_activity,
            },
        );
    }

    /// Emit a `StreakFreezeUsed` event when a freeze is consumed.
    pub fn freeze_used(env: &Env, user: &Address, remaining_freezes: u32) {
        env.events().publish(
            ("freeze_used", user.clone()),
            StreakFreezeUsed {
                user: user.clone(),
                remaining_freezes,
            },
        );
    }
}
