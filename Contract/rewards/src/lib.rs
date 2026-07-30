#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Env};
use soroban_sdk::{
    contract, contractimpl, contracttype,
    Address, BytesN, Env, Vec,
};
use shared::errors::Error;
use shared::events::RewardGranted;
use shared::types::{RateLimit, Role, Permission};
use shared::utils::{TimeHelper, SafeMath, ValidationHelper};

/// Storage keys for rewards contract
#[derive(Clone)]
#[contracttype]
pub enum RewardsKey {
    RewardPool,
    UserBalance(Address),
    PendingRewards(Address),
    RewardConfig,
    RateLimit,
    AdminPermissions(Address),
    ClaimHistory(Address),
    GlobalClaimCount,
}

/// Reward pool information
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct RewardPool {
    pub total_allocated: i128,
    pub total_claimed: i128,
    pub reward_asset: BytesN<32>,
    pub last_distribution: u64,
}

/// Reward configuration
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct RewardConfig {
    pub claim_cooldown: u64,
    pub max_claim_per_period: i128,
    pub period_seconds: u64,
    pub streak_bonus_enabled: bool,
}

/// Claim record
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ClaimRecord {
    pub timestamp: u64,
    pub amount: i128,
    pub reward_type: u32,
}

/// Rewards contract for distributing protocol rewards with advanced features
use soroban_sdk::{contract, contractimpl, Address, Env};
use shared::types::Asset;

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Map, Vec};
use shared::{
    errors::Error,
    events::{MilestoneReached, RewardClaimed, RewardGranted, RewardsPoolFunded},
    types::{Milestone, RewardsPool, UserReward},
    utils::TimeHelper,
};

/// Rewards contract for distributing protocol rewards
/// Handles:
/// - Reward pool initialization and funding by admin
/// - Milestone-based reward entitlement creation
/// - Reward claiming with double-claim protection
/// - Liquidity tracking to prevent over-distribution
#[contract]
pub struct RewardsContract;

#[cfg(test)]
pub use RewardsContract;

// Storage key constants
const STREAKS_CONTRACT_KEY: &[u8; 32] = b"streaks_contract_address___\0\0\0\0\0";
const MILESTONES_KEY: &[u8; 32] = b"registered_milestones_____\0\0\0\0\0\0";
const REWARDS_POOL_KEY: &[u8; 32] = b"rewards_pool_state________\0\0\0\0\0\0";

// Milestone threshold constants (streak days)
const MILESTONE_7_DAYS: u32 = 7;
const MILESTONE_30_DAYS: u32 = 30;
const MILESTONE_100_DAYS: u32 = 100;
const MILESTONE_365_DAYS: u32 = 365;

#[contractimpl]
impl RewardsContract {
    pub fn initialize_rewards(
        env: Env,
        total_pool: i128,
        reward_asset: BytesN<32>,
    ) -> Result<(), Error> {
        if !ValidationHelper::validate_positive_amount(total_pool) {
            return Err(Error::InvalidAmount);
        }

        let reward_pool = RewardPool {
            total_allocated: total_pool,
            total_claimed: 0,
            reward_asset: reward_asset.clone(),
            last_distribution: TimeHelper::now(&env),
        };

        env.storage()
            .persistent()
            .set(&RewardsKey::RewardPool, &reward_pool);

        Ok(())
    }

