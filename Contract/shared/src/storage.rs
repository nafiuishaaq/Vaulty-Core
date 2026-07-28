use crate::errors::Error;
use soroban_sdk::{BytesN, Env, IntoVal, Val};

/// Shared storage key type for raw byte-based maps
pub type StorageKey = BytesN<32>;

/// -------------------------------------------------------------------------
/// TTL POLICY
/// -------------------------------------------------------------------------
///
/// Soroban persistent storage expires unless its TTL is periodically extended.
///
/// Every protocol record should renew its TTL whenever it is:
///
/// - created
/// - read
/// - modified
///
/// This helper centralizes the TTL policy for every Vaulty contract.
/// -------------------------------------------------------------------------

pub struct StorageTTL;

/// Ledger estimates (~5 seconds/ledger)
impl StorageTTL {
    /// ≈ 1 day
    pub const DAY: u32 = 17_280;

    /// ≈ 30 days
    pub const MONTH: u32 = Self::DAY * 30;

    /// ≈ 1 year
    pub const YEAR: u32 = Self::DAY * 365;

    /// Safety buffer before expiration
    pub const BUFFER: u32 = Self::MONTH;

    /// ---------------------------------------------------------------------
    /// TTL categories
    /// ---------------------------------------------------------------------

    /// Contract instance storage
    pub const INSTANCE: u32 = Self::YEAR;

    /// Vault metadata
    pub const VAULT: u32 = Self::YEAR * 6;

    /// User balances
    pub const USER: u32 = Self::YEAR * 6;

    /// Reward state
    pub const REWARD: u32 = Self::YEAR * 3;

    /// Lending pools
    pub const LENDING: u32 = Self::YEAR * 6;

    /// Borrow positions
    pub const BORROWING: u32 = Self::YEAR * 6;

    /// Streak records
    pub const STREAK: u32 = Self::YEAR * 6;
}

/// Storage helpers
pub struct StorageHelper;

impl StorageHelper {
    /// ---------------------------------------------------------------------
    /// Basic helpers
    /// ---------------------------------------------------------------------

    pub fn has<K>(env: &Env, key: &K) -> bool
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        env.storage().persistent().has(key)
    }

    pub fn has_instance<K>(env: &Env, key: &K) -> bool
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        env.storage().instance().has(key)
    }

    /// ---------------------------------------------------------------------
    /// Persistent TTL
    /// ---------------------------------------------------------------------

    pub fn extend_persistent<K>(
        env: &Env,
        key: &K,
        threshold: u32,
        extend_to: u32,
    )
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        env.storage()
            .persistent()
            .extend_ttl(key, threshold, extend_to);
    }

    pub fn ensure_persistent_not_expired<K>(
        env: &Env,
        key: &K,
        threshold: u32,
    ) -> Result<(), Error>
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        if !env.storage().persistent().has(key) {
            return Err(Error::NotInitialized);
        }
        env.storage()
            .persistent()
            .extend_ttl(key, threshold, threshold);
        Ok(())
    }

    /// ---------------------------------------------------------------------
    /// Instance TTL
    /// ---------------------------------------------------------------------

    pub fn extend_instance(
        env: &Env,
        threshold: u32,
        extend_to: u32,
    ) {
        env.storage()
            .instance()
            .extend_ttl(threshold, extend_to);
    }

    /// ---------------------------------------------------------------------
    /// Category helpers
    /// ---------------------------------------------------------------------

    pub fn touch<K>(env: &Env, key: &K, ttl: u32)
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        Self::extend_persistent(env, key, StorageTTL::BUFFER, ttl);
    }

    pub fn touch_vault<K>(env: &Env, key: &K)
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        Self::touch(env, key, StorageTTL::VAULT);
    }

    pub fn touch_user<K>(env: &Env, key: &K)
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        Self::touch(env, key, StorageTTL::USER);
    }

    pub fn touch_reward<K>(env: &Env, key: &K)
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        Self::touch(env, key, StorageTTL::REWARD);
    }

    pub fn touch_lending<K>(env: &Env, key: &K)
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        Self::touch(env, key, StorageTTL::LENDING);
    }

    pub fn touch_borrowing<K>(env: &Env, key: &K)
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        Self::touch(env, key, StorageTTL::BORROWING);
    }

    pub fn touch_streak<K>(env: &Env, key: &K)
    where
        K: soroban_sdk::IntoVal<Env, soroban_sdk::Val> + ?Sized,
    {
        Self::touch(env, key, StorageTTL::STREAK);
    }

    pub fn touch_instance(env: &Env) {
        Self::extend_instance(
            env,
            StorageTTL::BUFFER,
            StorageTTL::INSTANCE,
        );
    }
}