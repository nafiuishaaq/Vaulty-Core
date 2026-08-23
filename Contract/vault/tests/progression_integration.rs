#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};
use streaks::StreaksContractClient;
use rewards::RewardsContractClient;
use vault::VaultContractClient;

fn setup_integration_test() -> (Env, Address, Address, VaultContractClient, StreaksContractClient, RewardsContractClient) {
    let env = Env::default();
    env.budget().reset_unlimited();

    // Create test accounts
    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // Deploy all contracts
    let streaks_id = env.register_contract(None, streaks::StreaksContract);
    let rewards_id = env.register_contract(None, rewards::RewardsContract);
    let vault_id = env.register_contract(None, vault::VaultContract);

    // Create clients
    let mut streaks = StreaksContractClient::new(&env, &streaks_id);
    let mut rewards = RewardsContractClient::new(&env, &rewards_id);
    let vault = VaultContractClient::new(&env, &vault_id);

    // Initialize all contracts in correct order
    streaks.initialize(&vault_id);
    
    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    rewards.initialize(&admin, &reward_asset, &streaks_id);
    
    vault.initialize(&streaks_id, &rewards_id);

    // Register vault with streaks to add it as authorized caller
    vault.register_with_streaks();

    // Fund the rewards pool as admin
    admin.require_auth();
    rewards.fund_rewards_pool(&10000_0000000);

    (env, admin, user, vault, streaks, rewards)
}

#[test]
fn test_full_progression_flow() {
    let (env, _admin, user, vault, streaks, rewards) = setup_integration_test();

    // Create a vault for the user
    let asset_code: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
    let token_issuer = Address::generate(&env);
    let vault_id_val = vault.create_vault(
        &user,
        &token_issuer,
        &asset_code,
        &86400, // 1 day lock
    );

    // DAY 1: First deposit creates streak
    env.ledger().set_timestamp(1704067200); // 2024-01-01 00:00:00 UTC
    vault.deposit(&vault_id_val, &user, &100);

    // Verify streak is 1
    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 1);

    // DAY 2: Second consecutive deposit increments streak to 2
    env.ledger().set_timestamp(1704153600); // 2024-01-02 00:00:00 UTC
    vault.deposit(&vault_id_val, &user, &100);

    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 2);

    // Continue depositing daily to reach 7-day milestone
    for day in 3..=7 {
        let timestamp = 1704067200 + (day as u64 - 1) * 86400;
        env.ledger().set_timestamp(timestamp);
        vault.deposit(&vault_id_val, &user, &100);
    }

    // After day 7, streak should be 7
    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 7);

    // Check that pending rewards include the 7-day milestone
    let pending = rewards.get_pending_rewards(&user);
    assert_eq!(pending, 10_0000000); // 10 tokens from 7-day milestone

    // Claim the rewards
    let claimed = rewards.claim_rewards(&user);
    assert_eq!(claimed, 10_0000000);

    // Pending should now be 0
    let pending = rewards.get_pending_rewards(&user);
    assert_eq!(pending, 0);

    // Test freeze usage
    // Miss one day, use a freeze
    env.ledger().set_timestamp(1704067200 + 9 * 86400); // Skip day 8, go to day 9
    let user_streak = streaks.get_user_streak(&user);
    assert_eq!(user_streak.available_freezes, 3); // Started with 3

    vault.deposit(&vault_id_val, &user, &100);
    let user_streak = streaks.get_user_streak(&user);
    assert_eq!(user_streak.available_freezes, 2); // Used one freeze
    assert_eq!(user_streak.current_streak, 8); // Streak continued

    // Miss two days - streak resets
    env.ledger().set_timestamp(1704067200 + 12 * 86400); // Skip 2 full days
    vault.deposit(&vault_id_val, &user, &100);
    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 1); // Streak reset to 1
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_unauthorized_streaks_caller() {
    let env = Env::default();
    let streaks_id = env.register_contract(None, streaks::StreaksContract);
    let mut streaks = StreaksContractClient::new(&env, &streaks_id);

    let vault_id = Address::generate(&env);
    streaks.initialize(&vault_id);

    // Try to call update_streak from unauthorized address
    let user = Address::generate(&env);
    streaks.update_streak(&user); // Should panic
}

