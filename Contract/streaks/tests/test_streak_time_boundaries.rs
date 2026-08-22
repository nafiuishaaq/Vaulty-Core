#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Env,
};
use streaks::{StreaksContract, StreaksContractClient};

const DAY: u64 = 86400;

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, StreaksContract);
    let vault = Address::generate(&env);
    StreaksContractClient::new(&env, &contract_id).initialize(&vault);

    let user = Address::generate(&env);
    (env, contract_id, user)
}

#[test]
fn utc_midnight_boundary_deterministic() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    // Activity at 23:59:59 UTC
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 86399,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert_eq!(client.get_streak(&user), 1);

    // Activity at 00:00:00 UTC next day (consecutive)
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + DAY,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert_eq!(client.get_streak(&user), 2);
}

#[test]
fn activity_within_same_utc_day_rejected() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    // Activity at 06:00 UTC
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 21600,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert_eq!(client.get_streak(&user), 1);

    // Activity at 18:00 UTC same day (should fail)
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 64800,
        ..env.ledger().get()
    });
    let result = client.try_update_streak(&user);
    assert!(result.is_err());

    // Streak should remain unchanged
    assert_eq!(client.get_streak(&user), 1);
}

#[test]
fn consecutive_days_across_multiple_periods() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    for day in 0..7u64 {
        env.ledger().set(LedgerInfo {
            timestamp: 1704067200 + day * DAY,
            ..env.ledger().get()
        });
        client.update_streak(&user);
    }

    assert_eq!(client.get_streak(&user), 7);
    let streak = client.get_user_streak(&user);
    assert_eq!(streak.longest_streak, 7);
}

#[test]
fn one_day_gap_uses_freeze() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    // Day 1
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    // Day 3 (skipping day 2)
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 2 * DAY,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    let streak = client.get_user_streak(&user);
    assert_eq!(streak.current_streak, 2);
    assert_eq!(streak.available_freezes, 2);
}

#[test]
fn multiple_freeze_usage() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    // Build streak
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    // Skip day 2, use freeze on day 3
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 2 * DAY,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    // Skip day 4, use freeze on day 5
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 4 * DAY,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    // Skip day 6, use freeze on day 7
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 6 * DAY,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    let streak = client.get_user_streak(&user);
    assert_eq!(streak.current_streak, 4);
    assert_eq!(streak.available_freezes, 0);
}

#[test]
fn three_day_gap_resets_streak() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    // Build a streak
    for day in 0..5u64 {
        env.ledger().set(LedgerInfo {
            timestamp: 1704067200 + day * DAY,
            ..env.ledger().get()
        });
        client.update_streak(&user);
    }
    assert_eq!(client.get_streak(&user), 5);

    // Skip 3 days
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 8 * DAY,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    // Streak resets to 1
    assert_eq!(client.get_streak(&user), 1);
    let streak = client.get_user_streak(&user);
    assert_eq!(streak.longest_streak, 5);
}

#[test]
fn activity_history_records_all_periods() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    for day in 0..5u64 {
        env.ledger().set(LedgerInfo {
            timestamp: 1704067200 + day * DAY,
            ..env.ledger().get()
        });
        client.update_streak(&user);
    }

    let history = client.get_activity_history(&user);
    assert_eq!(history.len(), 5);

    for (i, expected) in (0..5u64).enumerate() {
        assert_eq!(history.get(i as u32), Some(1704067200 + expected * DAY));
    }
}

#[test]
fn streak_inactive_after_48_hours() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert!(client.is_streak_active(&user));

    // 48 hours later - still active
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 2 * DAY,
        ..env.ledger().get()
    });
    assert!(client.is_streak_active(&user));

    // 49 hours later - inactive
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 2 * DAY + 3601,
        ..env.ledger().get()
    });
    assert!(!client.is_streak_active(&user));
}

#[test]
fn no_freezes_causes_reset_on_single_miss() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    // Use all freezes manually
    client.initialize_streak(&user);
    client.use_freeze(&user);
    client.use_freeze(&user);
    client.use_freeze(&user);

    // Start streak
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert_eq!(client.get_streak(&user), 2); // Continued from initialize_streak

    // Skip one day
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 2 * DAY,
        ..env.ledger().get()
    });
    client.update_streak(&user);

    // Should reset to 1 since no freezes
    assert_eq!(client.get_streak(&user), 1);
}

#[test]
fn large_gap_many_days_resets() {
    let (env, contract_id, user) = setup();
    let client = StreaksContractClient::new(&env, &contract_id);

    env.ledger().set(LedgerInfo {
        timestamp: 1704067200,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert_eq!(client.get_streak(&user), 1);

    // Jump 30 days
    env.ledger().set(LedgerInfo {
        timestamp: 1704067200 + 30 * DAY,
        ..env.ledger().get()
    });
    client.update_streak(&user);
    assert_eq!(client.get_streak(&user), 1);
}