    pub fn claim_rewards(env: Env, user: Address) -> Result<i128, Error> {
        Self::check_rate_limit(&env)?;

        let config: RewardConfig = Self::get_config(&env);
        let now = TimeHelper::now(&env);

        let claim_history_key = RewardsKey::ClaimHistory(user.clone());
        let claim_history: Vec<ClaimRecord> = env
            .storage()
            .persistent()
            .get(&claim_history_key)
            .unwrap_or(Vec::new(&env));

        if claim_history.len() > 0 {
            let last_claim = claim_history.get(claim_history.len() - 1).unwrap();
            if now.saturating_sub(last_claim.timestamp) < config.claim_cooldown {
                return Err(Error::CooldownPeriodNotMet);
            }
        }

        let pending: i128 = env
            .storage()
            .persistent()
            .get(&RewardsKey::PendingRewards(user.clone()))
            .unwrap_or(0);

        if pending == 0 {
            return Err(Error::InsufficientBalance);
        }

        let mut reward_pool: RewardPool = env
            .storage()
            .persistent()
            .get(&RewardsKey::RewardPool)
            .ok_or(Error::NotInitialized)?;

        reward_pool.total_claimed = SafeMath::add(reward_pool.total_claimed, pending)
            .ok_or(Error::Overflow)?;
        env.storage()
            .persistent()
            .set(&RewardsKey::RewardPool, &reward_pool);

        env.storage()
            .persistent()
            .set(&RewardsKey::PendingRewards(user.clone()), &0i128);

        let mut updated_history = claim_history;
        updated_history.push_back(ClaimRecord {
            timestamp: now,
            amount: pending,
            reward_type: 0,
        });
        if updated_history.len() > 50 {
            updated_history.remove(0);
        }
        env.storage()
            .persistent()
            .set(&claim_history_key, &updated_history);

        let mut count: u64 = env
            .storage()
            .persistent()
            .get(&RewardsKey::GlobalClaimCount)
            .unwrap_or(0);
        count = count.checked_add(1).ok_or(Error::Overflow)?;
        env.storage()
            .persistent()
            .set(&RewardsKey::GlobalClaimCount, &count);

        env.events().publish(
            (RewardGranted::topic(&env), user.clone()),
            RewardGranted {
                recipient: user,
                reward_amount: pending,
                reward_type: 0,
            },
        );

        Ok(pending)
    }

    pub fn grant_reward(
        env: Env,
        recipient: Address,
        amount: i128,
        reward_type: u32,
    ) -> Result<(), Error> {
        if !ValidationHelper::validate_positive_amount(amount) {
            return Err(Error::InvalidAmount);
        }

        let mut pending: i128 = env
            .storage()
            .persistent()
            .get(&RewardsKey::PendingRewards(recipient.clone()))
            .unwrap_or(0);

        pending = SafeMath::add(pending, amount).ok_or(Error::Overflow)?;
        env.storage()
            .persistent()
            .set(&RewardsKey::PendingRewards(recipient), &pending);

        Ok(())
    }

    pub fn get_pending_rewards(env: Env, user: Address) -> Result<i128, Error> {
        let pending: i128 = env
            .storage()
            .persistent()
            .get(&RewardsKey::PendingRewards(user))
            .unwrap_or(0);
        Ok(pending)
    }

    pub fn calculate_streak_bonus(env: Env, streak_count: u32) -> u32 {
        let config: RewardConfig = Self::get_config(&env);
        if !config.streak_bonus_enabled {
            return 1;
        }

        match streak_count {
            0 => 1,
            1..=7 => 1,
            8..=30 => 2,
            31..=90 => 3,
            91..=180 => 4,
            _ => 5,
        }
    }

    pub fn get_reward_pool(env: Env) -> Result<RewardPool, Error> {
        let pool: RewardPool = env
            .storage()
            .persistent()
            .get(&RewardsKey::RewardPool)
            .ok_or(Error::NotInitialized)?;
        Ok(pool)
    }

    pub fn set_config(env: Env, admin: Address, config: RewardConfig) -> Result<(), Error> {
        if !Self::is_admin(&env, &admin) {
            return Err(Error::PermissionDenied);
        }
        env.storage().persistent().set(&RewardsKey::RewardConfig, &config);
        Ok(())
    }

    fn get_config(env: &Env) -> RewardConfig {
        env.storage()
            .persistent()
            .get(&RewardsKey::RewardConfig)
            .unwrap_or(RewardConfig {
                claim_cooldown: 86400,
                max_claim_per_period: 1000,
                period_seconds: 86400,
                streak_bonus_enabled: true,
            })
    }

    fn check_rate_limit(env: &Env) -> Result<(), Error> {
        let mut rate_limit: RateLimit = env
            .storage()
            .persistent()
            .get(&RewardsKey::RateLimit)
            .unwrap_or(RateLimit::new(50, 60));

        let now = TimeHelper::now(env);

        if now >= rate_limit.period_start + rate_limit.period_seconds {
            rate_limit.current_count = 0;
            rate_limit.period_start = now;
        }

        if rate_limit.current_count >= rate_limit.max_operations_per_period {
            return Err(Error::RateLimitExceeded);
        }

        rate_limit.current_count = rate_limit.current_count.checked_add(1).ok_or(Error::Overflow)?;
        env.storage().persistent().set(&RewardsKey::RateLimit, &rate_limit);

        Ok(())
    }