#[test]
#[should_panic(expected = "RewardAlreadyClaimed")]
fn test_double_claim_prevention() {
    let (env, _admin, user, vault, streaks, rewards) = setup_integration_test();

    // Create user vault
    let asset_code: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
    let token_issuer = Address::generate(&env);
    let vault_id_val = vault.create_vault(&user, &token_issuer, &asset_code, &86400);

    // Reach 7-day milestone
    for day in 1..=7 {
        env.ledger().set_timestamp(1704067200 + (day as u64 - 1) * 86400);
        vault.deposit(&vault_id_val, &user, &100);
    }

    // Claim first time succeeds
    let claimed = rewards.claim_rewards(&user);
    assert_eq!(claimed, 10_0000000);

    // Claim second time should panic
    rewards.claim_rewards(&user);
}

#[test]
fn test_qualifying_deposit_updates_streak_once() {
    let (env, _admin, user, vault, streaks, _rewards) = setup_integration_test();

    // Create a vault for the user
    let asset_code: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
    let token_issuer = Address::generate(&env);
    let vault_id_val = vault.create_vault(
        &user,
        &token_issuer,
        &asset_code,
        &86400, // 1 day lock
    );

    // Set timestamp to day 1
    env.ledger().set_timestamp(1704067200); // 2024-01-01 00:00:00 UTC

    // Make first deposit
    vault.deposit(&vault_id_val, &user, &100);

    // Verify streak is 1
    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 1, "First deposit should create streak of 1");

    // Verify vault balance is correct
    let balance = vault.get_balance(&vault_id_val);
    assert_eq!(balance, 100, "Vault balance should be 100");
}

#[test]
fn test_same_day_second_deposit_does_not_add_streak() {
    let (env, _admin, user, vault, streaks, _rewards) = setup_integration_test();

    // Create a vault for the user
    let asset_code: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
    let token_issuer = Address::generate(&env);
    let vault_id_val = vault.create_vault(
        &user,
        &token_issuer,
        &asset_code,
        &86400, // 1 day lock
    );

    // Set timestamp to day 1
    env.ledger().set_timestamp(1704067200); // 2024-01-01 00:00:00 UTC

    // Make first deposit
    vault.deposit(&vault_id_val, &user, &100);

    // Verify streak is 1
    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 1);

    // Try second deposit same day - should fail due to duplicate activity
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        vault.deposit(&vault_id_val, &user, &50);
    }));
    
    assert!(result.is_err(), "Same-day second deposit should fail with duplicate activity error");

    // Verify streak is still 1 (not incremented)
    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 1, "Streak should not increment on same-day duplicate deposit");

    // Verify vault balance is still 100 (second deposit should not have gone through)
    let balance = vault.get_balance(&vault_id_val);
    assert_eq!(balance, 100, "Vault balance should remain unchanged after failed duplicate deposit");
}

#[test]
fn test_milestone_deposit_creates_pending_reward() {
    let (env, _admin, user, vault, streaks, rewards) = setup_integration_test();

    // Create a vault for the user
    let asset_code: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
    let token_issuer = Address::generate(&env);
    let vault_id_val = vault.create_vault(
        &user,
        &token_issuer,
        &asset_code,
        &86400, // 1 day lock
    );

    // Make deposits for 7 consecutive days to reach 7-day milestone
    for day in 1..=7 {
        let timestamp = 1704067200 + (day as u64 - 1) * 86400;
        env.ledger().set_timestamp(timestamp);
        vault.deposit(&vault_id_val, &user, &100);
    }

    // Verify streak is 7
    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 7, "Streak should be 7 after 7 consecutive deposits");

    // Check that pending rewards include the 7-day milestone (10 tokens)
    let pending = rewards.get_pending_rewards(&user);
    assert_eq!(pending, 10_0000000, "Pending rewards should be 10 tokens for 7-day milestone");

    // Verify vault balance is 700 (7 deposits of 100 each)
    let balance = vault.get_balance(&vault_id_val);
    assert_eq!(balance, 700, "Vault balance should be 700 after 7 deposits");
}

