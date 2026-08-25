//! Tests for vault governance: initialization, admin authorization,
//! two-step admin transfer, and unauthorized access prevention.

use soroban_sdk::{
    testutils::Address as _,
    Address, BytesN, Env,
};
use shared::errors::Error;
use vault::{VaultContract, VaultContractClient, VaultId, VaultConfig};

fn ghost_vault_id(env: &Env) -> VaultId {
    VaultId(BytesN::from_array(env, &[0xFFu8; 32]))
}

// ===========================================================================
// Initialization
// ===========================================================================

#[test]
fn test_initialize_sets_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);

    client.initialize(&admin, &streaks, &rewards);
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_initialize_panics_on_double_init() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);

    client.initialize(&admin, &streaks, &rewards);
    let result = client.try_initialize(&admin, &streaks, &rewards);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn test_get_admin_before_init_returns_not_initialized() {
    let env = Env::default();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let result = client.try_get_admin();
    assert_eq!(result, Err(Ok(Error::NotInitialized)));
}

// ===========================================================================
// Configuration requires admin
// ===========================================================================

#[test]
fn test_set_config_requires_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);
    client.initialize(&admin, &streaks, &rewards);

    let non_admin = Address::generate(&env);
    let config = VaultConfig::default();
    let result = client.try_set_config(&non_admin, &config);
    assert_eq!(result, Err(Ok(Error::PermissionDenied)));
}

#[test]
fn test_set_config_succeeds_for_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);
    client.initialize(&admin, &streaks, &rewards);

    let config = VaultConfig { max_vaults_per_user: 5, ..Default::default() };
    client.set_config(&admin, &config);
    assert_eq!(client.get_config().max_vaults_per_user, 5);
}

// ===========================================================================
// Two-step admin transfer
// ===========================================================================

#[test]
fn test_transfer_admin_rejects_self_transfer() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);
    client.initialize(&admin, &streaks, &rewards);

    let result = client.try_transfer_admin(&admin);
    assert_eq!(result, Err(Ok(Error::CannotTransferToSelf)));
}

#[test]
fn test_accept_admin_completes_transfer() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);
    client.initialize(&admin, &streaks, &rewards);

    let new_admin = Address::generate(&env);
    client.transfer_admin(&new_admin);
    client.accept_admin();
    assert_eq!(client.get_admin(), new_admin);
}

#[test]
fn test_accept_admin_without_pending_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);
    client.initialize(&admin, &streaks, &rewards);

    let result = client.try_accept_admin();
    assert_eq!(result, Err(Ok(Error::NoAdminTransferPending)));
}

#[test]
fn test_transfer_admin_clears_pending_after_accept() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);
    client.initialize(&admin, &streaks, &rewards);

    let new_admin = Address::generate(&env);
    let third_admin = Address::generate(&env);

    client.transfer_admin(&new_admin);
    client.accept_admin();

    client.transfer_admin(&third_admin);
    client.accept_admin();
    assert_eq!(client.get_admin(), third_admin);
}

// ===========================================================================
// Vault operations still work after governance changes
// ===========================================================================

#[test]
fn test_vault_create_works_after_init() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);
    client.initialize(&admin, &streaks, &rewards);

    let owner = Address::generate(&env);
    let token = Address::generate(&env);
    let symbol = BytesN::from_array(&env, &[0u8; 32]);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86_400);
    assert_eq!(client.get_vault(&vault_id).lock_period, 86_400);
}

#[test]
fn test_is_locked_for_existing_vault() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);
    client.initialize(&admin, &streaks, &rewards);

    let owner = Address::generate(&env);
    let token = Address::generate(&env);
    let symbol = BytesN::from_array(&env, &[0u8; 32]);
    let vault_id = client.create_vault(&owner, &token, &symbol, &86_400);

    assert!(client.is_locked(&vault_id));
}

// ===========================================================================
// Ghost vault operations still return VaultNotFound
// ===========================================================================

#[test]
fn test_ghost_vault_returns_vault_not_found_after_init() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let client = VaultContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);
    client.initialize(&admin, &streaks, &rewards);

    let vid = ghost_vault_id(&env);
    assert_eq!(client.try_get_vault(&vid), Err(Ok(Error::VaultNotFound)));
    assert_eq!(client.try_get_balance(&vid), Err(Ok(Error::VaultNotFound)));
}
