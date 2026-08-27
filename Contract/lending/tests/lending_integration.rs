//! Pool lifecycle integration test for the lending contract.
//!
//! Covers the full sequence described in issue #105:
//!   1. Pool creation — accounting starts at zero.
//!   2. Lender deposit — assets and shares are credited correctly.
//!   3. Debt increase via `update_debt` — available liquidity is reduced.
//!   4. A second lender deposit — pool absorbs additional liquidity cleanly.
//!   5. Partial debt repayment — liquidity is restored proportionally.
//!   6. Over-withdrawal is blocked — `InsufficientLiquidity` is returned when
//!      a lender tries to redeem more than the available liquid balance.
//!   7. Withdrawal within liquid balance succeeds — accounting is exact.
//!
//! All operations are deterministic and do not depend on a live Stellar
//! network.  The mock "borrowing contract" is a second instance of
//! `LendingContract` registered with `env.register_contract(None, …)`.
//! We only need a live on-chain address that `env.as_contract` can
//! impersonate to satisfy the authorization check inside `update_debt`;
//! the mock contract's own state is never used.

use lending::{LendingContract, LendingContractClient};
use shared::errors::Error;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env};

// ---------------------------------------------------------------------------
// Test fixture
// ---------------------------------------------------------------------------

/// All state needed by the pool lifecycle tests.
struct Fixture<'a> {
    env: &'a Env,
    client: LendingContractClient<'a>,
    /// Pool identifier — equals the asset bytes because `derive_pool_id`
    /// copies them verbatim.
    pool_id: BytesN<32>,
    #[allow(dead_code)]
    admin: Address,
    lender_a: Address,
    lender_b: Address,
    /// Registered as the authorized borrowing contract for the pool.
    borrowing_contract: Address,
}

/// Build a fresh environment with one pool and an authorized borrowing
/// contract already wired up.
fn setup(env: &Env) -> Fixture<'_> {
    let contract_id = env.register_contract(None, LendingContract);
    let client = LendingContractClient::new(env, &contract_id);

    let asset = BytesN::from_array(env, &[0xABu8; 32]);
    let admin = Address::generate(env);
    let lender_a = Address::generate(env);
    let lender_b = Address::generate(env);

    // Mock all authorizations so tests focus on accounting logic rather than
    // re-implementing the Stellar auth flow.
    env.mock_all_auths();

    // Initialize the contract — sets the contract-level admin required by
    // `create_pool`.
    client.initialize(&admin);

    // 8 % APR in basis points.
    client.create_pool(&admin, &asset, &800i128);

    // Register a second contract instance that acts as the mock borrowing
    // contract.  We only need a valid on-chain address that `env.as_contract`
    // can impersonate; registering the LendingContract struct a second time
    // gives us a distinct address without requiring a pre-built WASM artifact.
    let borrowing_contract = env.register_contract(None, LendingContract);
    client.initialize_borrowing_contract(&asset, &admin, &borrowing_contract);

    Fixture {
        env,
        client,
        pool_id: asset,
        admin,
        lender_a,
        lender_b,
        borrowing_contract,
    }
}

