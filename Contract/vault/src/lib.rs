#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, Vec,
};
use shared::{
    errors::Error,
    events::{VaultCreated, VaultUnlocked},
    types::{Asset, VaultMetadata, VaultStatus, EmergencyStop, RateLimit, Role, Permission},
    utils::{SafeMath, TimeHelper, ValidationHelper, FixedMath},
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

/// Storage keys for vault contract
#[derive(Clone)]
#[contracttype]
pub enum VaultKey {
    Vault(VaultId),
    Balance(VaultId),
    VaultCounter,
    EmergencyStop,
    RateLimit,
    AdminPermissions(Address),
    UserVaults(Address),
    VaultInterest(VaultId),
#[cfg(test)]
pub use VaultContract;

/// Storage keys - initialized at runtime
fn vaults_key(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

fn balances_key(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[1u8; 32])
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultConfig {
    pub max_vaults_per_user: u64,
    pub min_lock_period: u64,
    pub max_lock_period: u64,
    pub interest_rate: i128, // Basis points
    pub auto_compound: bool,
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
    ) -> Result<VaultId, Error> {
        owner.require_auth();

        // Check emergency stop
        Self::check_emergency_stop(&env)?;

        // Check rate limit
        Self::check_rate_limit(&env)?;

        // Get vault config
        let config: VaultConfig = env
            .storage()
            .persistent()
            .get(&VaultKey::VaultCounter)
            .unwrap_or(VaultConfig {
                max_vaults_per_user: 10,
                min_lock_period: 1,
                max_lock_period: 157_788_000, // 5 years
                interest_rate: 500, // 5%
                auto_compound: true,
            });

        // Validate lock period
        if lock_period < config.min_lock_period || lock_period > config.max_lock_period {
            return Err(Error::InvalidLockPeriod);
        }

        // Check user vault limit
        let user_vaults_key = VaultKey::UserVaults(owner.clone());
        let user_vaults: Vec<VaultId> = env
            .storage()
            .persistent()
            .get(&user_vaults_key)
            .unwrap_or(Vec::new(&env));
        if user_vaults.len() as u64 >= config.max_vaults_per_user {
            return Err(Error::InvalidParameters);
        }

        // Generate vault ID
        let counter_key = VaultKey::VaultCounter;
        let counter: u64 = env.storage().persistent().get(&counter_key).unwrap_or(0);
        let new_counter = counter.checked_add(1).ok_or(Error::Overflow)?;
        env.storage().persistent().set(&counter_key, &new_counter);
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
        let unlock_time = now.checked_add(lock_period).ok_or(Error::Overflow)?;
        let unlock_time = now.checked_add(lock_period).unwrap();

        let asset = Asset {
            token: token_contract,
            symbol: symbol.clone(),
            code: asset_code.clone(),
            issuer: asset_issuer,
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

        // Store vault metadata
        env.storage()
            .persistent()
            .set(&VaultKey::Vault(vault_id.clone()), &metadata);

        // Initialize balance to zero
        env.storage()
            .persistent()
            .set(&VaultKey::Balance(vault_id.clone()), &0i128);

        // Initialize interest tracking
        env.storage()
            .persistent()
            .set(&VaultKey::VaultInterest(vault_id.clone()), &0i128);

        // Add to user's vaults
        let mut updated_user_vaults = user_vaults;
        updated_user_vaults.push_back(vault_id.clone());
        env.storage()
            .persistent()
            .set(&user_vaults_key, &updated_user_vaults);
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
            (VaultCreated::topic(&env), vault_id_bytes.clone()),
            VaultCreated {
                vault_id: vault_id_bytes.clone(),
                owner,
                asset: asset_code.clone(),
                lock_period,
            },
            (VaultCreated {
                vault_id: vault_id_bytes,
                owner,
                asset: symbol.clone(),
                lock_period,
            },),
            (vault_id_bytes.clone(), owner, asset_code, lock_period),
            (),
        );

        Ok(vault_id)
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
    pub fn deposit(env: Env, vault_id: VaultId, from: Address, amount: i128) -> Result<(), Error> {
        from.require_auth();

        // Check emergency stop
        Self::check_emergency_stop(&env)?;

        // Validate amount
        if !ValidationHelper::validate_positive_amount(amount) {
            return Err(Error::InvalidAmount);
        }

        // Check vault exists
        let metadata: VaultMetadata = env
            .storage()
            .persistent()
            .get(&VaultKey::Vault(vault_id.clone()))
            .ok_or(Error::VaultNotFound)?;

        // Accrue interest before deposit
        Self::accrue_interest(env.clone(), vault_id.clone())?;
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

        // Update balance using safe arithmetic
        let current_balance: i128 = env
            .storage()
            .persistent()
            .get(&VaultKey::Balance(vault_id.clone()))
            .ok_or(Error::VaultNotFound)?;
        let new_balance = SafeMath::add(current_balance, amount).ok_or(Error::Overflow)?;
        env.storage()
            .persistent()
            .set(&VaultKey::Balance(vault_id.clone()), &new_balance);
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
            (vault_id.0.clone(), from, amount),
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
            (shared::events::DepositMade::topic(&env), vault_id.0.clone()),
            shared::events::DepositMade {
                vault_id: vault_id.0,
                depositor: from,
                amount,
            },
            ("deposit_made", from.clone()),
            DepositMade {
                vault_id: vault_id.0.clone(),
                depositor: from.clone(),
                amount,
            }
        );

        Ok(())
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
    pub fn withdraw(env: Env, vault_id: VaultId, to: Address, amount: i128) -> Result<(), Error> {
        // Get vault metadata
        let mut metadata: VaultMetadata = env
            .storage()
            .persistent()
            .get(&VaultKey::Vault(vault_id.clone()))
            .ok_or(Error::VaultNotFound)?;
    pub fn withdraw(env: Env, vault_id: VaultId, to: Address, amount: i128) {
        let vaults_key = vaults_key(&env);
        let mut vaults_map: Map<VaultId, VaultMetadata> = env
            .storage()
            .persistent()
            .get(&vaults_key)
            .expect("Vault not found");
        let mut metadata = vaults_map.get(vault_id.clone()).expect("Vault not found");

        metadata.owner.require_auth();

        // Check emergency stop
        Self::check_emergency_stop(&env)?;

        // Validate amount
        if !ValidationHelper::validate_positive_amount(amount) {
            return Err(Error::InvalidAmount);
        }

        // Accrue interest before withdrawal
        Self::accrue_interest(env.clone(), vault_id.clone())?;

        // Check lock period
        if metadata.status == VaultStatus::Locked {
            if !TimeHelper::is_past(&env, metadata.unlock_time) {
                return Err(Error::VaultLocked);
        if !ValidationHelper::validate_positive_amount(amount) {
            panic!("{:?}", Error::InvalidAmount);
        }

        if metadata.status == VaultStatus::Locked {
            if !TimeHelper::is_past(&env, metadata.unlock_time) {
                panic!("{:?}", Error::VaultLocked);
            }
            metadata.status = VaultStatus::Unlocked;
            env.storage()
                .persistent()
                .set(&VaultKey::Vault(vault_id.clone()), &metadata.clone());

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
            (shared::events::WithdrawalCompleted::topic(&env), vault_id.0.clone()),
            shared::events::WithdrawalCompleted {
                vault_id: vault_id.0,
                withdrawer: to,
                amount,
            },
            (vault_id.0.clone(), to, amount),
            (),
        );

        Ok(())
    }

    /// Get the balance of a vault
    ///
    /// # Arguments
    /// * `vault_id` - The vault to query
    ///
    /// # Returns
    /// The current balance of the vault
    pub fn get_balance(env: Env, vault_id: VaultId) -> Result<i128, Error> {
        let balance: i128 = env
            .storage()
            .persistent()
            .get(&VaultKey::Balance(vault_id))
            .ok_or(Error::VaultNotFound)?;
        Ok(balance)
    }

    /// Get the metadata of a vault
    ///
    /// # Arguments
    /// * `vault_id` - The vault to query
    ///
    /// # Returns
    /// The vault metadata
    pub fn get_vault(env: Env, vault_id: VaultId) -> Result<VaultMetadata, Error> {
        let metadata: VaultMetadata = env
            .storage()
            .persistent()
            .get(&VaultKey::Vault(vault_id))
            .ok_or(Error::VaultNotFound)?;
        Ok(metadata)
    }

    /// Get user's vaults
    ///
    /// # Arguments
    /// * `user` - The user address
    ///
    /// # Returns
    /// List of vault IDs owned by the user
    pub fn get_user_vaults(env: Env, user: Address) -> Result<Vec<VaultId>, Error> {
        let user_vaults: Vec<VaultId> = env
            .storage()
            .persistent()
            .get(&VaultKey::UserVaults(user))
            .unwrap_or(Vec::new(&env));
        Ok(user_vaults)
    }

    /// Accrue interest for a vault
    fn accrue_interest(env: Env, vault_id: VaultId) -> Result<(), Error> {
        let config: VaultConfig = env
            .storage()
            .persistent()
            .get(&VaultKey::VaultCounter)
            .unwrap_or(VaultConfig {
                max_vaults_per_user: 10,
                min_lock_period: 1,
                max_lock_period: 157_788_000,
                interest_rate: 500,
                auto_compound: true,
            });

        let metadata: VaultMetadata = env
            .storage()
            .persistent()
            .get(&VaultKey::Vault(vault_id.clone()))
            .ok_or(Error::VaultNotFound)?;

        let balance: i128 = env
            .storage()
            .persistent()
            .get(&VaultKey::Balance(vault_id.clone()))
            .ok_or(Error::VaultNotFound)?;

        if balance == 0 || config.interest_rate == 0 {
            return Ok(());
        }

        let now = TimeHelper::now(&env);
        let elapsed = now.saturating_sub(metadata.created_at);

        if elapsed == 0 {
            return Ok(());
        }

        // Calculate interest: balance * rate * time / (seconds_per_year * 10000)
        let seconds_per_year = 31_536_000i128;
        let interest = FixedMath::calculate_interest(
            balance,
            FixedMath::basis_points_to_fixed(config.interest_rate) / seconds_per_year,
            elapsed as i128,
        ).ok_or(Error::Overflow)?;

        if interest > 0 && config.auto_compound {
            let new_balance = SafeMath::add(balance, interest).ok_or(Error::Overflow)?;
            env.storage()
                .persistent()
                .set(&VaultKey::Balance(vault_id.clone()), &new_balance);
            env.storage()
                .persistent()
                .set(&VaultKey::VaultInterest(vault_id.clone()), &interest);
        }

        Ok(())
    }

    /// Check emergency stop status
    fn check_emergency_stop(env: &Env) -> Result<(), Error> {
        if let Some(emergency_stop) = env
            .storage()
            .persistent()
            .get::<_, EmergencyStop>(&VaultKey::EmergencyStop)
        {
            if emergency_stop.active {
                return Err(Error::EmergencyStopActive);
            }
        }
        Ok(())
    }

    /// Check and update rate limit
    fn check_rate_limit(env: &Env) -> Result<(), Error> {
        let mut rate_limit: RateLimit = env
            .storage()
            .persistent()
            .get(&VaultKey::RateLimit)
            .unwrap_or(RateLimit::new(1000, 3600));

        let now = TimeHelper::now(env);

        if now >= rate_limit.period_start + rate_limit.period_seconds {
            rate_limit.current_count = 0;
            rate_limit.period_start = now;
        }

        if rate_limit.current_count >= rate_limit.max_operations_per_period {
            return Err(Error::RateLimitExceeded);
        }

        rate_limit.current_count = rate_limit.current_count.checked_add(1).ok_or(Error::Overflow)?;
        env.storage().persistent().set(&VaultKey::RateLimit, &rate_limit);

        Ok(())
    }

    /// Trigger emergency stop
    pub fn trigger_emergency_stop(env: Env, admin: Address, reason: BytesN<32>) -> Result<(), Error> {
        if !Self::is_admin(&env, &admin) {
            return Err(Error::PermissionDenied);
        }

        let emergency_stop = EmergencyStop {
            active: true,
            triggered_by: admin,
            triggered_at: TimeHelper::now(&env),
            reason,
        };

        env.storage().persistent().set(&VaultKey::EmergencyStop, &emergency_stop);
        Ok(())
    }

    /// Lift emergency stop
    pub fn lift_emergency_stop(env: Env, admin: Address) -> Result<(), Error> {
        if !Self::is_admin(&env, &admin) {
            return Err(Error::PermissionDenied);
        }

        env.storage().persistent().remove(&VaultKey::EmergencyStop);
        Ok(())
    }

    /// Grant admin permission
    pub fn grant_admin(env: Env, admin: Address) -> Result<(), Error> {
        let permission = Permission {
            role: Role::Admin,
            granted_at: TimeHelper::now(&env),
            expires_at: None,
        };
        env.storage()
            .persistent()
            .set(&VaultKey::AdminPermissions(admin), &permission);
        Ok(())
    }

    /// Check if address is admin
    fn is_admin(env: &Env, address: &Address) -> bool {
        if let Some(permission) = env
            .storage()
            .persistent()
            .get::<_, Permission>(&VaultKey::AdminPermissions(address.clone()))
        {
            permission.role == Role::Admin
        } else {
            false
        }
    }

    /// Set vault configuration
    pub fn set_config(env: Env, admin: Address, config: VaultConfig) -> Result<(), Error> {
        if !Self::is_admin(&env, &admin) {
            return Err(Error::PermissionDenied);
        }
        env.storage().persistent().set(&VaultKey::VaultCounter, &config);
        Ok(())
    }

    /// Get vault configuration
    pub fn get_config(env: Env) -> Result<VaultConfig, Error> {
        let config: VaultConfig = env
            .storage()
            .persistent()
            .get(&VaultKey::VaultCounter)
            .unwrap_or(VaultConfig {
                max_vaults_per_user: 10,
                min_lock_period: 1,
                max_lock_period: 157_788_000,
                interest_rate: 500,
                auto_compound: true,
            });
        Ok(config)
    }

    /// Helper function to generate a vault ID from a counter
    fn generate_vault_id(env: &Env, counter: u64) -> BytesN<32> {
        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&counter.to_be_bytes());
        BytesN::from_array(env, &bytes)
    }
}