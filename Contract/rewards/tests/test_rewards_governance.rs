//! Tests for rewards governance: initialization, admin authorization,
//! two-step admin transfer, and unauthorized access prevention.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env,
};
use rewards::{RewardsContract, RewardsContractClient};

// ===========================================================================
// Initialization
// ===========================================================================

#[test]
fn test_initialize_rewards_sets_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);

    let pool = client.get_pool_state();
    assert_eq!(pool.admin, admin);
    assert!(pool.initialized);
}

#[test]
fn test_initialize_rewards_prevents_double_init() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);

    // Second init must fail
    let result = client.try_initialize(&admin, &reward_asset, &streaks);
    assert_eq!(result, Err(Ok(shared::errors::Error::AlreadyInitialized)));
}

#[test]
fn test_initialize_rewards_with_funding_sets_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize_rewards(&admin, &1000_0000000, &reward_asset);

    let pool = client.get_pool_state();
    assert_eq!(pool.admin, admin);
    assert_eq!(pool.total_funded, 1000_0000000);
}

#[test]
fn test_initialize_rewards_prevents_double_init() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize_rewards(&admin, &1000_0000000, &reward_asset);

    // Second init must fail
    let result = client.try_initialize_rewards(&admin, &500_0000000, &reward_asset);
    assert_eq!(result, Err(Ok(shared::errors::Error::AlreadyInitialized)));
}

// ===========================================================================
// grant_admin requires admin
// ===========================================================================

#[test]
fn test_grant_admin_requires_admin_auth() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);

    // Without mock_all_auths, non-admin won't have auth
    let env_no_mock = Env::default();
    let non_admin = Address::generate(&env_no_mock);
    let new_admin = Address::generate(&env_no_mock);
    let streaks2 = Address::generate(&env_no_mock);
    let contract_id2 = env_no_mock.register_contract(None, RewardsContract);
    let client2 = RewardsContractClient::new(&env_no_mock, &contract_id2);
    let ra2 = BytesN::from_array(&env_no_mock, &[0u8; 32]);
    client2.initialize(&non_admin, &ra2, &streaks2);

    // Non-admin trying to grant admin should fail
    let result = client2.try_grant_admin(&new_admin);
    assert_eq!(result, Err(Ok(shared::errors::Error::Unauthorized)));
}

#[test]
fn test_grant_admin_succeeds_for_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);

    let new_role_admin = Address::generate(&env);
    let result = client.try_grant_admin(&new_role_admin);
    assert_eq!(result, Ok(Ok(())));
}

// ===========================================================================
// fund_rewards_pool requires admin
// ===========================================================================

#[test]
fn test_fund_rewards_pool_requires_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);

    // Without mock_all_auths, non-admin won't have auth
    let env_no_mock = Env::default();
    let non_admin = Address::generate(&env_no_mock);
    let contract_id2 = env_no_mock.register_contract(None, RewardsContract);
    let client2 = RewardsContractClient::new(&env_no_mock, &contract_id2);
    let ra2 = BytesN::from_array(&env_no_mock, &[0u8; 32]);
    let streaks2 = Address::generate(&env_no_mock);
    client2.initialize(&non_admin, &ra2, &streaks2);

    // Non-admin funding should fail
    let result = client2.try_fund_rewards_pool(&1000_0000000);
    assert!(result.is_err());
}

#[test]
fn test_fund_rewards_pool_succeeds_for_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);
    client.fund_rewards_pool(&1000_0000000);

    let pool = client.get_pool_state();
    assert_eq!(pool.available_liquidity, 1000_0000000);
}

// ===========================================================================
// add_milestone requires admin
// ===========================================================================

#[test]
fn test_add_milestone_requires_admin() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);

    // Without mock_all_auths, non-admin won't have auth
    let env_no_mock = Env::default();
    let non_admin = Address::generate(&env_no_mock);
    let contract_id2 = env_no_mock.register_contract(None, RewardsContract);
    let client2 = RewardsContractClient::new(&env_no_mock, &contract_id2);
    let ra2 = BytesN::from_array(&env_no_mock, &[0u8; 32]);
    let streaks2 = Address::generate(&env_no_mock);
    client2.initialize(&non_admin, &ra2, &streaks2);

    let result = client2.try_add_milestone(&50, &25_0000000, &0);
    assert!(result.is_err());
}

#[test]
fn test_add_milestone_succeeds_for_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);

    client.add_milestone(&180, &100_0000000, &0);

    let milestones = client.get_milestones_list();
    // 4 defaults + 1 custom
    assert_eq!(milestones.len(), 5);
    let custom = milestones.get(4).unwrap();
    assert_eq!(custom.streak_threshold, 180);
}

// ===========================================================================
// Two-step admin transfer
// ===========================================================================

#[test]
fn test_transfer_and_accept_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);

    let new_admin = Address::generate(&env);
    client.transfer_admin(&new_admin);
    client.accept_admin();

    let pool = client.get_pool_state();
    assert_eq!(pool.admin, new_admin);
}

#[test]
fn test_transfer_admin_rejects_self_transfer() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);

    let result = client.try_transfer_admin(&admin);
    assert_eq!(result, Err(Ok(shared::errors::Error::CannotTransferToSelf)));
}

#[test]
fn test_accept_admin_without_pending_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);

    let result = client.try_accept_admin();
    assert_eq!(result, Err(Ok(shared::errors::Error::NoAdminTransferPending)));
}

// ===========================================================================
// Functional flows with new governance
// ===========================================================================

#[test]
fn test_full_reward_flow_after_governance_init() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let user = Address::generate(&env);
    let contract_id = env.register_contract(None, RewardsContract);
    let client = RewardsContractClient::new(&env, &contract_id);

    let reward_asset = BytesN::from_array(&env, &[0u8; 32]);
    client.initialize(&admin, &reward_asset, &streaks);
    client.fund_rewards_pool(&1000_0000000);

    // Grant 7-day milestone
    client.grant_reward(&user, &7);
    let pending = client.get_pending_rewards(user.clone());
    assert_eq!(pending, 10_0000000);

    // Claim
    let claimed = client.claim_rewards(&user);
    assert_eq!(claimed, 10_0000000);

    let pending_after = client.get_pending_rewards(user);
    assert_eq!(pending_after, 0);
}
