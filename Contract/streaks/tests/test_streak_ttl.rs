#![cfg(test)]
extern crate std;

use shared::{storage::StorageTTL, types::UserStreak};
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, BytesN, Env, Map, Vec,
};
use streaks::{StreakKey, StreaksContract, StreaksContractClient};

const SHORT_TTL: u32 = 100;

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        min_persistent_entry_ttl: SHORT_TTL,
        max_entry_ttl: StorageTTL::STREAK,
        ..env.ledger().get()
    });

    let contract_id = env.register_contract(None, StreaksContract);
    let vault = Address::generate(&env);
    StreaksContractClient::new(&env, &contract_id).initialize(&vault);
    let user = Address::generate(&env);
    (env, contract_id, user)
}

fn advance_ledgers(env: &Env, ledgers: u32) {
    env.ledger().set(LedgerInfo {
        sequence_number: env.ledger().get().sequence_number + ledgers,
        ..env.ledger().get()
    });
}

#[test]
fn created_and_updated_streak_outlives_default_ttl() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize_streak(&user);
    advance_ledgers(&env, SHORT_TTL + 1);
    assert_eq!(client.get_streak(&user), 1);

    env.ledger().set(LedgerInfo {
        timestamp: 86_400,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    advance_ledgers(&env, SHORT_TTL + 1);
    assert_eq!(client.get_streak(&user), 2);
}

#[test]
fn reading_streak_refreshes_its_ttl() {
    let (env, contract_id, user) = setup();
    let key = BytesN::from_array(&env, &[7u8; 32]);
    let mut streaks = Map::new(&env);
    streaks.set(
        user.clone(),
        UserStreak {
            current_streak: 7,
            longest_streak: 7,
            last_activity_period: 0,
            available_freezes: 3,
        },
    );
    env.as_contract(&contract_id, || {
        env.storage().persistent().set(&key, &streaks);
    });

    advance_ledgers(&env, SHORT_TTL / 2);
    assert_eq!(
        StreaksContractClient::new(&env, &contract_id).get_streak(&user),
        7
    );
    advance_ledgers(&env, SHORT_TTL / 2 + 1);
    assert_eq!(
        StreaksContractClient::new(&env, &contract_id).get_streak(&user),
        7
    );
}

#[test]
fn created_and_read_activity_history_outlives_default_ttl() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    client.initialize_streak(&user);
    advance_ledgers(&env, SHORT_TTL + 1);
    assert!(client.get_activity_history(&user).is_empty());

    let history_key = StreakKey::ActivityHistory(user.clone());
    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&history_key, &Vec::from_array(&env, [42u64]));
    });

    advance_ledgers(&env, SHORT_TTL / 2);
    assert_eq!(client.get_activity_history(&user).get(0), Some(42));
    advance_ledgers(&env, SHORT_TTL / 2 + 1);
    assert_eq!(client.get_activity_history(&user).get(0), Some(42));
}
