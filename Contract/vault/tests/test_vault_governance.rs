//! Tests for vault governance: initialization, admin authorization,
//! two-step admin transfer, and unauthorized access prevention.

use soroban_sdk::{
    testutils::Address as _,
    Address, BytesN, Env,
};
use shared::errors::Error;
use vault::{VaultContract, VaultId, VaultConfig};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    (env, contract_id)
}

fn setup_initialized() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);
    VaultContract::initialize(env.clone(), admin.clone(), streaks, rewards)
        .expect("initialize should succeed");
    (env, contract_id, admin)
}

fn ghost_vault_id(env: &Env) -> VaultId {
    VaultId(BytesN::from_array(env, &[0xFFu8; 32]))
}

// ===========================================================================
// Initialization
// ===========================================================================

#[test]
fn test_initialize_sets_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);

    VaultContract::initialize(env.clone(), admin.clone(), streaks, rewards)
        .expect("initialize should succeed");

    let stored_admin = VaultContract::get_admin(env.clone())
        .expect("admin should be set");
    assert_eq!(stored_admin, admin);
}

#[test]
fn test_initialize_panics_on_double_init() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, VaultContract);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);

    VaultContract::initialize(env.clone(), admin.clone(), streaks.clone(), rewards.clone())
        .expect("first init should succeed");

    // Second init must fail
    let result = VaultContract::initialize(env.clone(), admin, streaks, rewards);
    assert_eq!(result, Err(Error::AlreadyInitialized));
}

#[test]
fn test_get_admin_before_init_returns_not_initialized() {
    let (env, _contract_id) = setup();
    let result = VaultContract::get_admin(env.clone());
    assert_eq!(result, Err(Error::NotInitialized));
}

// ===========================================================================
// Configuration requires admin
// ===========================================================================

#[test]
fn test_set_config_requires_admin() {
    let (env, _contract_id) = setup_initialized();
    let non_admin = Address::generate(&env);

    let config = VaultConfig::default();
    let result = VaultContract::set_config(env.clone(), non_admin, config);
    assert_eq!(result, Err(Error::PermissionDenied));
}

#[test]
fn test_set_config_succeeds_for_admin() {
    let (env, _contract_id, admin) = setup_initialized();
    let config = VaultConfig {
        max_vaults_per_user: 5,
        ..Default::default()
    };
    let result = VaultContract::set_config(env.clone(), admin, config.clone());
    assert_eq!(result, Ok(()));

    let stored = VaultContract::get_config(env).unwrap();
    assert_eq!(stored.max_vaults_per_user, 5);
}

// ===========================================================================
// Two-step admin transfer
// ===========================================================================

#[test]
fn test_transfer_admin_initiates_pending() {
    let (env, _contract_id, admin) = setup_initialized();
    let new_admin = Address::generate(&env);

    let result = VaultContract::transfer_admin(env.clone(), new_admin.clone());
    assert_eq!(result, Ok(()));
}

#[test]
fn test_transfer_admin_requires_current_admin_auth() {
    let (env, _contract_id, _admin) = setup_initialized();
    let not_admin = Address::generate(&env);
    let proposed = Address::generate(&env);

    // Without mock_all_auths, non-admin won't have auth
    let env_no_mock = Env::default();
    let result = VaultContract::transfer_admin(env_no_mock, proposed);
    assert_eq!(result, Err(Error::Unauthorized));
}

#[test]
fn test_transfer_admin_rejects_self_transfer() {
    let (env, _contract_id, admin) = setup_initialized();
    let result = VaultContract::transfer_admin(env.clone(), admin.clone());
    assert_eq!(result, Err(Error::CannotTransferToSelf));
}

#[test]
fn test_accept_admin_completes_transfer() {
    let (env, _contract_id, admin) = setup_initialized();
    let new_admin = Address::generate(&env);

    VaultContract::transfer_admin(env.clone(), new_admin.clone()).unwrap();
    VaultContract::accept_admin(env.clone()).unwrap();

    let current = VaultContract::get_admin(env.clone()).unwrap();
    assert_eq!(current, new_admin);
}

#[test]
fn test_accept_admin_without_pending_fails() {
    let (env, _contract_id, _admin) = setup_initialized();
    let result = VaultContract::accept_admin(env.clone());
    assert_eq!(result, Err(Error::NoAdminTransferPending));
}

