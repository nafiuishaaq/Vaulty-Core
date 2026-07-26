use soroban_sdk::{
    testutils::{Address as _, BytesN as _, Ledger},
    Address, BytesN, Env,
};
use vault::VaultContractClient;

const WASM: &[u8] = vault::WASM;

#[test]
fn test_create_vault() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 86400u64; // 1 day

    let vault_id = client.create_vault(&owner, &asset_code, &Some(owner.clone()), &lock_period);

    // Verify vault was created
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.owner, owner);
    assert_eq!(vault.lock_period, lock_period);
    assert_eq!(vault.status, 1); // Locked
}

#[test]
fn test_deposit_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 86400u64;

    let vault_id = client.create_vault(&owner, &asset_code, &Some(owner.clone()), &lock_period);

    // Deposit funds
    let depositor = Address::generate(&env);
    let amount = 1000i128;
    client.deposit(&vault_id, &depositor, &amount);

    // Verify balance
    let balance = client.get_balance(&vault_id);
    assert_eq!(balance, amount);
}

#[test]
fn test_withdraw_after_lock_period() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 86400u64;

    let vault_id = client.create_vault(&owner, &asset_code, &Some(owner.clone()), &lock_period);

    // Deposit funds
    client.deposit(&vault_id, &owner, &1000i128);

    // Advance time past lock period
    env.ledger().set(soroban_sdk::LedgerInfo {
        timestamp: 100000,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    // Withdraw
    let to = Address::generate(&env);
    client.withdraw(&vault_id, &to, &500i128);

    // Verify balance
    let balance = client.get_balance(&vault_id);
    assert_eq!(balance, 500);
}

#[test]
#[should_panic(expected = "Vault is locked")]
fn test_withdraw_before_lock_period() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 86400u64;

    let vault_id = client.create_vault(&owner, &asset_code, &Some(owner.clone()), &lock_period);

    // Deposit funds
    client.deposit(&vault_id, &owner, &1000i128);

    // Try to withdraw before lock period expires
    let to = Address::generate(&env);
    client.withdraw(&vault_id, &to, &500i128);
}

#[test]
#[should_panic(expected = "Invalid amount")]
fn test_deposit_zero_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 86400u64;

    let vault_id = client.create_vault(&owner, &asset_code, &Some(owner.clone()), &lock_period);

    // Try to deposit zero
    client.deposit(&vault_id, &owner, &0i128);
}

#[test]
#[should_panic(expected = "Insufficient balance")]
fn test_withdraw_insufficient_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 86400u64;

    let vault_id = client.create_vault(&owner, &asset_code, &Some(owner.clone()), &lock_period);

    // Deposit small amount
    client.deposit(&vault_id, &owner, &100i128);

    // Advance time past lock period
    env.ledger().set(soroban_sdk::LedgerInfo {
        timestamp: 100000,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    // Try to withdraw more than balance
    let to = Address::generate(&env);
    client.withdraw(&vault_id, &to, &200i128);
}

#[test]
#[should_panic(expected = "Invalid lock period")]
fn test_create_vault_invalid_lock_period() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 0u64; // Invalid: must be at least 1

    client.create_vault(&owner, &asset_code, &Some(owner.clone()), &lock_period);
}

#[test]
fn test_get_lock_period() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 86400u64;

    let vault_id = client.create_vault(&owner, &asset_code, &Some(owner.clone()), &lock_period);

    let retrieved_lock_period = client.get_lock_period(&vault_id);
    assert_eq!(retrieved_lock_period, lock_period);
}

#[test]
fn test_get_unlock_time() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 86400u64;

    let vault_id = client.create_vault(&owner, &asset_code, &Some(owner.clone()), &lock_period);

    let unlock_time = client.get_unlock_time(&vault_id);
    assert!(unlock_time > 0);
}

#[test]
fn test_is_locked() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let lock_period = 86400u64;

    let vault_id = client.create_vault(&owner, &asset_code, &Some(owner.clone()), &lock_period);

    // Should be locked initially
    assert!(client.is_locked(&vault_id));

    // Advance time past lock period
    env.ledger().set(soroban_sdk::LedgerInfo {
        timestamp: 100000,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    // Should be unlocked after lock period
    assert!(!client.is_locked(&vault_id));
}

#[test]
#[should_panic(expected = "Vault not found")]
fn test_get_nonexistent_vault() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract_wasm(None, vault::WASM);
    let client = VaultContractClient::new(&env, &contract_id);

    // Create a vault to get a valid vault_id structure
    let owner = Address::generate(&env);
    let asset_code = BytesN::from_array(&env, &[0u8; 32]);
    let vault_id = client.create_vault(&owner, &asset_code, &Some(owner.clone()), &86400u64);
    
    // Create a fresh environment where this vault_id doesn't exist
    let env2 = Env::default();
    env2.mock_all_auths();
    let contract_id2 = env2.register_contract_wasm(None, vault::WASM);
    let client2 = VaultContractClient::new(&env2, &contract_id2);
    
    // Try to get the vault from the fresh environment - it won't exist
    client2.get_vault(&vault_id);
}