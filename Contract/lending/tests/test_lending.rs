use lending::LendingContractClient;
use shared::errors::Error;
use shared::types::PoolAccounting;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env};

use lending::LendingContract;

// ---------------------------------------------------------------------------
// Helper: set up a fresh lending contract and return the client + an admin.
// ---------------------------------------------------------------------------
struct TestEnv {
    env: Env,
    client: LendingContractClient<'static>,
    admin: Address,
    pool_id: BytesN<32>,
}

impl TestEnv {
    fn setup() -> Self {
        let env = Env::default();
        let contract_id = env.register_contract(None, LendingContract);
        // Leak the contract_id so the client can borrow it for 'static.
        let contract_id = Box::leak(Box::new(contract_id));

        let client = LendingContractClient::new(&env, contract_id);
        let admin = Address::generate(&env);
        let pool_id = BytesN::from_array(&env, &[1u8; 32]);
        let asset = BytesN::from_array(&env, &[2u8; 32]);

        // 5% APR in basis points.
        client.create_pool(&admin, &asset, &500i128);

        TestEnv {
            env,
            client,
            admin,
            pool_id: asset, // derive_pool_id copies the asset bytes
        }
    }

    /// Register a mock contract and initialize it as the pool's borrowing
    /// contract.  Returns the mock's address.
    fn init_borrowing_contract(&self) -> Address {
        let borrowing_id = self.env.register_contract_wasm(None, lending::WASM);
        self.client.initialize_borrowing_contract(
            &self.pool_id,
            &self.admin,
            &borrowing_id,
        );
        borrowing_id
    }

    /// Call a lending-contract method **as** the given contract address,
    /// simulating a cross-contract call.
    fn as_contract<F, T>(&self, contract_id: &Address, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        self.env.as_contract(contract_id, f)
    }
}

// ===========================================================================
// Pool creation
// ===========================================================================

#[test]
fn test_pool_creation() {
    let t = TestEnv::setup();

    let accounting = t.client.get_pool_accounting(&t.pool_id);
    assert_eq!(accounting.total_assets, 0);
    assert_eq!(accounting.total_shares, 0);
    assert_eq!(accounting.available_liquidity, 0);
    assert_eq!(accounting.outstanding_debt, 0);
}

#[test]
fn test_pool_already_exists() {
    let t = TestEnv::setup();

    let result = t
        .client
        .try_create_pool(&t.pool_id, &[2u8; 32].into(), &t.admin, &500i128);
    assert_eq!(result, Err(Ok(Error::PoolAlreadyExists)));
}

#[test]
fn test_invalid_interest_rate() {
    let env = Env::default();
    let contract_id = env.register_contract(None, LendingContract);
    let client = LendingContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let asset = BytesN::from_array(&env, &[2u8; 32]);

    let result = client.try_create_pool(&asset, &asset, &admin, &-1i128);
    assert_eq!(result, Err(Ok(Error::InvalidInterestRate)));

    let result = client.try_create_pool(&asset, &asset, &admin, &10001i128);
    assert_eq!(result, Err(Ok(Error::InvalidInterestRate)));
}

// ===========================================================================
// Deposits & withdrawals
// ===========================================================================

#[test]
fn test_single_deposit() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);

    t.client.deposit(&t.pool_id, &lender, &1000i128);

    let accounting = t.client.get_pool_accounting(&t.pool_id);
    assert_eq!(accounting.total_assets, 1000);
    assert_eq!(accounting.total_shares, 1000);
    assert_eq!(accounting.available_liquidity, 1000);

    let shares = t.client.get_share_balance(&t.pool_id, &lender);
    assert_eq!(shares.shares, 1000);
}

#[test]
fn test_multiple_suppliers() {
    let t = TestEnv::setup();
    let lender1 = Address::generate(&t.env);
    let lender2 = Address::generate(&t.env);

    t.client.deposit(&t.pool_id, &lender1, &1000i128);
    t.client.deposit(&t.pool_id, &lender2, &500i128);

    let accounting = t.client.get_pool_accounting(&t.pool_id);
    assert_eq!(accounting.total_assets, 1500);
    assert_eq!(accounting.total_shares, 1500);
    assert_eq!(accounting.available_liquidity, 1500);

    let shares = t.client.get_share_balance(&t.pool_id, &lender2);
    assert_eq!(shares.shares, 500);
}

#[test]
fn test_withdrawal() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);

    t.client.deposit(&t.pool_id, &lender, &1000i128);
    t.client.withdraw(&t.pool_id, &lender, &500i128);

    let accounting = t.client.get_pool_accounting(&t.pool_id);
    assert_eq!(accounting.total_assets, 500);
    assert_eq!(accounting.total_shares, 500);
    assert_eq!(accounting.available_liquidity, 500);

    let shares = t.client.get_share_balance(&t.pool_id, &lender);
    assert_eq!(shares.shares, 500);
}

#[test]
fn test_partial_redemption() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);

    t.client.deposit(&t.pool_id, &lender, &1000i128);
    t.client.withdraw(&t.pool_id, &lender, &250i128);

    let accounting = t.client.get_pool_accounting(&t.pool_id);
    assert_eq!(accounting.total_assets, 750);
    assert_eq!(accounting.total_shares, 750);
}