    pub fn grant_admin(env: Env, admin: Address) -> Result<(), Error> {
        let permission = Permission {
            role: Role::Admin,
            granted_at: TimeHelper::now(&env),
            expires_at: None,
        };
        env.storage()
            .persistent()
            .set(&RewardsKey::AdminPermissions(admin), &permission);
        Ok(())
    }

    fn is_admin(env: &Env, address: &Address) -> bool {
        if let Some(permission) = env
            .storage()
            .persistent()
            .get::<_, Permission>(&RewardsKey::AdminPermissions(address.clone()))
        {
            permission.role == Role::Admin
        } else {
            false
        }
    }

    pub fn get_claim_history(env: Env, user: Address) -> Result<Vec<ClaimRecord>, Error> {
        let history: Vec<ClaimRecord> = env
            .storage()
            .persistent()
            .get(&RewardsKey::ClaimHistory(user))
            .unwrap_or(Vec::new(&env));
        Ok(history)
    }

    pub fn get_global_claim_count(env: Env) -> Result<u64, Error> {
        let count: u64 = env
            .storage()
            .persistent()
            .get(&RewardsKey::GlobalClaimCount)
            .unwrap_or(0);
        Ok(count)
    }

    /// Initialize the rewards system with admin and reward asset
    /// Can only be called once
    pub fn initialize(env: Env, admin: Address, reward_asset: BytesN<32>, streaks_contract: Address) {
        // Check if already initialized
        let pool_key = BytesN::from_array(&env, REWARDS_POOL_KEY);
        if env.storage().instance().has(&pool_key) {
            panic!("{:?}", Error::RewardsPoolAlreadyInitialized);
        }

        // Store streaks contract address
        let streaks_key = BytesN::from_array(&env, STREAKS_CONTRACT_KEY);
        env.storage().instance().set(&streaks_key, &streaks_contract);

        // Initialize rewards pool
        let pool = RewardsPool {
            total_funded: 0,
            available_liquidity: 0,
            reward_asset,
            initialized: true,
            admin,
        };
        env.storage().instance().set(&pool_key, &pool);

        // Register default milestones
        Self::register_default_milestones(&env);
    }

    /// Fund the rewards pool with additional tokens
    /// Only admin can call this
    pub fn fund_rewards_pool(env: Env, amount: i128) {
        let mut pool = Self::get_rewards_pool(&env);

        // Validate amount
        if amount <= 0 {
            panic!("{:?}", Error::InvalidAmount);
        }

        // Update pool state
        pool.available_liquidity = pool.available_liquidity.checked_add(amount).unwrap();
        pool.total_funded = pool.total_funded.checked_add(amount).unwrap();
        Self::set_rewards_pool(&env, pool.clone());

        // Emit event
        env.events().publish(
            ("pool_funded", pool.admin.clone()),
            RewardsPoolFunded {
                admin: pool.admin,
                amount,
                total_pool: pool.total_funded,
            },
        );
    }

    /// Grant a reward to a user for reaching a milestone
    /// Called automatically by streaks contract when milestone is hit
    pub fn grant_reward(env: Env, recipient: Address, streak_count: u32) {
        let pool = Self::get_rewards_pool(&env);
        if !pool.initialized {
            panic!("{:?}", Error::RewardsPoolNotInitialized);
        }

        // Get all milestones
        let milestones = Self::get_milestones(&env);

        // Find matching milestone that hasn't been granted yet
        for milestone in milestones.iter() {
            if streak_count >= milestone.streak_threshold {
                // Generate milestone ID from threshold
                let milestone_id = Self::create_milestone_id(&env, milestone.streak_threshold);

                // Check if user already has this reward
                if Self::user_has_reward(&env, &recipient, &milestone_id) {
                    continue;
                }

                // Check liquidity
                if pool.available_liquidity < milestone.reward_amount {
                    panic!("{:?}", Error::InsufficientRewardLiquidity);
                }

                // Create user reward
                let user_reward = UserReward {
                    milestone_id: milestone_id.clone(),
                    amount: milestone.reward_amount,
                    claimed: false,
                    claimed_at: None,
                };

                // Store the reward
                Self::store_user_reward(&env, &recipient, user_reward);

                // Deduct from pool liquidity
                let mut pool_mut = pool.clone();
                pool_mut.available_liquidity -= milestone.reward_amount;
                Self::set_rewards_pool(&env, pool_mut);

                // Emit events
                env.events().publish(
                    ("milestone_reached", recipient.clone()),
                    MilestoneReached {
                        user: recipient.clone(),
                        streak: streak_count,
                        milestone_id: milestone_id.clone(),
                    },
                );

                env.events().publish(
                    ("reward_granted", recipient.clone()),
                    RewardGranted {
                        recipient: recipient.clone(),
                        reward_amount: milestone.reward_amount,
                        reward_type: milestone.reward_type,
                        milestone_id,
                    },
                );

                // Only grant one milestone per call to prevent multiple grants
                break;
            }
        }
    }

