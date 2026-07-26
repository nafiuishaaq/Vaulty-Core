#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};
use streaks::StreaksContractClient;
use rewards::RewardsContractClient;
use vault::VaultContractClient;

#[test]
fn test_full_progression_flow() {
    let env = Env::default();
    env.budget().reset_unlimited();

    // Create test accounts
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let token_issuer = Address::generate(&env);

    // Deploy all contracts
    let streaks_id = env.register_contract(None, streaks::StreaksContract);
    let rewards_id = env.register_contract(None, rewards::RewardsContract);
    let vault_id = env.register_contract(None, vault::VaultContract);

    // Create clients
    let mut streaks = StreaksContractClient::new(&env, &streaks_id);
    let mut rewards = RewardsContractClient::new(&env, &rewards_id);
    let mut vault = VaultContractClient::new(&env, &vault_id);

    // Initialize all contracts
    streaks.initialize(&vault_id); // Vault is the only authorized caller for streaks
    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    rewards.initialize(&admin, &reward_asset, &streaks_id);
    vault.initialize(&streaks_id, &rewards_id);

    // Register vault with streaks to add it as authorized caller
    vault.register_with_streaks();

    // Fund the rewards pool as admin
    rewards.require_auth_for_args(&admin, &()).fund_rewards_pool(&10000_0000000);

    // Create a vault for the user
    let asset_code: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
    let vault_id_val = vault.create_vault(
        &user,
        &asset_code,
        Some(token_issuer),
        86400, // 1 day lock
    );

    // DAY 1: First deposit creates streak
    env.ledger().set_timestamp(1704067200); // 2024-01-01 00:00:00 UTC
    vault.deposit(&vault_id_val, &user, &100);

    // Verify streak is 1
    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 1);

    // DAY 2: Second consecutive deposit increments streak to 2
    env.ledger().set_timestamp(1704153600); // 2024-01-02 00:00:00 UTC
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        vault.deposit(&vault_id_val, &user, &100);
    }));
    assert!(result.is_ok(), "Second deposit should succeed");

    let streak = streaks.get_streak(&user);
    assert_eq!(streak, 2);

    // Try to deposit again same day - should fail with duplicate activity
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        vault.deposit(&vault_id_val, &user, &100);
    }));
    assert!(result.is_err(), "Same-day duplicate deposit should fail");

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
    let env = Env::default();
    env.budget().reset_unlimited();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let streaks_id = env.register_contract(None, streaks::StreaksContract);
    let rewards_id = env.register_contract(None, rewards::RewardsContract);
    let vault_id = env.register_contract(None, vault::VaultContract);

    let mut streaks = StreaksContractClient::new(&env, &streaks_id);
    let mut rewards = RewardsContractClient::new(&env, &rewards_id);
    let mut vault = VaultContractClient::new(&env, &vault_id);

    // Initialize
    streaks.initialize(&vault_id);
    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    rewards.initialize(&admin, &reward_asset, &streaks_id);
    vault.initialize(&streaks_id, &rewards_id);
    vault.register_with_streaks();
    rewards.fund_rewards_pool(&10000_0000000);

    // Create user vault
    let asset_code: BytesN<32> = BytesN::from_array(&env, &[1u8; 32]);
    let token_issuer = Address::generate(&env);
    let vault_id_val = vault.create_vault(&user, &asset_code, Some(token_issuer), 86400);

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