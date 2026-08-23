
#![cfg(test)]
extern crate std;

use crate::{LendingContract, LendingContractClient};
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::{symbol_short, Address, BytesN, Env, IntoVal, Val};

// Import the token contract
use soroban_sdk::token::Client as TokenClient;

fn create_token_contract<'a>(e: &Env, admin: &Address) -> TokenClient<'a> {
    TokenClient::new(e, &e.register_stellar_asset_contract(admin.clone()))
}

fn create_lending_contract<'a>(e: &Env) -> LendingContractClient<'a> {
    LendingContractClient::new(e, &e.register_contract(None, LendingContract))
}

struct TestData<'a> {
    env: Env,
    admin: Address,
    lender: Address,
    borrower: Address,
    token: TokenClient<'a>,
    contract: LendingContractClient<'a>,
    pool_id: BytesN<32>,
}

impl<'a> TestData<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let lender = Address::generate(&env);
        let borrower = Address::generate(&env);

        let token = create_token_contract(&env, &admin);
        let contract = create_lending_contract(&env);

        let asset = token.address.clone().try_into().unwrap();
        contract.create_pool(&admin, &asset, &1000); // 10% interest rate

        let pool_id = contract.derive_pool_id(&asset);

        TestData {
            env,
            admin,
            lender,
            borrower,
            token,
            contract,
            pool_id,
        }
    }
}

#[test]
fn test_borrow_successful() {
    let mut data = TestData::new();

    // Setup: Lender deposits liquidity
    let deposit_amount = 100_000;
    data.token.mint(&data.lender, &deposit_amount);
    data.contract.deposit(&data.pool_id, &data.lender, &deposit_amount);

    // Action: Borrower borrows
    let borrow_amount = 50_000;
    data.contract.borrow(&data.pool_id, &data.borrower, &borrow_amount);

    // Verification
    assert_eq!(data.token.balance(&data.borrower), borrow_amount);
    assert_eq!(data.token.balance(&data.contract.address), deposit_amount - borrow_amount);

    let accounting = data.contract.get_pool_accounting(&data.pool_id);
    assert_eq!(accounting.available_liquidity, deposit_amount - borrow_amount);
    assert_eq!(accounting.outstanding_debt, borrow_amount);
}

#[test]
fn test_repay_successful() {
    let mut data = TestData::new();

    // Setup: Lender deposits, borrower borrows
    let deposit_amount = 100_000;
    data.token.mint(&data.lender, &deposit_amount);
    data.contract.deposit(&data.pool_id, &data.lender, &deposit_amount);

    let borrow_amount = 50_000;
    data.contract.borrow(&data.pool_id, &data.borrower, &borrow_amount);

    // Action: Borrower repays
    let principal_amount = 50_000;
    let interest_amount = 100; // Simplified interest for testing
    let total_payment = principal_amount + interest_amount;
    data.token.mint(&data.borrower, &total_payment); // Mint for repayment
    data.contract.repay(&data.pool_id, &data.borrower, &principal_amount, &interest_amount);

    // Verification
    assert_eq!(data.token.balance(&data.borrower), 0);
    assert_eq!(data.token.balance(&data.contract.address), deposit_amount + total_payment);

    let accounting = data.contract.get_pool_accounting(&data.pool_id);
    assert_eq!(accounting.available_liquidity, deposit_amount + total_payment);
    assert_eq!(accounting.outstanding_debt, 0);
    assert_eq!(accounting.accrued_interest, interest_amount);
}

#[test]
fn test_borrow_insufficient_liquidity() {
    let mut data = TestData::new();

    // Setup: No liquidity
    let borrow_amount = 50_000;

    // Action & Verification
    let result = data.contract.try_borrow(&data.pool_id, &data.borrower, &borrow_amount);
    assert!(result.is_err());
}

#[test]
fn test_repay_insufficient_balance() {
    let mut data = TestData::new();

    // Setup: Lender deposits, borrower borrows
    let deposit_amount = 100_000;
    data.token.mint(&data.lender, &deposit_amount);
    data.contract.deposit(&data.pool_id, &data.lender, &deposit_amount);

    let borrow_amount = 50_000;
    data.contract.borrow(&data.pool_id, &data.borrower, &borrow_amount);

    // Action: Borrower tries to repay without sufficient balance
    let principal_amount = 50_000;
    let interest_amount = 100;

    // Verification
    let result = data.contract.try_repay(&data.pool_id, &data.borrower, &principal_amount, &interest_amount);
    assert!(result.is_err());
}