//! Tests for borrowing-contract authorization on the lending pool.
//!
//! Verifies that only the configured borrowing contract may call `borrow`,
//! `repay`, and `update_debt`, while read-only endpoints remain public.

use lending::{LendingContract, LendingContractClient};
use shared::errors::Error;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env};

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

struct AuthFixture<'a> {
    env: &'a Env,
    client: LendingContractClient<'a>,
    pool_id: BytesN<32>,
    admin: Address,
    lender: Address,
    borrower: Address,
}

fn setup<'a>(env: &'a Env) -> AuthFixture<'a> {
    let contract_id = env.register_contract(None, LendingContract);
    let client = LendingContractClient::new(env, &contract_id);

    let admin = Address::generate(env);
    let asset = BytesN::from_array(env, &[2u8; 32]);
    let lender = Address::generate(env);
    let borrower = Address::generate(env);

    // Create pool with 5% APR.
    client.create_pool(&admin, &asset, &500i128);

    AuthFixture {
        env,
        client,
        pool_id: asset,
        admin,
        lender,
        borrower,
    }
}

/// Register a mock contract and return its address (used as the borrowing
/// contract).
fn mock_borrowing_contract(env: &Env) -> Address {
    env.register_contract_wasm(None, lending::WASM)
}

// ===========================================================================
// initialize_borrowing_contract
// ===========================================================================

#[test]
fn initialize_succeeds_for_pool_admin() {
    let env = Env::default();
    let f = setup(&env);
    let borrowing_addr = mock_borrowing_contract(&env);

    f.client
        .initialize_borrowing_contract(&f.pool_id, &f.admin, &borrowing_addr);

    let stored = f.client.get_borrowing_contract(&f.pool_id);
    assert_eq!(stored, borrowing_addr);
}

#[test]
fn initialize_fails_for_non_admin() {
    let env = Env::default();
    let f = setup(&env);
    let not_admin = Address::generate(&env);
    let borrowing_addr = mock_borrowing_contract(&env);

    let result = f.client.try_initialize_borrowing_contract(
        &f.pool_id,
        &not_admin,
        &borrowing_addr,
    );
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn initialize_fails_for_nonexistent_pool() {
    let env = Env::default();
    let f = setup(&env);
    let fake_pool = BytesN::from_array(&env, &[99u8; 32]);
    let borrowing_addr = mock_borrowing_contract(&env);

    let result = f.client.try_initialize_borrowing_contract(
        &fake_pool,
        &f.admin,
        &borrowing_addr,
    );
    assert_eq!(result, Err(Ok(Error::PoolNotFound)));
}

#[test]
fn initialize_immutable_after_first_call() {
    let env = Env::default();
    let f = setup(&env);
    let addr1 = mock_borrowing_contract(&env);
    let addr2 = mock_borrowing_contract(&env);

    f.client
        .initialize_borrowing_contract(&f.pool_id, &f.admin, &addr1);

    // Second initialization must be rejected.
    let result = f.client.try_initialize_borrowing_contract(
        &f.pool_id,
        &f.admin,
        &addr2,
    );
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));

    // The original address must be preserved.
    let stored = f.client.get_borrowing_contract(&f.pool_id);
    assert_eq!(stored, addr1);
}

// ===========================================================================
// Borrow authorization
// ===========================================================================

#[test]
fn authorized_borrowing_contract_can_borrow() {
    let env = Env::default();
    let f = setup(&env);
    let borrowing_addr = mock_borrowing_contract(&env);

    f.client
        .initialize_borrowing_contract(&f.pool_id, &f.admin, &borrowing_addr);

    // Seed the pool with liquidity.
    f.client.deposit(&f.pool_id, &f.lender, &10_000i128);

    // Borrow as the authorized contract.
    env.as_contract(&borrowing_addr, || {
        f.client.borrow(&f.pool_id, &f.borrower, &3_000i128);
    });

    let accounting = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(accounting.outstanding_debt, 3_000);
    assert_eq!(accounting.available_liquidity, 7_000);
}

