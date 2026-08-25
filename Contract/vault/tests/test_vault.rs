use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, BytesN, Env, Map,
};
use vault::{VaultContract, VaultContractClient, VaultId};

#[contracttype]
#[derive(Clone, Copy, Debug)]
pub enum TokenError {
    InsufficientBalance = 1,
    InvalidAmount = 4,
}

#[contract]
pub struct TokenMock;

#[contractimpl]
impl TokenMock {
    fn balances(env: &Env) -> Map<Address, i128> {
        env.storage()
            .persistent()
            .get(&"balances")
            .unwrap_or_else(|| Map::new(env))
    }

    fn save_balances(env: &Env, balances: Map<Address, i128>) {
        env.storage().persistent().set(&"balances", &balances);
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        if amount <= 0 {
            panic!("{:?}", TokenError::InvalidAmount);
        }
        let mut balances = Self::balances(&env);
        let current = balances.get(to.clone()).unwrap_or(0);
        balances.set(to, current.checked_add(amount).unwrap());
        Self::save_balances(&env, balances);
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        let balances = Self::balances(&env);
        balances.get(id).unwrap_or(0)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        if amount <= 0 {
            panic!("{:?}", TokenError::InvalidAmount);
        }

        let mut balances = Self::balances(&env);
        let from_balance = balances.get(from.clone()).unwrap_or(0);
        if from_balance < amount {
            panic!("{:?}", TokenError::InsufficientBalance);
        }

        let new_from = from_balance.checked_sub(amount).unwrap();
        if new_from == 0 {
            balances.remove(from.clone());
        } else {
            balances.set(from, new_from);
        }

        let to_balance = balances.get(to.clone()).unwrap_or(0);
        balances.set(to, to_balance.checked_add(amount).unwrap());
        Self::save_balances(&env, balances);
    }
}

fn setup() -> (Env, Address, Address, BytesN<32>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, VaultContract);
    let owner = Address::generate(&env);
    let token = env.register_contract(None, TokenMock);
    let symbol = BytesN::from_array(&env, &[0u8; 32]);

    (env, contract_id, owner, symbol, token)
}

fn first_vault_id_bytes(env: &Env) -> BytesN<32> {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&1u64.to_be_bytes());
    BytesN::from_array(env, &bytes)
}

#[test]
fn test_lock_vault_updates_direct_vault_record() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);
    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);
    let vault_id_bytes = first_vault_id_bytes(&env);

    client.unlock_collateral_vault(&vault_id);
    assert_eq!(
        client.get_vault(&vault_id),
        client.get_vault_by_bytes(&vault_id_bytes)
    );

    client.lock_vault(&vault_id);
    assert_eq!(
        client.get_vault(&vault_id),
        client.get_vault_by_bytes(&vault_id_bytes)
    );
}

#[test]
fn test_unlock_collateral_vault_updates_direct_vault_record() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);
    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);
    let vault_id_bytes = first_vault_id_bytes(&env);

    client.unlock_collateral_vault(&vault_id);

    assert_eq!(
        client.get_vault(&vault_id),
        client.get_vault_by_bytes(&vault_id_bytes)
    );
}

#[test]
fn test_transfer_vault_ownership_updates_direct_vault_record() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);
    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);
    let vault_id_bytes = first_vault_id_bytes(&env);
    let new_owner = Address::generate(&env);

    client.transfer_vault_ownership(&vault_id, &new_owner);

    let direct_metadata = client.get_vault(&vault_id);
    assert_eq!(direct_metadata.owner, new_owner);
    assert_eq!(
        direct_metadata,
        client.get_vault_by_bytes(&vault_id_bytes)
    );
}

#[test]
fn test_create_vault() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64;
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.owner, owner);
    assert_eq!(vault.asset.symbol, symbol);
    assert_eq!(vault.asset.token, token);
    assert_eq!(vault.lock_period, lock_period);
    assert_eq!(vault.status as u32, shared::types::VaultStatus::Locked as u32);
}

#[test]
fn test_deposit_flow() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);

    TokenMock::mint(env.clone(), owner.clone(), 1000i128);

    let amount = 500i128;
    client.deposit(&vault_id, &owner, &amount);

    let vault_balance = client.get_balance(&vault_id);
    assert_eq!(vault_balance, 500);
}

#[test]
fn test_withdraw_after_lock_period() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);

    TokenMock::mint(env.clone(), owner.clone(), 1000i128);
    client.deposit(&vault_id, &owner, &1000i128);

    env.ledger().set(LedgerInfo {
        timestamp: 100000,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    let to = Address::generate(&env);
    client.withdraw(&vault_id, &to, &500i128);

    let balance = client.get_balance(&vault_id);
    assert_eq!(balance, 500);
}

#[test]
#[should_panic(expected = "VaultLocked")]
fn test_withdraw_before_lock_period() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);

    TokenMock::mint(env.clone(), owner.clone(), 1000i128);
    client.deposit(&vault_id, &owner, &1000i128);

    let to = Address::generate(&env);
    client.withdraw(&vault_id, &to, &500i128);
}

#[test]
#[should_panic(expected = "InvalidAmount")]
fn test_deposit_zero_amount() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);

    TokenMock::mint(env.clone(), owner.clone(), 1000i128);
    client.deposit(&vault_id, &owner, &0i128);
}

