#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Map, Vec};
use shared::{
    errors::Error,
    types::{Asset, LendingPool, PoolPosition},
    utils::{SafeMath, ValidationHelper},
};

/// Lending contract for managing lending pools and interest.
#[contract]
pub struct LendingContract;

#[contractimpl]
impl LendingContract {
    pub fn create_pool(env: Env, asset: BytesN<32>, interest_rate_bps: i128) -> LendingPool {
        if interest_rate_bps <= 0 || interest_rate_bps > 10_000 {
            panic!("{:?}", Error::InvalidInterestRate);
        }

        let pool_id = Self::derive_pool_id(&env, &asset);
        let mut pools = Self::get_pools(&env);
        if pools.contains_key(pool_id.clone()) {
            panic!("{:?}", Error::PoolAlreadyInitialized);
        }

        let pool = LendingPool {
            pool_id: pool_id.clone(),
            asset: Asset { code: asset.clone(), issuer: None },
            interest_rate_bps: interest_rate_bps as u32,
            total_deposits: 0,
            total_shares: 0,
            initialized: true,
        };
        pools.set(pool_id.clone(), pool.clone());
        Self::set_pools(&env, pools);
        pool
    }

    pub fn deposit(env: Env, pool_id: BytesN<32>, from: Address, amount: i128) {
        from.require_auth();

        if !ValidationHelper::validate_positive_amount(amount) {
            panic!("{:?}", Error::InvalidAmount);
        }

        let mut pools = Self::get_pools(&env);
        let mut pool = pools.get(pool_id.clone()).unwrap_or_else(|| panic!("{:?}", Error::PoolNotFound));

        let shares = if pool.total_shares == 0 || pool.total_deposits == 0 {
            amount
        } else {
            (amount * pool.total_shares) / pool.total_deposits
        };

        pool.total_deposits = SafeMath::add(pool.total_deposits, amount).unwrap_or(pool.total_deposits);
        pool.total_shares = SafeMath::add(pool.total_shares, shares).unwrap_or(pool.total_shares);
        pools.set(pool_id.clone(), pool.clone());
        Self::set_pools(&env, pools);

        let mut positions = Self::get_positions(&env, &pool_id);
        if let Some(index) = positions.iter().position(|position: PoolPosition| position.user == from) {
            let mut existing = positions.get(index as u32).unwrap();
            existing.shares = SafeMath::add(existing.shares, shares).unwrap_or(existing.shares);
            existing.last_accrued_at = env.ledger().timestamp();
            positions.set(index as u32, existing);
        } else {
            positions.push_back(PoolPosition {
                user: from.clone(),
                shares,
                last_accrued_at: env.ledger().timestamp(),
            });
        }
        Self::set_positions(&env, &pool_id, positions);
    }

    pub fn withdraw(env: Env, pool_id: BytesN<32>, to: Address, amount: i128) {
        to.require_auth();

        if !ValidationHelper::validate_positive_amount(amount) {
            panic!("{:?}", Error::InvalidAmount);
        }

        let mut pools = Self::get_pools(&env);
        let mut pool = pools.get(pool_id.clone()).unwrap_or_else(|| panic!("{:?}", Error::PoolNotFound));

        let mut positions = Self::get_positions(&env, &pool_id);
        let index = positions
            .iter()
            .position(|position: PoolPosition| position.user == to)
            .unwrap_or_else(|| panic!("{:?}", Error::NoPositionFound));

        let position = positions.get(index as u32).unwrap();
        if position.shares < amount {
            panic!("{:?}", Error::InsufficientShares);
        }

        pool.total_deposits = SafeMath::sub(pool.total_deposits, amount).unwrap_or(pool.total_deposits);
        pool.total_shares = SafeMath::sub(pool.total_shares, amount).unwrap_or(pool.total_shares);
        pools.set(pool_id.clone(), pool.clone());
        Self::set_pools(&env, pools);

        let mut updated_position = positions.get(index as u32).unwrap();
        updated_position.shares = SafeMath::sub(updated_position.shares, amount).unwrap_or(updated_position.shares);
        updated_position.last_accrued_at = env.ledger().timestamp();
        positions.set(index as u32, updated_position);
        Self::set_positions(&env, &pool_id, positions);
    }

    pub fn get_pool_balance(env: Env, pool_id: BytesN<32>) -> i128 {
        let pools = Self::get_pools(&env);
        pools.get(pool_id).unwrap_or_else(|| panic!("{:?}", Error::PoolNotFound)).total_deposits
    }

    pub fn calculate_interest(env: Env, lender: Address, pool_id: BytesN<32>) -> i128 {
        let pools = Self::get_pools(&env);
        let pool = pools.get(pool_id.clone()).unwrap_or_else(|| panic!("{:?}", Error::PoolNotFound));

        let positions = Self::get_positions(&env, &pool_id);
        let mut matched = None;
        for index in 0..positions.len() {
            let position = positions.get(index as u32).unwrap();
            if position.user == lender {
                matched = Some(position);
                break;
            }
        }
        let Some(position) = matched else {
            return 0;
        };

        let elapsed = env.ledger().timestamp().saturating_sub(position.last_accrued_at);
        ValidationHelper::calculate_interest(position.shares, pool.interest_rate_bps, elapsed).unwrap_or(0)
    }

    fn derive_pool_id(env: &Env, asset: &BytesN<32>) -> BytesN<32> {
        let mut bytes = [0u8; 32];
        let asset_bytes = asset.to_array();
        for (index, byte) in asset_bytes.iter().enumerate() {
            bytes[index % 32] = *byte;
        }
        BytesN::from_array(env, &bytes)
    }

    fn get_pools(env: &Env) -> Map<BytesN<32>, LendingPool> {
        let key = Self::storage_key(env, b"pools");
        env.storage().persistent().get(&key).unwrap_or_else(|| Map::new(env))
    }

    fn set_pools(env: &Env, pools: Map<BytesN<32>, LendingPool>) {
        let key = Self::storage_key(env, b"pools");
        env.storage().persistent().set(&key, &pools);
    }

    fn get_positions(env: &Env, pool_id: &BytesN<32>) -> Vec<PoolPosition> {
        let positions_key = Self::positions_key(env, pool_id);
        env.storage().persistent().get(&positions_key).unwrap_or_else(|| Vec::new(env))
    }

    fn set_positions(env: &Env, pool_id: &BytesN<32>, positions: Vec<PoolPosition>) {
        let positions_key = Self::positions_key(env, pool_id);
        env.storage().persistent().set(&positions_key, &positions);
    }

    fn positions_key(env: &Env, pool_id: &BytesN<32>) -> BytesN<32> {
        let mut bytes = [0u8; 32];
        let pool_bytes = pool_id.to_array();
        for (index, byte) in pool_bytes.iter().enumerate() {
            bytes[index % 32] = *byte;
        }
        BytesN::from_array(env, &bytes)
    }

    fn storage_key(env: &Env, label: &[u8]) -> BytesN<32> {
        let mut bytes = [0u8; 32];
        let label_len = label.len().min(32);
        bytes[..label_len].copy_from_slice(&label[..label_len]);
        BytesN::from_array(env, &bytes)
    }
}