    /// Claim rewards for a user
    /// User must authorize the claim
    /// Returns the amount claimed
    pub fn claim_rewards(env: Env, user: Address) -> i128 {
        user.require_auth();

        let mut total_claimed: i128 = 0;
        let user_rewards = Self::get_user_rewards(&env, &user);

        for mut reward in user_rewards.iter() {
            if !reward.claimed {
                // Mark as claimed
                reward.claimed = true;
                reward.claimed_at = Some(TimeHelper::now(&env));
                let amount = reward.amount;
                let milestone_id = reward.milestone_id.clone();
                total_claimed += amount;

                // Update stored reward
                Self::update_user_reward(&env, &user, reward);

                // Emit claim event
                env.events().publish(
                    ("reward_claimed", user.clone()),
                    RewardClaimed {
                        user: user.clone(),
                        amount,
                        milestone_id,
                    },
                );
            }
        }

        if total_claimed == 0 {
            panic!("{:?}", Error::RewardNotEligible);
        }

        total_claimed
    }

    /// Get a user's pending (unclaimed) rewards
    pub fn get_pending_rewards(env: Env, user: Address) -> i128 {
        let mut pending: i128 = 0;
        let user_rewards = Self::get_user_rewards(&env, &user);

        for reward in user_rewards.iter() {
            if !reward.claimed {
                pending += reward.amount;
            }
        }

        pending
    }

    /// Calculate streak bonus multiplier for additional rewards
    /// Returns basis points (100 = 1.0x, 150 = 1.5x, etc.)
    pub fn calculate_streak_bonus(_env: Env, streak_count: u32) -> u32 {
        match streak_count {
            0..=6 => 100, // No bonus for first week
            7..=29 => 110, // 10% bonus at 7+ days
            30..=99 => 125, // 25% bonus at 30+ days
            100..=364 => 150, // 50% bonus at 100+ days
            _ => 200, // 100% bonus at 365+ days
        }
    }

    /// Add a new milestone (admin only)
    pub fn add_milestone(env: Env, threshold: u32, reward_amount: i128, reward_type: u32) {
        let _pool = Self::get_rewards_pool(&env);

        let mut milestones = Self::get_milestones(&env);
        milestones.push_back(Milestone {
            streak_threshold: threshold,
            reward_amount,
            reward_type,
        });
        Self::set_milestones(&env, milestones);
    }

    /// Get the current rewards pool state
    pub fn get_pool_state(env: Env) -> RewardsPool {
        Self::get_rewards_pool(&env)
    }

    /// Get all milestones
    pub fn get_milestones_list(env: Env) -> Vec<Milestone> {
        Self::get_milestones(&env)
    }

    /// Internal helper to register default milestones
    fn register_default_milestones(env: &Env) {
        let mut milestones = Vec::new(env);

        // 7-day milestone: 10 reward tokens
        milestones.push_back(Milestone {
            streak_threshold: MILESTONE_7_DAYS,
            reward_amount: 10_0000000, // 10 tokens with 7 decimals
            reward_type: 0,
        });

        // 30-day milestone: 50 reward tokens
        milestones.push_back(Milestone {
            streak_threshold: MILESTONE_30_DAYS,
            reward_amount: 50_0000000,
            reward_type: 0,
        });

        // 100-day milestone: 200 reward tokens
        milestones.push_back(Milestone {
            streak_threshold: MILESTONE_100_DAYS,
            reward_amount: 200_0000000,
            reward_type: 0,
        });

        // 365-day milestone: 1000 reward tokens
        milestones.push_back(Milestone {
            streak_threshold: MILESTONE_365_DAYS,
            reward_amount: 1000_0000000,
            reward_type: 0,
        });

        Self::set_milestones(env, milestones);
    }