#[test]
fn test_withdraw_insufficient_shares() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);

    t.client.deposit(&t.pool_id, &lender, &1000i128);

    let result = t.client.try_withdraw(&t.pool_id, &lender, &1500i128);
    assert_eq!(result, Err(Ok(Error::InsufficientShares)));
}

#[test]
fn test_withdraw_insufficient_liquidity() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);
    let borrowing_id = t.init_borrowing_contract();

    t.client.deposit(&t.pool_id, &lender, &1000i128);

    // Simulate borrowing via the authorized borrowing contract
    t.as_contract(&borrowing_id, || {
        t.client.borrow(&t.pool_id, &Address::generate(&t.env), &800i128);
    });

    let accounting = t.client.get_pool_accounting(&t.pool_id);
    assert_eq!(accounting.available_liquidity, 200);

    // 500 shares redeem to 500 assets, exceeding the 200 available.
    let result = t.client.try_withdraw(&t.pool_id, &lender, &500i128);
    assert_eq!(result, Err(Ok(Error::InsufficientLiquidity)));
}

#[test]
fn test_zero_liquidity_withdrawal() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);
    let borrowing_id = t.init_borrowing_contract();

    t.client.deposit(&t.pool_id, &lender, &1000i128);

    // Borrow all liquidity via the authorized borrowing contract
    t.as_contract(&borrowing_id, || {
        t.client.borrow(&t.pool_id, &Address::generate(&t.env), &1000i128);
    });

    let result = t.client.try_withdraw(&t.pool_id, &lender, &100i128);
    assert_eq!(result, Err(Ok(Error::InsufficientLiquidity)));
}

// ===========================================================================
// Interest accrual
// ===========================================================================

#[test]
fn test_interest_accrual() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);
    let borrowing_id = t.init_borrowing_contract();

    t.client.deposit(&t.pool_id, &lender, &1000i128);

    // Simulate borrowing
    t.as_contract(&borrowing_id, || {
        t.client.borrow(&t.pool_id, &Address::generate(&t.env), &500i128);
    });

    // Jump forward 1 day
    t.env.ledger().set(86400, 1, 1);

    // Trigger accrual via deposit
    t.client.deposit(&t.pool_id, &lender, &100i128);

    let accounting = t.client.get_pool_accounting(&t.pool_id);
    assert!(accounting.accrued_interest > 0);
    assert!(accounting.interest_index > 1_000_000_000_000_000_000);
}

#[test]
fn test_long_time_jump() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);
    let borrowing_id = t.init_borrowing_contract();

    t.client.deposit(&t.pool_id, &lender, &1000i128);

    t.as_contract(&borrowing_id, || {
        t.client.borrow(&t.pool_id, &Address::generate(&t.env), &500i128);
    });

    // Jump forward 1 year
    t.env.ledger().set(31_536_000, 1, 1);

    t.client.deposit(&t.pool_id, &lender, &100i128);

    let accounting = t.client.get_pool_accounting(&t.pool_id);
    assert!(accounting.accrued_interest > 0);
}

// ===========================================================================
// Input validation
// ===========================================================================

#[test]
fn test_arithmetic_limits() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);

    let result = t.client.try_deposit(&t.pool_id, &lender, &-100i128);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));

    let result = t.client.try_deposit(&t.pool_id, &lender, &0i128);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));
}

#[test]
fn test_pool_not_found() {
    let env = Env::default();
    let contract_id = env.register_contract(None, LendingContract);
    let client = LendingContractClient::new(&env, &contract_id);

    let pool_id = BytesN::from_array(&env, &[1u8; 32]);
    let lender = Address::generate(&env);

    let result = client.try_deposit(&pool_id, &lender, &1000i128);
    assert_eq!(result, Err(Ok(Error::PoolNotFound)));

    let result = client.try_get_pool_balance(&pool_id);
    assert_eq!(result, Err(Ok(Error::PoolNotFound)));
}

// ===========================================================================
// Debt tracking
// ===========================================================================

#[test]
fn test_debt_tracking() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);
    let borrowing_id = t.init_borrowing_contract();

    t.client.deposit(&t.pool_id, &lender, &1000i128);

    // Borrow
    t.as_contract(&borrowing_id, || {
        t.client.update_debt(&t.pool_id, &300i128);
    });

    let accounting = t.client.get_pool_accounting(&t.pool_id);
    assert_eq!(accounting.outstanding_debt, 300);
    assert_eq!(accounting.available_liquidity, 700);

    // Repay
    t.as_contract(&borrowing_id, || {
        t.client.update_debt(&t.pool_id, &-100i128);
    });

    let accounting = t.client.get_pool_accounting(&t.pool_id);
    assert_eq!(accounting.outstanding_debt, 200);
    assert_eq!(accounting.available_liquidity, 800);
}

// ===========================================================================
// User interest calculation
// ===========================================================================

#[test]
fn test_user_interest_calculation() {
    let t = TestEnv::setup();
    let lender = Address::generate(&t.env);
    let borrowing_id = t.init_borrowing_contract();

    t.client.deposit(&t.pool_id, &lender, &1000i128);

    t.as_contract(&borrowing_id, || {
        t.client.borrow(&t.pool_id, &Address::generate(&t.env), &500i128);
    });

    // Jump forward 1 day
    t.env.ledger().set(86400, 1, 1);

    // Trigger accrual
    t.client.deposit(&t.pool_id, &lender, &100i128);

    let interest = t.client.calculate_interest(&t.pool_id, &lender);
    assert!(interest >= 0);
}