#[test]
#[should_panic(expected = "InsufficientBalance")]
fn test_withdraw_insufficient_balance() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);

    TokenMock::mint(env.clone(), owner.clone(), 1000i128);
    client.deposit(&vault_id, &owner, &100i128);

    env.ledger().set(LedgerInfo {
        timestamp: 100000,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    let to = Address::generate(&env);
    client.withdraw(&vault_id, &to, &200i128);
}

#[test]
#[should_panic(expected = "InvalidLockPeriod")]
fn test_create_vault_invalid_lock_period() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 0u64;
    client.create_vault(&owner, &token, &symbol, &lock_period);
}

#[test]
fn test_get_lock_period() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64;
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    let retrieved = client.get_lock_period(&vault_id);
    assert_eq!(retrieved, lock_period);
}

#[test]
fn test_get_unlock_time() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64;
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    let unlock_time = client.get_unlock_time(&vault_id);
    assert!(unlock_time >= lock_period);
}

#[test]
fn test_is_locked() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64;
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    // Should be locked initially
    assert!(client.is_locked(&vault_id));

    env.ledger().set(LedgerInfo {
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
#[should_panic(expected = "VaultLocked")]
fn test_unlock_vault_before_unlock_time() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);
    client.unlock_vault(&vault_id);
}

#[test]
fn test_unlock_vault_at_exact_unlock_time() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);

    let unlock_time = client.get_unlock_time(&vault_id);
    env.ledger().set(LedgerInfo {
        timestamp: unlock_time,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    client.unlock_vault(&vault_id);
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status as u32, shared::types::VaultStatus::Unlocked as u32);
    assert!(!client.is_locked(&vault_id));
}

#[test]
fn test_unlock_vault_after_unlock_time() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);

    let unlock_time = client.get_unlock_time(&vault_id);
    env.ledger().set(LedgerInfo {
        timestamp: unlock_time + 1,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    client.unlock_vault(&vault_id);
    let vault = client.get_vault(&vault_id);
    assert_eq!(vault.status as u32, shared::types::VaultStatus::Unlocked as u32);
    assert!(!client.is_locked(&vault_id));
}

#[test]
#[should_panic(expected = "VaultAlreadyUnlocked")]
fn test_unlock_vault_only_once() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);

    let unlock_time = client.get_unlock_time(&vault_id);
    env.ledger().set(LedgerInfo {
        timestamp: unlock_time,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    client.unlock_vault(&vault_id);
    client.unlock_vault(&vault_id);
}

#[test]
fn test_get_nonexistent_vault() {
    let (env, contract_id, _owner, _symbol, _token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let fake_vault_id = VaultId(BytesN::from_array(&env, &[1u8; 32]));
    let result = client.try_get_vault(&fake_vault_id);
    assert_eq!(result, Err(Ok(shared::errors::Error::VaultNotFound)));
}

#[test]
fn test_exact_unlock_time_withdrawal() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64;
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    TokenMock::mint(env.clone(), owner.clone(), 1000i128);
    client.deposit(&vault_id, &owner, &1000i128);

    let unlock_time = client.get_unlock_time(&vault_id);

    env.ledger().set(LedgerInfo {
        timestamp: unlock_time,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    let to = Address::generate(&env);
    client.withdraw(&vault_id, &to, &1000i128);

    let balance = client.get_balance(&vault_id);
    assert_eq!(balance, 0);
}

#[test]
fn test_multiple_deposits() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);

    TokenMock::mint(env.clone(), owner.clone(), 3000i128);

    client.deposit(&vault_id, &owner, &1000i128);
    let balance = client.get_balance(&vault_id);
    assert_eq!(balance, 1000);

    client.deposit(&vault_id, &owner, &1000i128);
    let balance = client.get_balance(&vault_id);
    assert_eq!(balance, 2000);

    client.deposit(&vault_id, &owner, &1000i128);
    let balance = client.get_balance(&vault_id);
    assert_eq!(balance, 3000);
}

#[test]
fn test_withdraw_exact_balance_no_underflow() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let vault_id = client.create_vault(&owner, &token, &symbol, &86400u64);

    TokenMock::mint(env.clone(), owner.clone(), 100i128);
    client.deposit(&vault_id, &owner, &100i128);

    env.ledger().set(LedgerInfo {
        timestamp: 100000,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    let to = Address::generate(&env);
    client.withdraw(&vault_id, &to, &100i128);

    let balance = client.get_balance(&vault_id);
    assert_eq!(balance, 0);
}

#[test]
fn test_withdraw_at_exact_unlock_time() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64;
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    TokenMock::mint(env.clone(), owner.clone(), 1000i128);
    client.deposit(&vault_id, &owner, &1000i128);

    let unlock_time = client.get_unlock_time(&vault_id);

    env.ledger().set(LedgerInfo {
        timestamp: unlock_time,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    assert!(
        !client.is_locked(&vault_id),
        "Vault should be unlocked at exactly unlock_time"
    );

    let to = Address::generate(&env);
    client.withdraw(&vault_id, &to, &500i128);

    let balance = client.get_balance(&vault_id);
    assert_eq!(balance, 500, "Balance should be 500 after withdrawing 500 from 1000");
}
