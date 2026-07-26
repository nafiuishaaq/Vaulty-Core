#![no_std]
use soroban_sdk::{
    token,
    contract, contractimpl, contracttype, Address, BytesN, Env, Map,
};
use shared::{
    errors::Error,
    events::{DepositMade, VaultCreated, VaultUnlocked, WithdrawalCompleted},
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, Map, IntoVal, Symbol, Vec,
};
use shared::{
    errors::Error,
    types::{Asset, VaultMetadata, VaultStatus},
    utils::{SafeMath, TimeHelper, ValidationHelper},
};

/// Vault contract for managing savings vaults with time-locked deposits
#[contract]
pub struct VaultContract;

#[cfg(test)]
pub use VaultContract;

/// Storage keys - initialized at runtime
fn vaults_key(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

fn balances_key(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[1u8; 32])
}

fn vault_counter_key(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[2u8; 32])
}

fn streaks_contract_key(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[3u8; 32])
}

fn rewards_contract_key(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[4u8; 32])
}

#[contracttype]
#[derive(Clone)]
pub struct VaultId(BytesN<32>);

#[contractimpl]
impl VaultContract {
    /// Initialize the vault contract with linked streaks and rewards contracts
    /// Can only be called once
    pub fn initialize(env: Env, streaks_contract: Address, rewards_contract: Address) {
        // Check if already initialized
        let streaks_key = streaks_contract_key(&env);
        if env.storage().instance().has(&streaks_key) {
            panic!("{:?}", Error::AlreadyInitialized);
        }

        // Store the contract addresses
        env.storage().instance().set(&streaks_key, &streaks_contract);
        let rewards_key = rewards_contract_key(&env);
        env.storage().instance().set(&rewards_key, &rewards_contract);
    }

    /// Add streaks contract as an authorized caller in the streaks contract
    /// (Called during setup)
    pub fn register_with_streaks(env: Env) {
        let streaks_key = streaks_contract_key(&env);
        let streaks_contract: Address = env.storage().instance().get(&streaks_key).expect("Streaks contract not initialized");

        let mut args = Vec::new(&env);
        args.push_back(env.current_contract_address().into_val(&env));

        // Invoke add_authorized_caller on streaks contract to add vault as authorized
        env.invoke_contract::<()>(
            &streaks_contract,
            &Symbol::new(&env, "add_authorized_caller"),
            args,
        );
    }

    /// Create a new vault with the specified asset and lock period
    ///
    /// # Arguments
    /// * `owner` - The address that will own this vault
    /// * `token_contract` - The token contract address for asset identity
    /// * `symbol` - Human-readable asset symbol for indexing
    /// * `lock_period` - Lock period in seconds (min 1, max 5 years)
    ///
    /// # Returns
    /// The unique vault ID
    ///
    /// # Auth
    /// Requires authorization from the owner
    pub fn create_vault(
        env: Env,
        owner: Address,
        token_contract: Address,
        symbol: BytesN<32>,
        lock_period: u64,
    ) -> VaultId {
        owner.require_auth();

        if !ValidationHelper::validate_lock_period(lock_period) {
            panic!("{:?}", Error::InvalidLockPeriod);
        }

        let counter_key = vault_counter_key(&env);
        let counter: u64 = env.storage().instance().get(&counter_key).unwrap_or(0);
        let new_counter = counter.checked_add(1).unwrap();
        env.storage().instance().set(&counter_key, &new_counter);

        let vault_id_bytes = Self::generate_vault_id(&env, new_counter);
        let vault_id = VaultId(vault_id_bytes.clone());

        let now = TimeHelper::now(&env);
        let unlock_time = now.checked_add(lock_period).unwrap();

        let asset = Asset {
            token: token_contract,
            symbol: symbol.clone(),
            code: asset_code.clone(),
            issuer: asset_issuer.unwrap_or_else(|| owner.clone()),
        };

        let metadata = VaultMetadata {
            owner: owner.clone(),
            asset,
            lock_period,
            created_at: now,
            unlock_time,
            status: VaultStatus::Locked,
        };

        let vaults_key = vaults_key(&env);
        let mut vaults_map: Map<VaultId, VaultMetadata> = env
            .storage()
            .persistent()
            .get(&vaults_key)
            .unwrap_or_else(|| Map::new(&env));
        vaults_map.set(vault_id.clone(), metadata);
        env.storage().persistent().set(&vaults_key, &vaults_map);

        let balances_key = balances_key(&env);
        let mut balances_map: Map<VaultId, i128> = env
            .storage()
            .persistent()
            .get(&balances_key)
            .unwrap_or_else(|| Map::new(&env));
        balances_map.set(vault_id.clone(), 0i128);
        env.storage().persistent().set(&balances_key, &balances_map);

        env.events().publish(
            (VaultCreated {
                vault_id: vault_id_bytes,
                owner,
                asset: symbol.clone(),
                lock_period,
            },),
            (vault_id_bytes.clone(), owner, asset_code, lock_period),
            (),
        );

        vault_id
    }

