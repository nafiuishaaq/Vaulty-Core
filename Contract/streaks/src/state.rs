use shared::{errors::Error, storage::StorageHelper, types::UserStreak};
use soroban_sdk::{contracttype, Address, BytesN, Env, Map, Vec};

/// Storage keys for the streak contract.
#[derive(Clone)]
#[contracttype]
pub enum StreakKey {
    /// Per-user activity history (list of UTC day periods).
    ActivityHistory(Address),
    /// Authorized callers (instance storage).
    AuthorizedCallers,
}

/// Initial number of freezes granted to every new streak.
pub const INITIAL_FREEZES: u32 = 3;

pub struct State;

impl State {
    fn streaks_storage_key(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[7u8; 32])
    }

    pub fn is_initialized(env: &Env) -> bool {
        let key = StreakKey::AuthorizedCallers;
        env.storage().instance().has(&key)
    }

    pub fn get_authorized_callers(env: &Env) -> Result<Vec<Address>, Error> {
        let key = StreakKey::AuthorizedCallers;
        let authorized: Option<Vec<Address>> = env.storage().instance().get(&key);
        authorized.ok_or(Error::NotInitialized)
    }

    pub fn set_authorized_callers(env: &Env, callers: &Vec<Address>) {
        let key = StreakKey::AuthorizedCallers;
        env.storage().instance().set(&key, callers);
    }

    pub fn streak_exists(env: &Env, user: &Address) -> bool {
        let key = Self::streaks_storage_key(env);
        let streaks: Map<Address, UserStreak> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Map::new(env));
        let exists = streaks.get(user.clone()).is_some();
        if exists {
            StorageHelper::touch_streak(env, &key);
        }
        exists
    }

    pub fn get_streak(env: &Env, user: &Address) -> Result<UserStreak, Error> {
        let key = Self::streaks_storage_key(env);
        let streaks: Map<Address, UserStreak> = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::StreakNotFound)?;

        let streak = streaks.get(user.clone()).ok_or(Error::StreakNotFound)?;

        StorageHelper::touch_streak(env, &key);
        Ok(streak)
    }

    pub fn set_streak(env: &Env, user: &Address, streak: &UserStreak) -> Result<(), Error> {
        let key = Self::streaks_storage_key(env);
        let mut streaks: Map<Address, UserStreak> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Map::new(env));
        streaks.set(user.clone(), streak.clone());
        env.storage().persistent().set(&key, &streaks);
        StorageHelper::touch_streak(env, &key);
        Ok(())
    }

    pub fn get_activity_history(env: &Env, user: &Address) -> Result<Vec<u64>, Error> {
        let key = StreakKey::ActivityHistory(user.clone());
        let history: Vec<u64> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env));
        StorageHelper::touch_streak(env, &key);
        Ok(history)
    }

    pub fn set_activity_history(env: &Env, user: &Address, history: &Vec<u64>) -> Result<(), Error> {
        let key = StreakKey::ActivityHistory(user.clone());
        env.storage().persistent().set(&key, history);
        StorageHelper::touch_streak(env, &key);
        Ok(())
    }
}
