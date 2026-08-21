//! Tests proving that public vault operations return `Error::VaultNotFound`
//! when called with a vault ID that was never created, rather than trapping
//! execution with a panic.

use soroban_sdk::{
    testutils::Address as _,
    Address, BytesN, Env,
};
use shared::errors::Error;
use vault::{VaultContract, VaultId};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Register the vault contract and return `(env, contract_id)`.
fn setup_env() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    (env, contract_id)
}

/// Build a `VaultId` that was never inserted into any contract storage.
fn ghost_vault_id(env: &Env) -> VaultId {
    // Use all-0xFF bytes so this ID is extremely unlikely to match anything.
    VaultId(BytesN::from_array(env, &[0xFFu8; 32]))
}

// ---------------------------------------------------------------------------
// get_vault
// ---------------------------------------------------------------------------

/// `get_vault` must return `Err(VaultNotFound)` for an unknown vault ID.
#[test]
fn test_get_vault_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let result = VaultContract::get_vault(&env, &contract_id, &ghost_vault_id(&env));
    assert_eq!(result, Err(Error::VaultNotFound));
}

// ---------------------------------------------------------------------------
// get_balance
// ---------------------------------------------------------------------------

/// `get_balance` must return `Err(VaultNotFound)` for an unknown vault ID.
#[test]
fn test_get_balance_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let result = VaultContract::get_balance(&env, &contract_id, &ghost_vault_id(&env));
    assert_eq!(result, Err(Error::VaultNotFound));
}

// ---------------------------------------------------------------------------
// get_lock_period / get_unlock_time
// ---------------------------------------------------------------------------

/// `get_lock_period` must return `Err(VaultNotFound)` for an unknown vault ID.
#[test]
fn test_get_lock_period_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let result = VaultContract::get_lock_period(&env, &contract_id, &ghost_vault_id(&env));
    assert_eq!(result, Err(Error::VaultNotFound));
}

/// `get_unlock_time` must return `Err(VaultNotFound)` for an unknown vault ID.
#[test]
fn test_get_unlock_time_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let result = VaultContract::get_unlock_time(&env, &contract_id, &ghost_vault_id(&env));
    assert_eq!(result, Err(Error::VaultNotFound));
}

// ---------------------------------------------------------------------------
// is_locked
// ---------------------------------------------------------------------------

/// `is_locked` must return `Err(VaultNotFound)` for an unknown vault ID.
#[test]
fn test_is_locked_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let result = VaultContract::is_locked(&env, &contract_id, &ghost_vault_id(&env));
    assert_eq!(result, Err(Error::VaultNotFound));
}

// ---------------------------------------------------------------------------
// withdraw
// ---------------------------------------------------------------------------

/// `withdraw` must return `Err(VaultNotFound)` for an unknown vault ID.
#[test]
fn test_withdraw_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let recipient = Address::generate(&env);
    let result = VaultContract::withdraw(
        &env,
        &contract_id,
        &ghost_vault_id(&env),
        &recipient,
        &100i128,
    );
    assert_eq!(result, Err(Error::VaultNotFound));
}

// ---------------------------------------------------------------------------
// lock_vault
// ---------------------------------------------------------------------------

/// `lock_vault` must return `Err(VaultNotFound)` for an unknown vault ID.
#[test]
fn test_lock_vault_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let result = VaultContract::lock_vault(&env, &contract_id, &ghost_vault_id(&env));
    assert_eq!(result, Err(Error::VaultNotFound));
}

// ---------------------------------------------------------------------------
// unlock_vault
// ---------------------------------------------------------------------------

/// `unlock_vault` must return `Err(VaultNotFound)` for an unknown vault ID.
#[test]
fn test_unlock_vault_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let result = VaultContract::unlock_vault(&env, &contract_id, &ghost_vault_id(&env));
    assert_eq!(result, Err(Error::VaultNotFound));
}

// ---------------------------------------------------------------------------
// unlock_collateral_vault
// ---------------------------------------------------------------------------

/// `unlock_collateral_vault` must return `Err(VaultNotFound)` for an unknown
/// vault ID.
#[test]
fn test_unlock_collateral_vault_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let result =
        VaultContract::unlock_collateral_vault(&env, &contract_id, &ghost_vault_id(&env));
    assert_eq!(result, Err(Error::VaultNotFound));
}

// ---------------------------------------------------------------------------
// transfer_vault_ownership
// ---------------------------------------------------------------------------

/// `transfer_vault_ownership` must return `Err(VaultNotFound)` for an unknown
/// vault ID.
#[test]
fn test_transfer_vault_ownership_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let new_owner = Address::generate(&env);
    let result = VaultContract::transfer_vault_ownership(
        &env,
        &contract_id,
        &ghost_vault_id(&env),
        &new_owner,
    );
    assert_eq!(result, Err(Error::VaultNotFound));
}

// ---------------------------------------------------------------------------
// get_vault_by_bytes
// ---------------------------------------------------------------------------

/// `get_vault_by_bytes` must return `Err(VaultNotFound)` for an unknown vault
/// ID represented as raw bytes (cross-contract lookup path).
#[test]
fn test_get_vault_by_bytes_unknown_id_returns_vault_not_found() {
    let (env, contract_id) = setup_env();
    let ghost_bytes = BytesN::from_array(&env, &[0xFFu8; 32]);
    let result = VaultContract::get_vault_by_bytes(&env, &contract_id, &ghost_bytes);
    assert_eq!(result, Err(Error::VaultNotFound));
}

// ---------------------------------------------------------------------------
// Valid operations are unaffected
// ---------------------------------------------------------------------------

/// Sanity check: a real vault created with a valid lock period can be looked
/// up successfully, and the returned metadata matches what was supplied at
/// creation time.  This ensures the VaultNotFound fix has not broken the
/// happy path.
#[test]
fn test_valid_vault_operations_unaffected() {
    let (env, contract_id) = setup_env();

    let owner = Address::generate(&env);
    let token = Address::generate(&env);
    let symbol = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 86_400u64; // 1 day

    // Create a real vault.
    let vault_id = VaultContract::create_vault(
        &env,
        &contract_id,
        &owner,
        &token,
        &symbol,
        &lock_period,
    )
    .expect("create_vault should succeed");

    // All lookup operations must succeed for a vault that really exists.
    let metadata = VaultContract::get_vault(&env, &contract_id, &vault_id)
        .expect("get_vault should succeed for a real vault");
    assert_eq!(metadata.lock_period, lock_period);

    let balance = VaultContract::get_balance(&env, &contract_id, &vault_id)
        .expect("get_balance should succeed for a real vault");
    assert_eq!(balance, 0);

    let lp = VaultContract::get_lock_period(&env, &contract_id, &vault_id)
        .expect("get_lock_period should succeed for a real vault");
    assert_eq!(lp, lock_period);

    let ut = VaultContract::get_unlock_time(&env, &contract_id, &vault_id)
        .expect("get_unlock_time should succeed for a real vault");
    assert!(ut > 0);

    let locked = VaultContract::is_locked(&env, &contract_id, &vault_id)
        .expect("is_locked should succeed for a real vault");
    assert!(locked, "Newly created vault should be locked");
}