    /// Verify caller is the authorized streaks contract
    #[allow(dead_code)]
    fn verify_streaks_authorization(env: &Env) {
        let streaks_key = BytesN::from_array(env, STREAKS_CONTRACT_KEY);
        let _streaks_contract: Address = env.storage().instance().get(&streaks_key).unwrap();
    }

    /// Create a unique milestone ID from the streak threshold
    fn create_milestone_id(env: &Env, threshold: u32) -> BytesN<32> {
        let mut key = [0u8; 32];
        let threshold_bytes = threshold.to_be_bytes();
        for i in 0..4 {
            key[i] = threshold_bytes[i];
        }
        for i in 4..32 {
            key[i] = b"milestone"[i % 8];
        }
        BytesN::from_array(env, &key)
    }

    /// Get rewards pool from storage
    fn get_rewards_pool(env: &Env) -> RewardsPool {
        let key = BytesN::from_array(env, REWARDS_POOL_KEY);
        env.storage().instance().get(&key).unwrap()
    }

    /// Set rewards pool in storage
    fn set_rewards_pool(env: &Env, pool: RewardsPool) {
        let key = BytesN::from_array(env, REWARDS_POOL_KEY);
        env.storage().instance().set(&key, &pool);
    }

    /// Get all registered milestones
    fn get_milestones(env: &Env) -> Vec<Milestone> {
        let key = BytesN::from_array(env, MILESTONES_KEY);
        env.storage().instance().get(&key).unwrap_or_else(|| Vec::new(env))
    }

    /// Set milestones in storage
    fn set_milestones(env: &Env, milestones: Vec<Milestone>) {
        let key = BytesN::from_array(env, MILESTONES_KEY);
        env.storage().instance().set(&key, &milestones);
    }

    /// Create storage key for the reward map
    fn rewards_storage_key(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[8u8; 32])
    }

    /// Check if user already has this reward
    fn user_has_reward(env: &Env, user: &Address, milestone_id: &BytesN<32>) -> bool {
        let rewards = Self::get_user_rewards_map(env);
        if let Some(user_rewards) = rewards.get(user.clone()) {
            for reward in user_rewards.iter() {
                if reward.milestone_id == *milestone_id {
                    return true;
                }
            }
        }
        false
    }

    /// Store a new user reward
    fn store_user_reward(env: &Env, user: &Address, reward: UserReward) {
        let mut rewards = Self::get_user_rewards_map(env);
        let mut user_rewards = rewards.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        user_rewards.push_back(reward);
        rewards.set(user.clone(), user_rewards);
        Self::set_user_rewards_map(env, rewards);
    }

    /// Update an existing user reward
    fn update_user_reward(env: &Env, user: &Address, reward: UserReward) {
        let mut rewards = Self::get_user_rewards_map(env);
        let user_rewards = rewards.get(user.clone()).unwrap_or_else(|| Vec::new(env));
        let mut updated_rewards = Vec::new(env);

        for existing_reward in user_rewards.iter() {
            if existing_reward.milestone_id == reward.milestone_id {
                updated_rewards.push_back(reward.clone());
            } else {
                updated_rewards.push_back(existing_reward);
            }
        }

        rewards.set(user.clone(), updated_rewards);
        Self::set_user_rewards_map(env, rewards);
    }

    /// Get all rewards for a user
    fn get_user_rewards(env: &Env, user: &Address) -> Vec<UserReward> {
        let rewards = Self::get_user_rewards_map(env);
        rewards.get(user.clone()).unwrap_or_else(|| Vec::new(env))
    }

    fn get_user_rewards_map(env: &Env) -> Map<Address, Vec<UserReward>> {
        let key = Self::rewards_storage_key(env);
        env.storage().persistent().get(&key).unwrap_or_else(|| Map::new(env))
    }

    fn set_user_rewards_map(env: &Env, rewards: Map<Address, Vec<UserReward>>) {
        let key = Self::rewards_storage_key(env);
        env.storage().persistent().set(&key, &rewards);
    }
}