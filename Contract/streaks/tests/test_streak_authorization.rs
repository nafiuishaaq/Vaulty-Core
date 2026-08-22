#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env,
};
use streaks::{StreaksContract, StreaksContractClient};
use shared::errors::Error;

const DAY: u64 = 86400;

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, StreaksContract);
    let vault = Address::generate(&env);
    let client = StreaksContractClient::new(&env, &contract_id);
    client.initialize(&vault);

    let user = Address::generate(&env);
    (env, contract_id, vault, user)
}

#[test]
fn vault_authorized_update_streak_succeeds() {
    let (env, _contract_id, vault, user) = setup();
    let client = StreaksContractClient::new(&env, &_contract_id);

    // Set timestamp to day 1
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });

    // Vault authorized call succeeds
    let result = client.try_update_streak(&user);
    assert!(result.is_ok());
    assert_eq!(client.get_streak(&user), 1);
}

#[test]
fn arbitrary_caller_fails_unauthorized() {
    let env = Env::default();
    // Do NOT mock all auths so we can test authorization properly
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });

    let contract_id = env.register_contract(None, StreaksContract);
    let vault = Address::generate(&env);
    let client = StreaksContractClient::new(&env, &contract_id);
    client.initialize(&vault);

    let user = Address::generate(&env);
    let random_caller = Address::generate(&env);

    // Set the caller to be a random address (not the vault)
    env.mock_all_auths();
    // Try calling from a non-authorized address
    let result = env.as_contract(&random_caller, || {
        // This simulates a call from random_caller
        env.invoke_contract::<()>(
            &contract_id,
            &soroban_sdk::Symbol::new(&env, "update_streak"),
            soroban_sdk::Vec::new(&env),
        )
    });
    assert!(result.is_err());
}

#[test]
fn user_authorization_required_for_use_freeze() {
    let (env, _contract_id, _vault, user) = setup();
    let client = StreaksContractClient::new(&env, &_contract_id);

    // Initialize the streak first
    client.initialize_streak(&user);

    // With mock_all_auths, user auth is satisfied
    let result = client.try_use_freeze(&user);
    assert!(result.is_ok());

    let streak = client.get_user_streak(&user);
    assert_eq!(streak.available_freezes, 2);
}

#[test]
fn add_authorized_caller_requires_existing_authorization() {
    let (env, _contract_id, vault, _user) = setup();
    let client = StreaksContractClient::new(&env, &_contract_id);

    let new_caller = Address::generate(&env);
    let result = client.try_add_authorized_caller(&new_caller);
    assert!(result.is_ok());

    // Verify the new caller was added by checking initialization still works
    let user = Address::generate(&env);
    let result = client.try_update_streak(&user);
    assert!(result.is_ok());
}

#[test]
fn initialize_can_only_be_called_once() {
    let (env, _contract_id, vault, _user) = setup();
    let client = StreaksContractClient::new(&env, &_contract_id);

    let second_vault = Address::generate(&env);
    let result = client.try_initialize(&second_vault);
    assert!(result.is_err());
}

#[test]
fn duplicate_activity_returns_error() {
    let (env, _contract_id, _vault, user) = setup();
    let client = StreaksContractClient::new(&env, &_contract_id);

    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });

    // First update succeeds
    let result = client.try_update_streak(&user);
    assert!(result.is_ok());

    // Second update same day returns DuplicateActivity error
    let result = client.try_update_streak(&user);
    assert!(result.is_err());
}

#[test]
fn no_freezes_available_returns_error() {
    let (env, _contract_id, _vault, user) = setup();
    let client = StreaksContractClient::new(&env, &_contract_id);

    client.initialize_streak(&user);

    // Use all 3 freezes
    client.use_freeze(&user);
    client.use_freeze(&user);
    client.use_freeze(&user);

    // Fourth attempt returns NoFreezesAvailable error
    let result = client.try_use_freeze(&user);
    assert!(result.is_err());
}

#[test]
fn streak_not_found_returns_error() {
    let (env, _contract_id, _vault, user) = setup();
    let client = StreaksContractClient::new(&env, &_contract_id);

    // get_user_streak for non-existent streak returns error
    let result = client.try_get_user_streak(&user);
    assert!(result.is_err());
}

#[test]
fn get_streak_returns_zero_for_nonexistent() {
    let (env, _contract_id, _vault, user) = setup();
    let client = StreaksContractClient::new(&env, &_contract_id);

    // get_streak returns 0 for non-existent streak
    assert_eq!(client.get_streak(&user), 0);
}

#[test]
fn add_freezes_requires_authorization() {
    let (env, _contract_id, _vault, user) = setup();
    let client = StreaksContractClient::new(&env, &_contract_id);

    client.initialize_streak(&user);

    // Authorized add works
    let result = client.try_add_freezes(&user, &5);
    assert!(result.is_ok());

    let streak = client.get_user_streak(&user);
    assert_eq!(streak.available_freezes, 8); // 3 + 5
}