/// Execute `f` as the authorized borrowing contract so that calls to
/// `borrow`, `repay`, and `update_debt` pass the authorization check.
fn as_borrower<F>(fixture: &Fixture<'_>, f: F)
where
    F: FnOnce(),
{
    fixture.env.as_contract(&fixture.borrowing_contract, f);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Step 1 — Pool creation: all accounting fields must be zero immediately
/// after `create_pool` returns.
#[test]
fn pool_creation_initializes_accounting_to_zero() {
    let env = Env::default();
    let f = setup(&env);

    let acc = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(acc.total_assets, 0, "total_assets should start at zero");
    assert_eq!(acc.total_shares, 0, "total_shares should start at zero");
    assert_eq!(
        acc.available_liquidity, 0,
        "available_liquidity should start at zero"
    );
    assert_eq!(
        acc.outstanding_debt, 0,
        "outstanding_debt should start at zero"
    );
    assert_eq!(acc.accrued_interest, 0, "accrued_interest should start at zero");
}

/// Step 2 — First lender deposit: assets and shares are credited 1-to-1 for
/// the first depositor (pool is empty, so each unit of asset mints one share).
#[test]
fn first_deposit_credits_assets_and_shares_one_to_one() {
    let env = Env::default();
    let f = setup(&env);

    f.client.deposit(&f.pool_id, &f.lender_a, &1_000i128);

    let acc = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(acc.total_assets, 1_000);
    assert_eq!(acc.total_shares, 1_000);
    assert_eq!(acc.available_liquidity, 1_000);
    assert_eq!(acc.outstanding_debt, 0);

    let shares = f.client.get_share_balance(&f.pool_id, &f.lender_a);
    assert_eq!(shares.shares, 1_000);
}

/// Step 3 — Debt increase via `update_debt`: available liquidity decreases by
/// the debt amount while total assets and shares stay the same.
#[test]
fn debt_increase_reduces_available_liquidity() {
    let env = Env::default();
    let f = setup(&env);

    f.client.deposit(&f.pool_id, &f.lender_a, &1_000i128);

    // The borrowing contract draws 600 from the pool.
    as_borrower(&f, || {
        f.client.update_debt(&f.pool_id, &600i128);
    });

    let acc = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(
        acc.available_liquidity, 400,
        "available_liquidity should be reduced by the debt amount"
    );
    assert_eq!(
        acc.outstanding_debt, 600,
        "outstanding_debt should equal the amount drawn"
    );
    // Total assets and shares must remain untouched.
    assert_eq!(acc.total_assets, 1_000);
    assert_eq!(acc.total_shares, 1_000);
}

/// Step 4 — Second lender deposit while the pool carries debt: the new
/// liquidity is added to `available_liquidity` and new shares are minted.
#[test]
fn second_deposit_adds_liquidity_while_pool_has_debt() {
    let env = Env::default();
    let f = setup(&env);

    f.client.deposit(&f.pool_id, &f.lender_a, &1_000i128);

    as_borrower(&f, || {
        f.client.update_debt(&f.pool_id, &600i128);
    });

    // Lender B supplies an additional 500.
    f.client.deposit(&f.pool_id, &f.lender_b, &500i128);

    let acc = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(acc.total_assets, 1_500);
    // Available liquidity = 400 (after debt) + 500 (new deposit).
    assert_eq!(acc.available_liquidity, 900);
    assert_eq!(acc.outstanding_debt, 600);

    let shares_b = f.client.get_share_balance(&f.pool_id, &f.lender_b);
    assert!(shares_b.shares > 0, "lender B should hold a positive share balance");
}

/// Step 5 — Partial debt repayment via `update_debt` with a negative delta:
/// liquidity is restored by the repaid amount and debt decreases.
#[test]
fn partial_debt_repayment_restores_liquidity() {
    let env = Env::default();
    let f = setup(&env);

    f.client.deposit(&f.pool_id, &f.lender_a, &1_000i128);

    as_borrower(&f, || {
        f.client.update_debt(&f.pool_id, &600i128);
    });

    // Repay 250 of the outstanding debt.
    as_borrower(&f, || {
        f.client.update_debt(&f.pool_id, &-250i128);
    });

    let acc = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(
        acc.outstanding_debt, 350,
        "outstanding_debt should decrease by the repaid amount"
    );
    assert_eq!(
        acc.available_liquidity, 650,
        "available_liquidity should be restored by the repaid amount"
    );
    // Total assets and shares remain at the original deposit.
    assert_eq!(acc.total_assets, 1_000);
    assert_eq!(acc.total_shares, 1_000);
}

/// Step 6 — Over-withdrawal is blocked: a lender holding enough shares to
/// cover a redemption is still rejected when the liquid balance is insufficient.
#[test]
fn over_withdrawal_blocked_when_insufficient_liquidity() {
    let env = Env::default();
    let f = setup(&env);

    f.client.deposit(&f.pool_id, &f.lender_a, &1_000i128);

    // Borrow 800, leaving only 200 liquid.
    as_borrower(&f, || {
        f.client.update_debt(&f.pool_id, &800i128);
    });

    let acc_before = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(acc_before.available_liquidity, 200);

    // Lender A holds 1000 shares (redeemable for 1000 assets) but only
    // 200 are liquid — the withdrawal must be rejected.
    let result = f.client.try_withdraw(&f.pool_id, &f.lender_a, &500i128);
    assert_eq!(
        result,
        Err(Ok(Error::InsufficientLiquidity)),
        "should fail with InsufficientLiquidity when redeem amount > available liquidity"
    );

    // Pool accounting must be unchanged after the failed call.
    let acc_after = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(acc_after.total_assets, acc_before.total_assets);
    assert_eq!(acc_after.total_shares, acc_before.total_shares);
    assert_eq!(acc_after.available_liquidity, acc_before.available_liquidity);
    assert_eq!(acc_after.outstanding_debt, acc_before.outstanding_debt);

    // The lender's share balance must be preserved.
    let shares = f.client.get_share_balance(&f.pool_id, &f.lender_a);
    assert_eq!(shares.shares, 1_000);
}

/// Step 7 — Withdrawal within liquid balance succeeds: accounting values are
/// exact after a valid redemption.
#[test]
fn withdrawal_within_liquid_balance_succeeds() {
    let env = Env::default();
    let f = setup(&env);

    f.client.deposit(&f.pool_id, &f.lender_a, &1_000i128);

    // Borrow 600, leaving 400 liquid.
    as_borrower(&f, || {
        f.client.update_debt(&f.pool_id, &600i128);
    });

    // Withdraw 300 shares (redeems 300 assets — within the 400 available).
    f.client.withdraw(&f.pool_id, &f.lender_a, &300i128);

    let acc = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(acc.total_assets, 700, "total_assets must decrease by the redeemed amount");
    assert_eq!(acc.total_shares, 700, "total_shares must decrease by the withdrawn shares");
    assert_eq!(
        acc.available_liquidity, 100,
        "available_liquidity = 400 - 300 = 100"
    );
    // Debt must be unaffected by the lender's withdrawal.
    assert_eq!(acc.outstanding_debt, 600);

    let shares = f.client.get_share_balance(&f.pool_id, &f.lender_a);
    assert_eq!(shares.shares, 700);
}

/// Full lifecycle: create → deposit → borrow → second deposit → repay →
/// withdraw — verifies the final state of all accounting fields in one
/// end-to-end sequence.
#[test]
fn full_pool_lifecycle() {
    let env = Env::default();
    let f = setup(&env);

    // 1. Lender A deposits 2 000.
    f.client.deposit(&f.pool_id, &f.lender_a, &2_000i128);

    let after_deposit = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(after_deposit.total_assets, 2_000);
    assert_eq!(after_deposit.available_liquidity, 2_000);
    assert_eq!(after_deposit.outstanding_debt, 0);

    // 2. Borrowing contract draws 1 200.
    as_borrower(&f, || {
        f.client.update_debt(&f.pool_id, &1_200i128);
    });

    let after_borrow = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(after_borrow.available_liquidity, 800);
    assert_eq!(after_borrow.outstanding_debt, 1_200);

    // 3. Lender B deposits 500 while debt is outstanding.
    f.client.deposit(&f.pool_id, &f.lender_b, &500i128);

    let after_second_deposit = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(after_second_deposit.total_assets, 2_500);
    assert_eq!(after_second_deposit.available_liquidity, 1_300);

    // 4. Borrowing contract repays 700.
    as_borrower(&f, || {
        f.client.update_debt(&f.pool_id, &-700i128);
    });

    let after_repay = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(after_repay.outstanding_debt, 500);
    assert_eq!(after_repay.available_liquidity, 2_000);

    // 5. Lender A withdraws 400 shares (redeems 400 assets, within the 2 000
    //    liquid).
    f.client.withdraw(&f.pool_id, &f.lender_a, &400i128);

    let final_acc = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(final_acc.total_assets, 2_100);
    assert_eq!(final_acc.available_liquidity, 1_600);
    assert_eq!(final_acc.outstanding_debt, 500);

    // Lender A retains 1 600 shares (started with 2 000, withdrew 400).
    let shares_a = f.client.get_share_balance(&f.pool_id, &f.lender_a);
    assert_eq!(shares_a.shares, 1_600);
}