    /// Deposit tokens into a vault
    ///
    /// Transfers tokens from the depositor to the vault contract before updating the internal balance.
    /// If the token transfer fails, the state is reverted automatically.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to deposit into
    /// * `from` - The address depositing funds
    /// * `amount` - The amount to deposit (must be positive)
    ///
    /// # Auth
    /// Requires authorization from the depositor
    pub fn deposit(env: Env, vault_id: VaultId, from: Address, amount: i128) {
        from.require_auth();

        if !ValidationHelper::validate_positive_amount(amount) {
            panic!("{:?}", Error::InvalidAmount);
        }

        let vaults_key = vaults_key(&env);
        let vaults_map: Map<VaultId, VaultMetadata> = env
            .storage()
            .persistent()
            .get(&vaults_key)
            .expect("Vault not found");
        let metadata = vaults_map.get(vault_id.clone()).expect("Vault not found");

        let token_client = token::Client::new(&env, &metadata.asset.token);
        token_client.transfer(&from, &env.current_contract_address(), &amount);
        let vaults_map: Map<VaultId, VaultMetadata> = env.storage().persistent().get(&vaults_key).expect("Vault not found");
        let metadata = vaults_map.get(vault_id.clone()).expect("Vault not found");

        let balances_key = balances_key(&env);
        let mut balances_map: Map<VaultId, i128> = env
            .storage()
            .persistent()
            .get(&balances_key)
            .expect("Balance not found");
        let current_balance = balances_map.get(vault_id.clone()).expect("Balance not found");

        let new_balance: i128 = SafeMath::add(current_balance, amount as i128)
            .expect("Overflow");

        balances_map.set(vault_id.clone(), new_balance);
        env.storage().persistent().set(&balances_key, &balances_map);

        env.events().publish(
            (DepositMade {
                vault_id: vault_id.0.clone(),
                depositor: from,
                asset: metadata.asset.symbol,
                amount,
            },),
            (),
        // Update user's streak in streaks contract if it's initialized
        let streaks_key = streaks_contract_key(&env);
        if let Some(streaks_contract) = env.storage().instance().get::<BytesN<32>, Address>(&streaks_key) {
            // Call update_streak on the streaks contract
            // This will panic if called twice in the same day, but that's okay - it prevents duplicate streak increments
            let mut args = Vec::new(&env);
            args.push_back(metadata.owner.clone().into_val(&env));
            let result = env.try_invoke_contract::<(), shared::errors::Error>(
                &streaks_contract,
                &Symbol::new(&env, "update_streak"),
                args,
            );
            // If the call succeeds, also try to notify rewards contract if a milestone was reached
            if result.is_ok() {
                // Get the updated streak count
                let mut get_streak_args = Vec::new(&env);
                get_streak_args.push_back(metadata.owner.clone().into_val(&env));
                let streak_count: u32 = env.invoke_contract(
                    &streaks_contract,
                    &Symbol::new(&env, "get_streak"),
                    get_streak_args,
                );

                // Call rewards contract to check for milestones
                let rewards_key = rewards_contract_key(&env);
                if let Some(rewards_contract) = env.storage().instance().get::<BytesN<32>, Address>(&rewards_key) {
                    // Invoke grant_reward which will check if any milestones are met
                    let mut grant_args = Vec::new(&env);
                    grant_args.push_back(metadata.owner.clone().into_val(&env));
                    grant_args.push_back(streak_count.into_val(&env));
                    let _ = env.try_invoke_contract::<(), shared::errors::Error>(
                        &rewards_contract,
                        &Symbol::new(&env, "grant_reward"),
                        grant_args,
                    );
                }
            }
        }

        // Emit deposit event using shared event type
        use shared::events::DepositMade;
        env.events().publish(
            ("deposit_made", from.clone()),
            DepositMade {
                vault_id: vault_id.0.clone(),
                depositor: from.clone(),
                amount,
            }
        );
    }

    /// Withdraw tokens from a vault (only after lock period expires)
    ///
    /// Checks ownership and lock period before transferring tokens.
    /// Transfers tokens from vault custody to the recipient only after checks pass.
    ///
    /// # Arguments
    /// * `vault_id` - The vault to withdraw from
    /// * `to` - The address to receive funds
    /// * `amount` - The amount to withdraw (must be positive and <= balance)
    ///
    /// # Auth
    /// Requires authorization from the vault owner
    pub fn withdraw(env: Env, vault_id: VaultId, to: Address, amount: i128) {
        let vaults_key = vaults_key(&env);
        let mut vaults_map: Map<VaultId, VaultMetadata> = env
            .storage()
            .persistent()
            .get(&vaults_key)
            .expect("Vault not found");
        let mut metadata = vaults_map.get(vault_id.clone()).expect("Vault not found");

        metadata.owner.require_auth();

        if !ValidationHelper::validate_positive_amount(amount) {
            panic!("{:?}", Error::InvalidAmount);
        }

        if metadata.status == VaultStatus::Locked {
            if !TimeHelper::is_past(&env, metadata.unlock_time) {
                panic!("{:?}", Error::VaultLocked);
            }
            metadata.status = VaultStatus::Unlocked;
            vaults_map.set(vault_id.clone(), metadata.clone());
            env.storage().persistent().set(&vaults_key, &vaults_map);

            env.events().publish(
                (VaultUnlocked {
                    vault_id: vault_id.0.clone(),
                    asset: metadata.asset.symbol.clone(),
                    unlock_time: metadata.unlock_time,
                },),
                (vault_id.0.clone(), metadata.unlock_time),
                (),
            );
        }

        let balances_key = balances_key(&env);
        let mut balances_map: Map<VaultId, i128> = env
            .storage()
            .persistent()
            .get(&balances_key)
            .expect("Balance not found");
        let current_balance = balances_map.get(vault_id.clone()).expect("Balance not found");
        if amount > current_balance {
            panic!("{:?}", Error::InsufficientBalance);
        }

        let new_balance: i128 = SafeMath::sub(current_balance, amount as i128)
            .expect("Underflow");
        balances_map.set(vault_id.clone(), new_balance);
        env.storage().persistent().set(&balances_key, &balances_map);

        let token_client = token::Client::new(&env, &metadata.asset.token);
        token_client.transfer(&env.current_contract_address(), &to, &amount);

        env.events().publish(
            (WithdrawalCompleted {
                vault_id: vault_id.0.clone(),
                withdrawer: to,
                asset: metadata.asset.symbol,
                amount,
            },),
            (vault_id.0.clone(), to, amount),
            (),
        );
    }

    /// Get the balance of a vault
    ///
    /// # Arguments
    /// * `vault_id` - The vault to query
    ///
    /// # Returns
    /// The current balance of the vault
    pub fn get_balance(env: Env, vault_id: VaultId) -> i128 {
        let balances_key = balances_key(&env);
        let balances_map: Map<VaultId, i128> = env
            .storage()
            .persistent()
            .get(&balances_key)
            .expect("Vault not found");
        balances_map.get(vault_id).expect("Vault not found")
    }

    /// Get the metadata of a vault
    ///
    /// # Arguments
    /// * `vault_id` - The vault to query
    ///
    /// # Returns
    /// The vault metadata
    pub fn get_vault(env: Env, vault_id: VaultId) -> VaultMetadata {
        let vaults_key = vaults_key(&env);
        let vaults_map: Map<VaultId, VaultMetadata> = env
            .storage()
            .persistent()
            .get(&vaults_key)
            .expect("Vault not found");
        vaults_map.get(vault_id).expect("Vault not found")
    }

    /// Get the lock period of a vault
    ///
    /// # Arguments
    /// * `vault_id` - The vault to query
    ///
    /// # Returns
    /// The lock period in seconds
    pub fn get_lock_period(env: Env, vault_id: VaultId) -> u64 {
        let vaults_key = vaults_key(&env);
        let vaults_map: Map<VaultId, VaultMetadata> = env
            .storage()
            .persistent()
            .get(&vaults_key)
            .expect("Vault not found");
        let metadata = vaults_map.get(vault_id).expect("Vault not found");
        metadata.lock_period
    }

    /// Get the unlock time of a vault
    ///
    /// # Arguments
    /// * `vault_id` - The vault to query
    ///
    /// # Returns
    /// The timestamp when the vault unlocks
    pub fn get_unlock_time(env: Env, vault_id: VaultId) -> u64 {
        let vaults_key = vaults_key(&env);
        let vaults_map: Map<VaultId, VaultMetadata> = env
            .storage()
            .persistent()
            .get(&vaults_key)
            .expect("Vault not found");
        let metadata = vaults_map.get(vault_id).expect("Vault not found");
        metadata.unlock_time
    }

    /// Check if a vault is currently locked
    ///
    /// # Arguments
    /// * `vault_id` - The vault to query
    ///
    /// # Returns
    /// True if the vault is locked, false otherwise
    pub fn is_locked(env: Env, vault_id: VaultId) -> bool {
        let vaults_key = vaults_key(&env);
        let vaults_map: Map<VaultId, VaultMetadata> = env
            .storage()
            .persistent()
            .get(&vaults_key)
            .expect("Vault not found");
        let metadata = vaults_map.get(vault_id).expect("Vault not found");
        if metadata.status == VaultStatus::Locked {
            !TimeHelper::is_past(&env, metadata.unlock_time)
        } else {
            false
        }
    }

    /// Helper function to generate a vault ID from a counter
    fn generate_vault_id(env: &Env, counter: u64) -> BytesN<32> {
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&counter.to_be_bytes());
        BytesN::from_array(env, &bytes)
    }
}