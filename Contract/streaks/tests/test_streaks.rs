#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Env,
};
use streaks::StreaksContractClient;
use shared::types::UserStreak;

#[test]
fn test_streak_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, streaks::WASM);
    let client = StreaksContractClient::new(&env, &contract_id);

    // Initialize with vault as authorized caller
    client.initialize(&vault_id);

    // Initialize a user's streak
    let user = Address::generate(&env);
    client.initialize_streak(&user);

    // Verify streak was created
    let streak = client.get_user_streak(&user);
    assert_eq!(streak.current_streak, 1);
    assert_eq!(streak.longest_streak, 1);
    assert_eq!(streak.available_freezes, 3);
}

#[test]
fn test_consecutive_day_streak_increment() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, streaks::WASM);
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize(&vault_id);
    let user = Address::generate(&env);

    // Day 1
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200, // 2024-01-01
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert_eq!(client.get_streak(&user), 1);

    // Day 2 (consecutive)
    env.ledger().set(LedgerInfo {
        timestamp: 1704153600, // 2024-01-02
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert_eq!(client.get_streak(&user), 2);

    // Day 3 (consecutive)
    env.ledger().set(LedgerInfo {
        timestamp: 1704240000, // 2024-01-03
        ..env.ledger().get()
    });
    client.update_streak(&user);
    let streak = client.get_user_streak(&user);
    assert_eq!(streak.current_streak, 3);
    assert_eq!(streak.longest_streak, 3);
}

#[test]
fn test_same_day_duplicate_prevention() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, streaks::WASM);
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize(&vault_id);
    let user = Address::generate(&env);

    // First deposit on day 1
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    // Second deposit same day should return error
    let result = client.try_update_streak(&user);
    assert!(result.is_err());
}

#[test]
fn test_one_day_missed_uses_freeze() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, streaks::WASM);
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize(&vault_id);
    let user = Address::generate(&env);

    // Day 1
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    let streak = client.get_user_streak(&user);
    assert_eq!(streak.available_freezes, 3);

    // Miss day 2, deposit on day 3
    env.ledger().set(LedgerInfo {
        timestamp: 1704240000, // 2 days later
        ..env.ledger().get()
    });
    client.update_streak(&user);

    // Should have used one freeze
    let streak = client.get_user_streak(&user);
    assert_eq!(streak.available_freezes, 2);
    assert_eq!(streak.current_streak, 2); // Streak continued
}

#[test]
fn test_two_days_missed_resets_streak() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, streaks::WASM);
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize(&vault_id);
    let user = Address::generate(&env);

    // Day 1
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert_eq!(client.get_streak(&user), 1);

    // Miss two full days, deposit on day 4
    env.ledger().set(LedgerInfo {
        timestamp: 1704326400, // 3 days later
        ..env.ledger().get()
    });
    client.update_streak(&user);

    // Streak should reset to 1
    assert_eq!(client.get_streak(&user), 1);
}

#[test]
fn test_manual_freeze_usage() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, streaks::WASM);
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize(&vault_id);
    let user = Address::generate(&env);

    // Initialize streak
    client.initialize_streak(&user);

    // User uses a freeze manually
    client.use_freeze(&user);

    let streak = client.get_user_streak(&user);
    assert_eq!(streak.available_freezes, 2);
}

#[test]
fn test_no_freezes_left() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, streaks::WASM);
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize(&vault_id);
    let user = Address::generate(&env);

    client.initialize_streak(&user);

    // Use all 3 freezes
    client.use_freeze(&user);
    client.use_freeze(&user);
    client.use_freeze(&user);

    // Try to use another - should return error
    let result = client.try_use_freeze(&user);
    assert!(result.is_err());
}

#[test]
fn test_add_freezes_authorized() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, streaks::WASM);
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize(&vault_id);
    let user = Address::generate(&env);

    client.initialize_streak(&user);
    client.add_freezes(&user, &2);

    let streak = client.get_user_streak(&user);
    assert_eq!(streak.available_freezes, 5); // 3 + 2
}

#[test]
fn test_streak_active_check() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, streaks::WASM);
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize(&vault_id);
    let user = Address::generate(&env);

    // Activity today
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert!(client.is_streak_active(&user));

    // Activity yesterday
    env.ledger().set(LedgerInfo {
        timestamp: 1704153600 + 43200, // 36 hours later (still active)
        ..env.ledger().get()
    });
    assert!(client.is_streak_active(&user));

    // Activity more than 48h ago - inactive
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 3 * 86400, // 3 days later
        ..env.ledger().get()
    });
    assert!(!client.is_streak_active(&user));
}

#[test]
fn test_activity_history_stored() {
    let env = Env::default();
    env.mock_all_auths();

    let vault_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, streaks::WASM);
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize(&vault_id);
    let user = Address::generate(&env);

    // Day 1
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    // Day 2
    env.ledger().set(LedgerInfo {
        timestamp: 1704153600,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    let history = client.get_activity_history(&user);
    assert_eq!(history.len(), 2);
    assert_eq!(history.get(0), Some(1704067200));
    assert_eq!(history.get(1), Some(1704153600));
}
