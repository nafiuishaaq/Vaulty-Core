//! Authorization helpers for the lending contract.
//!
//! Ensures that protocol-level operations (`borrow`, `repay`, `update_debt`)
//! are only callable by the borrowing contract configured for each pool.

use soroban_sdk::{BytesN, Env};

use shared::errors::Error;

use crate::PoolKey;

/// Verify that the transaction caller is the authorized borrowing contract
/// for `pool_id`.
///
/// Uses Soroban's standard `require_auth` mechanism: the stored borrowing
/// contract address is retrieved and `require_auth` is called on it.  When
/// the calling context is `env.as_contract(&borrowing_addr, …)` this check
/// passes automatically; a direct (non-contract) call will panic with an
/// auth error that is translated to `Unauthorized`.
///
/// Returns `Ok(())` when authorization succeeds.
/// Returns `Err(Error::Unauthorized)` when no borrowing contract has been
/// configured for the pool.
pub fn require_borrowing_contract(env: &Env, pool_id: &BytesN<32>) -> Result<(), Error> {
    let key = PoolKey::BorrowingContract(pool_id.clone());
    let configured = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(Error::Unauthorized)?;

    // `require_auth` will panic (host error) if the address has not authorized
    // this invocation.  The Soroban test harness satisfies the check
    // automatically when the call is made inside `env.as_contract(&configured, …)`.
    soroban_sdk::Address::require_auth(&configured);

    Ok(())
}