#[test]
fn direct_user_borrow_rejected() {
    let env = Env::default();
    let f = setup(&env);
    let borrowing_addr = mock_borrowing_contract(&env);

    f.client
        .initialize_borrowing_contract(&f.pool_id, &f.admin, &borrowing_addr);
    f.client.deposit(&f.pool_id, &f.lender, &10_000i128);

    // A direct call (not through the borrowing contract) must fail.
    let result = f.client.try_borrow(&f.pool_id, &f.borrower, &1_000i128);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn borrow_rejected_when_no_contract_configured() {
    let env = Env::default();
    let f = setup(&env);

    f.client.deposit(&f.pool_id, &f.lender, &10_000i128);

    // No borrowing contract initialized — must be rejected.
    let result = f.client.try_borrow(&f.pool_id, &f.borrower, &1_000i128);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

#[test]
fn borrow_rejected_for_wrong_contract() {
    let env = Env::default();
    let f = setup(&env);
    let correct_addr = mock_borrowing_contract(&env);
    let wrong_addr = mock_borrowing_contract(&env);

    f.client
        .initialize_borrowing_contract(&f.pool_id, &f.admin, &correct_addr);
    f.client.deposit(&f.pool_id, &f.lender, &10_000i128);

    // Calling from the wrong contract must fail.
    env.as_contract(&wrong_addr, || {
        let result = f.client.try_borrow(&f.pool_id, &f.borrower, &1_000i128);
        assert_eq!(result, Err(Ok(Error::Unauthorized)));
    });
}

// ===========================================================================
// Repay authorization
// ===========================================================================

#[test]
fn authorized_borrowing_contract_can_repay() {
    let env = Env::default();
    let f = setup(&env);
    let borrowing_addr = mock_borrowing_contract(&env);

    f.client
        .initialize_borrowing_contract(&f.pool_id, &f.admin, &borrowing_addr);
    f.client.deposit(&f.pool_id, &f.lender, &10_000i128);

    // Borrow first.
    env.as_contract(&borrowing_addr, || {
        f.client.borrow(&f.pool_id, &f.borrower, &5_000i128);
    });

    // Repay as the authorized contract.
    env.as_contract(&borrowing_addr, || {
        f.client.repay(&f.pool_id, &f.borrower, &2_000i128, &100i128);
    });

    let accounting = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(accounting.outstanding_debt, 3_000);
    assert_eq!(accounting.accrued_interest, 100);
}

#[test]
fn direct_user_repay_rejected() {
    let env = Env::default();
    let f = setup(&env);
    let borrowing_addr = mock_borrowing_contract(&env);

    f.client
        .initialize_borrowing_contract(&f.pool_id, &f.admin, &borrowing_addr);
    f.client.deposit(&f.pool_id, &f.lender, &10_000i128);

    env.as_contract(&borrowing_addr, || {
        f.client.borrow(&f.pool_id, &f.borrower, &5_000i128);
    });

    // Direct repay must fail.
    let result = f.client.try_repay(&f.pool_id, &f.borrower, &1_000i128, &50i128);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

// ===========================================================================
// update_debt authorization
// ===========================================================================

#[test]
fn authorized_borrowing_contract_can_update_debt() {
    let env = Env::default();
    let f = setup(&env);
    let borrowing_addr = mock_borrowing_contract(&env);

    f.client
        .initialize_borrowing_contract(&f.pool_id, &f.admin, &borrowing_addr);
    f.client.deposit(&f.pool_id, &f.lender, &10_000i128);

    env.as_contract(&borrowing_addr, || {
        f.client.update_debt(&f.pool_id, &4_000i128);
    });

    let accounting = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(accounting.outstanding_debt, 4_000);
    assert_eq!(accounting.available_liquidity, 6_000);
}

#[test]
fn direct_user_update_debt_rejected() {
    let env = Env::default();
    let f = setup(&env);
    let borrowing_addr = mock_borrowing_contract(&env);

    f.client
        .initialize_borrowing_contract(&f.pool_id, &f.admin, &borrowing_addr);
    f.client.deposit(&f.pool_id, &f.lender, &10_000i128);

    let result = f.client.try_update_debt(&f.pool_id, &1_000i128);
    assert_eq!(result, Err(Ok(Error::Unauthorized)));
}

// ===========================================================================
// Read-only endpoints remain public
// ===========================================================================

#[test]
fn public_read_endpoints_unaffected() {
    let env = Env::default();
    let f = setup(&env);
    let borrowing_addr = mock_borrowing_contract(&env);

    f.client
        .initialize_borrowing_contract(&f.pool_id, &f.admin, &borrowing_addr);
    f.client.deposit(&f.pool_id, &f.lender, &5_000i128);

    // All read-only calls succeed without authorization.
    let accounting = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(accounting.total_assets, 5_000);

    let balance = f.client.get_pool_balance(&f.pool_id);
    assert_eq!(balance, 5_000);

    let shares = f.client.get_share_balance(&f.pool_id, &f.lender);
    assert_eq!(shares.shares, 5_000);

    let rate = f.client.get_interest_rate(&f.pool_id);
    assert_eq!(rate, 500);

    let stored = f.client.get_borrowing_contract(&f.pool_id);
    assert_eq!(stored, borrowing_addr);

    let status = f.client.get_pool_status(&f.pool_id);
    assert_eq!(status, shared::types::PoolStatus::Active);
}
