//! Authorization helpers for the lending contract.
//!
//! Ensures that protocol-level operations (`borrow`, `repay`, `update_debt`)
//! are only callable by the borrowing contract configured for each pool.

use soroban_sdk::{Address, BytesN, Env};

use shared::errors::Error;

use crate::PoolKey;

/// Verify that the transaction caller is the authorized borrowing contract
/// for `pool_id`.
///
/// Returns `Ok(())` when the transacter matches the stored address.
/// Returns `Err(Error::Unauthorized)` when no borrowing contract has been
/// configured **or** the caller does not match.
pub fn require_borrowing_contract(env: &Env, pool_id: &BytesN<32>) -> Result<(), Error> {
    let key = PoolKey::BorrowingContract(pool_id.clone());
    let configured: Address = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(Error::Unauthorized)?;

    let caller = env.transacter().address();
    if caller != configured {
        return Err(Error::Unauthorized);
    }

    Ok(())
}
