#![cfg(test)]

use lending::{LendingContract, LendingContractClient};
use shared::errors::Error;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env};

/// Common fixture: a freshly created pool plus the addresses used by the tests.
struct PoolFixture<'a> {
    client: LendingContractClient<'a>,
    pool_id: BytesN<32>,
    lender: Address,
    borrower: Address,
}

/// Register the lending contract and create a single pool.
///
/// `derive_pool_id` copies the asset bytes verbatim, so the pool id is the
/// asset id.
fn setup(env: &Env) -> PoolFixture<'_> {
    let contract_id = env.register_contract(None, LendingContract);
    let client = LendingContractClient::new(env, &contract_id);

    let asset = BytesN::from_array(env, &[2u8; 32]);
    let admin = Address::generate(env);
    let lender = Address::generate(env);
    let borrower = Address::generate(env);

    // 5% APR in basis points.
    client.create_pool(&admin, &asset, &500i128);

    PoolFixture {
        client,
        pool_id: asset,
        lender,
        borrower,
    }
}

/// A withdrawal that the lender has the shares for must still be rejected when
/// the pool has lent those assets out.
#[test]
fn withdraw_blocked_when_insufficient_liquidity() {
    let env = Env::default();
    let f = setup(&env);

    // Lender supplies 1000; first deposit mints shares 1:1.
    f.client.deposit(&f.pool_id, &f.lender, &1000i128);
    // Borrower draws 800, leaving 200 of available liquidity.
    f.client.borrow(&f.pool_id, &f.borrower, &800i128);

    let before = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(before.available_liquidity, 200);
    assert_eq!(before.outstanding_debt, 800);

    // 500 shares redeem to 500 assets, which exceeds the 200 available.
    let result = f.client.try_withdraw(&f.pool_id, &f.lender, &500i128);
    assert_eq!(result, Err(Ok(Error::InsufficientLiquidity)));

    // Nothing was paid out: the lender keeps every share and the pool
    // accounting is untouched.
    let shares = f.client.get_share_balance(&f.pool_id, &f.lender);
    assert_eq!(shares.shares, 1000);

    let after = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(after.total_assets, before.total_assets);
    assert_eq!(after.total_shares, before.total_shares);
    assert_eq!(after.available_liquidity, before.available_liquidity);
    assert_eq!(after.outstanding_debt, before.outstanding_debt);
}

/// A withdrawal that fits inside the available liquidity is unaffected by the
/// guard, even while the pool carries debt.
#[test]
fn withdraw_succeeds_when_liquidity_sufficient() {
    let env = Env::default();
    let f = setup(&env);

    f.client.deposit(&f.pool_id, &f.lender, &1000i128);
    f.client.borrow(&f.pool_id, &f.borrower, &800i128);

    // 150 shares redeem to 150 assets, within the 200 available.
    f.client.withdraw(&f.pool_id, &f.lender, &150i128);

    // The lender was paid 150, so 150 of both assets and shares left the pool.
    let shares = f.client.get_share_balance(&f.pool_id, &f.lender);
    assert_eq!(shares.shares, 850);

    let after = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(after.total_assets, 850);
    assert_eq!(after.total_shares, 850);
    assert_eq!(after.available_liquidity, 50);
    // The guard must not disturb outstanding debt.
    assert_eq!(after.outstanding_debt, 800);
}

/// With no debt outstanding the whole balance is available, so a full exit is
/// allowed.
#[test]
fn withdraw_succeeds_when_no_outstanding_debt() {
    let env = Env::default();
    let f = setup(&env);

    f.client.deposit(&f.pool_id, &f.lender, &1000i128);

    let before = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(before.available_liquidity, 1000);
    assert_eq!(before.outstanding_debt, 0);

    f.client.withdraw(&f.pool_id, &f.lender, &1000i128);

    let after = f.client.get_pool_accounting(&f.pool_id);
    assert_eq!(after.total_assets, 0);
    assert_eq!(after.total_shares, 0);
    assert_eq!(after.available_liquidity, 0);
    assert_eq!(after.outstanding_debt, 0);

    // A fully exited lender has their share entry removed.
    let result = f.client.try_get_share_balance(&f.pool_id, &f.lender);
    assert_eq!(result, Err(Ok(Error::InsufficientShares)));
}
