#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};
use rewards::{RewardsContract, RewardsContractClient};

#[test]
fn test_rewards_pool_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);

    client.fund_rewards_pool(&1000_0000000);

    let pool = client.get_pool_state();
    assert_eq!(pool.available_liquidity, 1000_0000000);
    assert_eq!(pool.reward_asset, reward_asset);
}

#[test]
fn test_milestone_7day_grant() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);
    client.fund_rewards_pool(&1000_0000000);

    client.grant_reward(&user, &7);

    let pending = client.get_pending_rewards(&user);
    assert_eq!(pending, 10_0000000);
}

#[test]
fn test_multiple_milestones() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);
    client.fund_rewards_pool(&1000_0000000);

    client.grant_reward(&user, &30);

    let pending = client.get_pending_rewards(&user);
    assert_eq!(pending, 60_0000000);

    let pool = client.get_pool_state();
    assert_eq!(pool.available_liquidity, 940_0000000);

    client.grant_reward(&user, &30);
    assert_eq!(client.get_pending_rewards(&user), 60_0000000);
    assert_eq!(client.get_pool_state().available_liquidity, 940_0000000);
}

#[test]
fn test_claim_rewards() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);
    client.fund_rewards_pool(&1000_0000000);

    client.grant_reward(&user, &7);

    let claimed = client.claim_rewards(&user);
    assert_eq!(claimed, 10_0000000);

    let pending = client.get_pending_rewards(&user);
    assert_eq!(pending, 0);

    let pool = client.get_pool_state();
    assert_eq!(pool.available_liquidity, 990_0000000);
}

#[test]
#[should_panic(expected = "RewardNotEligible")]
fn test_double_claim_prevention() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);
    client.fund_rewards_pool(&1000_0000000);

    client.grant_reward(&user, &7);

    client.claim_rewards(&user);

    client.claim_rewards(&user);
}

#[test]
fn test_add_custom_milestone() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let mut client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);

    client.add_milestone(&180, &100_0000000, &0);

    let milestones = client.get_milestones_list();
    assert_eq!(milestones.len(), 5);
    let custom_milestone = milestones.get(4).unwrap();
    assert_eq!(custom_milestone.streak_threshold, 180);
    assert_eq!(custom_milestone.reward_amount, 100_0000000);
}

#[test]
#[should_panic(expected = "InsufficientRewardLiquidity")]
fn test_insufficient_liquidity() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks_id = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset: BytesN<32> = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks_id);
    client.fund_rewards_pool(&15_0000000);

    client.grant_reward(&user1, &7);
    assert_eq!(client.get_pending_rewards(&user1), 10_0000000);

    client.grant_reward(&user2, &7);
}