#[test]
fn test_failed_streak_call_does_not_corrupt_vault_state() {
    let (env, _admin, user, vault, streaks, _rewards) = setup_integration_test();

    // Create a vault for the user
    let asset_code: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
    let token_issuer = Address::generate(&env);
    let vault_id_val = vault.create_vault(
        &user,
        &token_issuer,
        &asset_code,
        &86400, // 1 day lock
    );

    // Set timestamp to day 1
    env.ledger().set_timestamp(1704067200);

    // Make first deposit - this should succeed
    vault.deposit(&vault_id_val, &user, &100);

    // Verify initial state
    let initial_balance = vault.get_balance(&vault_id_val);
    assert_eq!(initial_balance, 100);
    
    let initial_streak = streaks.get_streak(&user);
    assert_eq!(initial_streak, 1);

    // Try same-day deposit - this will fail at streak level due to duplicate activity
    // but should not corrupt vault balance
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        vault.deposit(&vault_id_val, &user, &50);
    }));
    
    assert!(result.is_err(), "Same-day deposit should fail");

    // Verify vault state is not corrupted
    let final_balance = vault.get_balance(&vault_id_val);
    assert_eq!(final_balance, 100, "Vault balance should remain unchanged after failed streak call");
    
    let final_streak = streaks.get_streak(&user);
    assert_eq!(final_streak, 1, "Streak should remain unchanged after failed call");

    // Verify vault metadata is intact
    let vault_metadata = vault.get_vault(&vault_id_val);
    assert_eq!(vault_metadata.owner, user);
    assert_eq!(vault_metadata.status, shared::types::VaultStatus::Locked);
}

#[test]
fn test_failed_reward_call_does_not_corrupt_vault_state() {
    let (env, admin, user, vault, streaks, rewards) = setup_integration_test();

    // Create a vault for the user
    let asset_code: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
    let token_issuer = Address::generate(&env);
    let vault_id_val = vault.create_vault(
        &user,
        &token_issuer,
        &asset_code,
        &86400, // 1 day lock
    );

    // Empty the rewards pool to simulate liquidity failure
    rewards.require_auth_for_args(&admin, &()).fund_rewards_pool(&0_0000000);

    // Make deposits for 7 consecutive days
    for day in 1..=7 {
        let timestamp = 1704067200 + (day as u64 - 1) * 86400;
        env.ledger().set_timestamp(timestamp);
        
        // The 7th day deposit might fail when trying to grant reward due to insufficient liquidity
        // but should not corrupt vault state
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            vault.deposit(&vault_id_val, &user, &100);
        }));
        
        // On day 7, the reward grant will fail due to insufficient liquidity
        if day == 7 {
            assert!(result.is_err(), "Deposit on day 7 should fail due to insufficient reward liquidity");
        }
    }

    // Verify vault state is not corrupted despite reward failure
    // The vault should still have processed the deposits correctly
    let balance = vault.get_balance(&vault_id_val);
    // Since day 7 failed, we should have 6 deposits of 100 each
    assert_eq!(balance, 600, "Vault balance should reflect successful deposits only");

    // Verify streak is still correct
    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 6, "Streak should be 6 after 6 successful deposits");

    // Verify vault metadata is intact
    let vault_metadata = vault.get_vault(&vault_id_val);
    assert_eq!(vault_metadata.owner, user);
    assert_eq!(vault_metadata.status, shared::types::VaultStatus::Locked);
}