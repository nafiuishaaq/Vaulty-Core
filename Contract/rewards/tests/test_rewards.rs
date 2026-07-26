#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{Address, Ledger},
    BytesN, Env,
};
use rewards::RewardsContractClient;
use shared::types::UserReward;

const WASM: &[u8] = rewards::WASM;

#[test]
fn test_rewards_pool_initialization() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, rewards::WASM);
    let mut client = RewardsContractClient::new(&env, &contract_id);

    // Initialize rewards pool
    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);

    // Fund the pool
    admin.require_auth();
    client.fund_rewards_pool(&1000_0000000);

    let pool = client.get_pool_state();
    assert_eq!(pool.available_liquidity, 1000_0000000);
    assert_eq!(pool.reward_asset, reward_asset);
}

#[test]
fn test_milestone_7day_grant() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, rewards::WASM);
    let mut client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);
    admin.require_auth();
    client.fund_rewards_pool(&1000_0000000);

    // Grant reward when streak reaches 7
    client.grant_reward(&user, &7);

    let pending = client.get_pending_rewards(&user);
    assert_eq!(pending, 10_0000000); // Default 7-day milestone reward
}

#[test]
fn test_multiple_milestones() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, rewards::WASM);
    let mut client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);
    admin.require_auth();
    client.fund_rewards_pool(&1000_0000000);

    // Grant reward when streak reaches 30 - should get both 7 and 30 day rewards
    client.grant_reward(&user, &30);

    let pending = client.get_pending_rewards(&user);
    // 10 (7-day) + 50 (30-day) = 60
    assert_eq!(pending, 60_0000000);
}

#[test]
fn test_claim_rewards() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, rewards::WASM);
    let mut client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);
    admin.require_auth();
    client.fund_rewards_pool(&1000_0000000);

    // Grant 7-day reward
    client.grant_reward(&user, &7);

    // Claim rewards
    user.require_auth();
    let claimed = client.claim_rewards(&user);
    assert_eq!(claimed, 10_0000000);

    // Pending should be 0
    let pending = client.get_pending_rewards(&user);
    assert_eq!(pending, 0);

    // Pool liquidity should be reduced
    let pool = client.get_pool_state();
    assert_eq!(pool.available_liquidity, 990_0000000);
}

#[test]
#[should_panic(expected = "RewardAlreadyClaimed")]
fn test_double_claim_prevention() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, rewards::WASM);
    let mut client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);
    admin.require_auth();
    client.fund_rewards_pool(&1000_0000000);

    // Grant reward
    client.grant_reward(&user, &7);

    // Claim first time
    user.require_auth();
    client.claim_rewards(&user);

    // Claim second time - should panic
    client.claim_rewards(&user);
}

#[test]
fn test_add_custom_milestone() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, rewards::WASM);
    let mut client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);

    // Add custom milestone: 180 days for 100 tokens
    admin.require_auth();
    client.add_milestone(&180, &100_0000000, &0);

    let milestones = client.get_milestones_list();
    // 4 default + 1 custom = 5 milestones
    assert_eq!(milestones.len(), 5);
    let custom_milestone = milestones.get(4).unwrap();
    assert_eq!(custom_milestone.streak_threshold, 180);
    assert_eq!(custom_milestone.reward_amount, 100_0000000);
}

#[test]
#[should_panic(expected = "InsufficientRewardLiquidity")]
fn test_insufficient_liquidity() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let contract_id = env.register_contract_wasm(None, rewards::WASM);
    let mut client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);
    // Only fund pool with 15 tokens - enough for one 10-token reward, but not two
    admin.require_auth();
    client.fund_rewards_pool(&15_0000000);

    // First user gets reward
    client.grant_reward(&user1, &7);
    assert_eq!(client.get_pending_rewards(&user1), 10_0000000);

    // Second user tries to get reward - pool only has 5 left, insufficient for 10
    client.grant_reward(&user2, &7);
}