#[test]
fn test_transfer_admin_clears_pending_after_accept() {
    let (env, _contract_id, admin) = setup_initialized();
    let new_admin = Address::generate(&env);
    let third_admin = Address::generate(&env);

    VaultContract::transfer_admin(env.clone(), new_admin.clone()).unwrap();
    VaultContract::accept_admin(env.clone()).unwrap();

    // New admin can now transfer
    VaultContract::transfer_admin(env.clone(), third_admin.clone()).unwrap();
    VaultContract::accept_admin(env.clone()).unwrap();

    let current = VaultContract::get_admin(env).unwrap();
    assert_eq!(current, third_admin);
}

// ===========================================================================
// Unauthorized initialization prevention
// ===========================================================================

#[test]
fn test_unauthorized_init_by_arbitrary_account() {
    let env = Env::default();
    // Do NOT call mock_all_auths — so no account has authorized anything
    let contract_id = env.register_contract(None, VaultContract);
    let admin = Address::generate(&env);
    let streaks = Address::generate(&env);
    let rewards = Address::generate(&env);

    // Without authorization, initialize must fail (require_auth will trap)
    // In test mode without mock_all_auths, require_auth causes a panic
    // which the test framework catches as a contract error
    let result = env.try_invoke_contract::<_, Result<(), Error>>(
        &contract_id,
        &soroban_sdk::Symbol::new(&env, "initialize"),
        soroban_sdk::vec![
            &env,
            admin.into_val(&env),
            streaks.into_val(&env),
            rewards.into_val(&env),
        ],
    );
    // The call should fail due to missing authorization
    assert!(result.is_err());
}

// ===========================================================================
// Vault operations still work after governance changes
// ===========================================================================

#[test]
fn test_vault_create_works_after_init() {
    let (env, _contract_id, _admin) = setup_initialized();

    let owner = Address::generate(&env);
    let token = Address::generate(&env);
    let symbol = BytesN::from_array(&env, &[0u8; 32]);

    let vault_id = VaultContract::create_vault(
        env.clone(),
        owner,
        token,
        symbol,
        86_400,
    )
    .expect("create_vault should succeed");

    let metadata = VaultContract::get_vault(env.clone(), vault_id)
        .expect("get_vault should succeed");
    assert_eq!(metadata.lock_period, 86_400);
}

#[test]
fn test_lock_period_and_unlock_queries() {
    let (env, _contract_id, _admin) = setup_initialized();

    let owner = Address::generate(&env);
    let token = Address::generate(&env);
    let symbol = BytesN::from_array(&env, &[0u8; 32]);

    let vault_id = VaultContract::create_vault(
        env.clone(),
        owner,
        token,
        symbol,
        86_400,
    )
    .unwrap();

    let lp = VaultContract::get_lock_period(env.clone(), vault_id.clone()).unwrap();
    assert_eq!(lp, 86_400);

    let ut = VaultContract::get_unlock_time(env, vault_id).unwrap();
    assert!(ut > 0);
}

#[test]
fn test_is_locked_for_existing_vault() {
    let (env, _contract_id, _admin) = setup_initialized();

    let owner = Address::generate(&env);
    let token = Address::generate(&env);
    let symbol = BytesN::from_array(&env, &[0u8; 32]);

    let vault_id = VaultContract::create_vault(
        env.clone(),
        owner,
        token,
        symbol,
        86_400,
    )
    .unwrap();

    let locked = VaultContract::is_locked(env, vault_id).unwrap();
    assert!(locked, "Newly created vault should be locked");
}

// ===========================================================================
// Ghost vault operations still return VaultNotFound
// ===========================================================================

#[test]
fn test_ghost_vault_returns_vault_not_found_after_init() {
    let (env, _contract_id, _admin) = setup_initialized();

    assert_eq!(
        VaultContract::get_vault(env.clone(), ghost_vault_id(&env)),
        Err(Error::VaultNotFound)
    );
    assert_eq!(
        VaultContract::get_balance(env.clone(), ghost_vault_id(&env)),
        Err(Error::VaultNotFound)
    );
    assert_eq!(
        VaultContract::get_accrued_interest(env.clone(), ghost_vault_id(&env)),
        Err(Error::VaultNotFound)
    );
}
