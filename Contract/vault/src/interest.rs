use soroban_sdk::Env;
use shared::{
    errors::Error,
    storage::StorageHelper,
    types::VaultMetadata,
    utils::{FixedMath, SafeMath, TimeHelper},
};

use crate::state::{VaultConfig, VaultId, VaultKey};

/// Number of seconds in a 365-day year used for annualized rate calculations
pub const SECONDS_PER_YEAR: i128 = 31_536_000;

/// Pure calculation helper for interest accrued over a time period using fixed-point math.
///
/// # Arguments
/// * `balance` - Current principal balance
/// * `rate_bps` - Interest rate in basis points (e.g. 500 = 5.00%)
/// * `elapsed_seconds` - Time elapsed in seconds since the previous accrual
///
/// # Returns
/// The calculated accrued interest amount, or `Error::Overflow` on arithmetic overflow.
pub fn calculate_interest(balance: i128, rate_bps: i128, elapsed_seconds: u64) -> Result<i128, Error> {
    if balance <= 0 || rate_bps <= 0 || elapsed_seconds == 0 {
        return Ok(0);
    }

    let rate_per_second = FixedMath::basis_points_to_fixed(rate_bps) / SECONDS_PER_YEAR;
    let interest = FixedMath::calculate_interest(balance, rate_per_second, elapsed_seconds as i128)
        .ok_or(Error::Overflow)?;

    Ok(interest)
}

/// Accrues interest for a vault since its previous accrual timestamp.
///
/// Key Safety Guarantees:
/// 1. Accrues interest strictly for `now - last_accrued_at`.
/// 2. Updates `last_accrued_at` to `now`, guaranteeing sequential periods do not overlap.
/// 3. Repeated invocations at the same timestamp produce zero additional yield (`elapsed == 0`).
/// 4. Accrued yield is stored purely as informational data in `VaultKey::VaultInterest` and
///    is NOT added to withdrawable balances (`VaultKey::Balance`) without an approved on-chain
///    funding source.
///
/// # Arguments
/// * `env` - Soroban environment reference
/// * `vault_id` - The vault ID to accrue interest for
///
/// # Returns
/// The newly accrued interest for this period (`i128`).
pub fn accrue_vault_interest(env: &Env, vault_id: &VaultId) -> Result<i128, Error> {
    let metadata: VaultMetadata = env
        .storage()
        .persistent()
        .get(&VaultKey::Vault(vault_id.clone()))
        .ok_or(Error::VaultNotFound)?;
    StorageHelper::touch_vault(env, &VaultKey::Vault(vault_id.clone()));

    let config: VaultConfig = env
        .storage()
        .persistent()
        .get(&VaultKey::VaultConfig)
        .unwrap_or_default();
    StorageHelper::touch_vault(env, &VaultKey::VaultConfig);

    let now = TimeHelper::now(env);

    // Retrieve previous accrual timestamp (fall back to creation timestamp if never accrued)
    let last_accrual: u64 = env
        .storage()
        .persistent()
        .get(&VaultKey::LastAccrual(vault_id.clone()))
        .unwrap_or(metadata.created_at);

    let elapsed = now.saturating_sub(last_accrual);

    // Update the last accrual timestamp to current ledger time
    env.storage()
        .persistent()
        .set(&VaultKey::LastAccrual(vault_id.clone()), &now);
    StorageHelper::touch_vault(env, &VaultKey::LastAccrual(vault_id.clone()));

    // If zero time elapsed since last accrual, no additional interest is accrued
    if elapsed == 0 {
        return Ok(0);
    }

    let balance: i128 = env
        .storage()
        .persistent()
        .get(&VaultKey::Balance(vault_id.clone()))
        .unwrap_or(0);
    StorageHelper::touch_vault(env, &VaultKey::Balance(vault_id.clone()));

    if balance <= 0 || config.interest_rate <= 0 {
        return Ok(0);
    }

    let interest = calculate_interest(balance, config.interest_rate, elapsed)?;

    if interest > 0 {
        let prev_interest: i128 = env
            .storage()
            .persistent()
            .get(&VaultKey::VaultInterest(vault_id.clone()))
            .unwrap_or(0);

        let total_interest = SafeMath::add(prev_interest, interest).ok_or(Error::Overflow)?;

        // Informational tracking only: do NOT inflate withdrawable balance
        env.storage()
            .persistent()
            .set(&VaultKey::VaultInterest(vault_id.clone()), &total_interest);
        StorageHelper::touch_vault(env, &VaultKey::VaultInterest(vault_id.clone()));
    }

    Ok(interest)
}
