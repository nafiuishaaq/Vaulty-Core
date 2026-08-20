//! Tests for `BorrowingContract::add_collateral`.
//!
//! `add_collateral` must move the additional collateral through the approved
//! vault custody path (a `deposit` into the collateralized vault) before it
//! updates the loan's `collateral_amount`, and the whole operation must be
//! atomic: if the token transfer fails, the loan accounting (and the vault
//! balance) must remain unchanged.
//!
//! The tests use lightweight mock contracts for the vault, lending pool, and
//! token so the borrowing contract's cross-contract calls are exercised
//! against a deterministic environment where transfers can be made to fail.

use shared::errors::Error;
use shared::types::{Asset, PoolAccounting, VaultMetadata, VaultStatus};
use soroban_sdk::{
    contract, contractimpl, contracttype, testutils::Address as _, Address, BytesN, Env, Map,
};

// ---------------------------------------------------------------------------
// Mock token
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Copy, Debug)]
pub enum TokenError {
    InvalidAmount = 1,
    InsufficientBalance = 2,
    TransferRejected = 3,
}

/// Minimal token implementing just enough of the Soroban token interface for
/// the vault mock: `mint`, `balance`, and `transfer`. Transfers can be made
/// to fail deterministically via `set_reject_transfers`.
#[contract]
pub struct TokenMock;

#[contractimpl]
impl TokenMock {
    pub fn mint(env: Env, to: Address, amount: i128) {
        if amount <= 0 {
            panic!("{:?}", TokenError::InvalidAmount);
        }
        let mut balances = Self::balances(&env);
        let current = balances.get(to.clone()).unwrap_or(0);
        balances.set(to, current.checked_add(amount).unwrap());
        Self::save_balances(&env, balances);
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        let balances = Self::balances(&env);
        balances.get(id).unwrap_or(0)
    }

    pub fn set_reject_transfers(env: Env, reject: bool) {
        env.storage().instance().set(&"reject_transfers", &reject);
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        if amount <= 0 {
            panic!("{:?}", TokenError::InvalidAmount);
        }
        let reject: bool = env
            .storage()
            .instance()
            .get(&"reject_transfers")
            .unwrap_or(false);
        if reject {
            panic!("{:?}", TokenError::TransferRejected);
        }

        let mut balances = Self::balances(&env);
        let from_balance = balances.get(from.clone()).unwrap_or(0);
        if from_balance < amount {
            panic!("{:?}", TokenError::InsufficientBalance);
        }

        let new_from = from_balance.checked_sub(amount).unwrap();
        if new_from == 0 {
            balances.remove(from.clone());
        } else {
            balances.set(from, new_from);
        }

        let to_balance = balances.get(to.clone()).unwrap_or(0);
        balances.set(to, to_balance.checked_add(amount).unwrap());
        Self::save_balances(&env, balances);
    }

    fn balances(env: &Env) -> Map<Address, i128> {
        env.storage()
            .persistent()
            .get(&"balances")
            .unwrap_or_else(|| Map::new(env))
    }

    fn save_balances(env: &Env, balances: Map<Address, i128>) {
        env.storage().persistent().set(&"balances", &balances);
    }
}

// ---------------------------------------------------------------------------
// Mock vault
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, PartialEq, Eq)]
pub enum MockVaultKey {
    Metadata(BytesN<32>),
    Balance(BytesN<32>),
}

/// Mirrors the vault contract's `VaultId` ABI shape (a single-field tuple
/// struct) so arguments encoded by the borrowing contract as a `VaultId`
/// decode correctly against this mock.
#[contracttype]
#[derive(Clone, PartialEq, Eq)]
pub struct MockVaultId(BytesN<32>);

/// Mock of the vault contract's custody interface: `create_vault`,
/// `get_vault`, `get_balance`, `lock_vault`, and `deposit`. `deposit` moves
/// tokens from the depositor into the vault contract before crediting the
/// vault's balance, mirroring the real vault's deposit ordering.
#[contract]
pub struct MockVault;

#[contractimpl]
impl MockVault {
    pub fn create_vault(env: Env, owner: Address, token: Address) -> BytesN<32> {
        let counter: u64 = env.storage().persistent().get(&"counter").unwrap_or(0);
        let new_counter = counter.checked_add(1).unwrap();
        env.storage().persistent().set(&"counter", &new_counter);

        let mut bytes = [0u8; 32];
        bytes[0..8].copy_from_slice(&new_counter.to_be_bytes());
        let vault_id = BytesN::from_array(&env, &bytes);

        let asset = Asset {
            token,
            symbol: BytesN::from_array(&env, &[0u8; 32]),
            code: BytesN::from_array(&env, &[0u8; 32]),
            issuer: owner.clone(),
        };
        let metadata = VaultMetadata {
            owner,
            asset,
            lock_period: 0,
            created_at: 0,
            unlock_time: 0,
            status: VaultStatus::Active,
        };
        env.storage()
            .persistent()
            .set(&MockVaultKey::Metadata(vault_id.clone()), &metadata);
        env.storage()
            .persistent()
            .set(&MockVaultKey::Balance(vault_id.clone()), &0i128);
        vault_id
    }

    pub fn get_vault(env: Env, vault_id: BytesN<32>) -> VaultMetadata {
        env.storage()
            .persistent()
            .get(&MockVaultKey::Metadata(vault_id))
            .unwrap()
    }

    pub fn get_balance(env: Env, vault_id: BytesN<32>) -> i128 {
        env.storage()
            .persistent()
            .get(&MockVaultKey::Balance(vault_id))
            .unwrap()
    }

    pub fn lock_vault(env: Env, vault_id: BytesN<32>) {
        let mut metadata: VaultMetadata = env
            .storage()
            .persistent()
            .get(&MockVaultKey::Metadata(vault_id.clone()))
            .unwrap();
        metadata.status = VaultStatus::Locked;
        env.storage()
            .persistent()
            .set(&MockVaultKey::Metadata(vault_id), &metadata);
    }

    pub fn deposit(
        env: Env,
        vault_id: MockVaultId,
        from: Address,
        amount: i128,
    ) -> Result<(), Error> {
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let metadata: VaultMetadata = env
            .storage()
            .persistent()
            .get(&MockVaultKey::Metadata(vault_id.0.clone()))
            .ok_or(Error::VaultNotFound)?;

        // Transfer tokens from the depositor into the vault contract first.
        // If this fails the whole invocation reverts, so the balance update
        // below never lands.
        soroban_sdk::token::Client::new(&env, &metadata.asset.token).transfer(
            &from,
            &env.current_contract_address(),
            &amount,
        );

        let balance: i128 = env
            .storage()
            .persistent()
            .get(&MockVaultKey::Balance(vault_id.0.clone()))
            .unwrap_or(0);
        let new_balance = balance.checked_add(amount).ok_or(Error::Overflow)?;
        env.storage()
            .persistent()
            .set(&MockVaultKey::Balance(vault_id.0), &new_balance);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Mock lending pool
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, PartialEq, Eq)]
pub enum MockPoolKey {
    Accounting(BytesN<32>),
}

/// Mock of the lending pool functions used by the borrowing contract's
/// `borrow` flow: `create_pool`, `get_pool_accounting`, and `borrow`.
#[contract]
pub struct MockLendingPool;

#[contractimpl]
impl MockLendingPool {
    pub fn create_pool(env: Env, pool_id: BytesN<32>, liquidity: i128) {
        let accounting = PoolAccounting {
            total_assets: liquidity,
            total_shares: liquidity,
            available_liquidity: liquidity,
            outstanding_debt: 0,
            accrued_interest: 0,
            interest_index: 0,
        };
        env.storage()
            .persistent()
            .set(&MockPoolKey::Accounting(pool_id), &accounting);
    }

    pub fn get_pool_accounting(env: Env, pool_id: BytesN<32>) -> PoolAccounting {
        env.storage()
            .persistent()
            .get(&MockPoolKey::Accounting(pool_id))
            .unwrap()
    }

    pub fn borrow(env: Env, pool_id: BytesN<32>, _to: Address, amount: i128) {
        let mut accounting: PoolAccounting = env
            .storage()
            .persistent()
            .get(&MockPoolKey::Accounting(pool_id.clone()))
            .unwrap();
        accounting.available_liquidity = accounting.available_liquidity - amount;
        env.storage()
            .persistent()
            .set(&MockPoolKey::Accounting(pool_id), &accounting);
    }
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

type Setup = (
    Env,
    Address,    // borrowing contract
    Address,    // vault contract
    Address,    // token
    Address,    // borrower
    BytesN<32>, // vault id
    BytesN<32>, // loan id
);

/// Deploy all contracts, create a vault, deposit initial collateral, and open
/// a loan of 5000 against 8000 of collateral.
fn setup_with_loan() -> Setup {
    let env = Env::default();
    env.mock_all_auths();

    let token = Address::generate(&env);
    env.register_contract(&token, TokenMock);

    let vault_contract_id = env.register_contract(None, MockVault);
    let pool_contract_id = env.register_contract(None, MockLendingPool);
    let borrowing_id = env.register_contract(None, borrowing::BorrowingContract);

    let token_client = TokenMockClient::new(&env, &token);
    let vault_client = MockVaultClient::new(&env, &vault_contract_id);
    let pool_client = MockLendingPoolClient::new(&env, &pool_contract_id);
    let borrowing_client = borrowing::BorrowingContractClient::new(&env, &borrowing_id);

    let borrower = Address::generate(&env);
    let pool_id = BytesN::from_array(&env, &[1u8; 32]);

    // Fund the borrower: 20_000 total, 8_000 goes into the vault as initial
    // collateral, the rest stays liquid.
    token_client.mint(&borrower, &20_000i128);

    // Create the borrower's vault and deposit the initial collateral.
    let vault_id = vault_client.create_vault(&borrower, &token);
    vault_client.deposit(&MockVaultId(vault_id.clone()), &borrower, &8_000i128);

    // Provide lending pool liquidity and wire the contracts together.
    pool_client.create_pool(&pool_id, &10_000i128);
    borrowing_client.initialize(&pool_contract_id, &vault_contract_id);
    borrowing_client.configure_collateral(
        &vault_id, &8_000i128, // liquidation threshold (80%)
        &7_500i128, // loan-to-value (75%)
        &9_500i128, // safety factor
    );

    // Open the loan against the vault's collateral.
    let loan_id = BytesN::from_array(&env, &[3u8; 32]);
    borrowing_client.borrow(&loan_id, &borrower, &vault_id, &pool_id, &5_000i128);

    let loan = borrowing_client.get_loan(&loan_id);
    assert_eq!(loan.collateral_amount, 8_000);

    (
        env,
        borrowing_id,
        vault_contract_id,
        token,
        borrower,
        vault_id,
        loan_id,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_add_collateral_transfers_tokens_into_vault() {
    let (env, borrowing_id, vault_contract_id, token, borrower, vault_id, loan_id) =
        setup_with_loan();

    let token_client = TokenMockClient::new(&env, &token);
    let vault_client = MockVaultClient::new(&env, &vault_contract_id);
    let borrowing_client = borrowing::BorrowingContractClient::new(&env, &borrowing_id);

    let vault_balance_before = vault_client.get_balance(&vault_id);
    assert_eq!(vault_balance_before, 8_000);
    let borrower_balance_before = token_client.balance(&borrower);
    let vault_contract_balance_before = token_client.balance(&vault_contract_id);

    borrowing_client.add_collateral(&loan_id, &500i128);

    // Loan accounting reflects the additional collateral.
    let loan = borrowing_client.get_loan(&loan_id);
    assert_eq!(loan.collateral_amount, 8_500);

    // The vault actually received the tokens.
    assert_eq!(vault_client.get_balance(&vault_id), 8_500);

    // Token movement happened: borrower sent 500, vault contract received 500.
    assert_eq!(
        token_client.balance(&borrower),
        borrower_balance_before - 500
    );
    assert_eq!(
        token_client.balance(&vault_contract_id),
        vault_contract_balance_before + 500
    );
}

#[test]
fn test_add_collateral_invalid_amount() {
    let (env, borrowing_id, _vault_contract_id, _token, _borrower, _vault_id, loan_id) =
        setup_with_loan();

    let borrowing_client = borrowing::BorrowingContractClient::new(&env, &borrowing_id);

    let result = borrowing_client.try_add_collateral(&loan_id, &0i128);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));

    let result = borrowing_client.try_add_collateral(&loan_id, &-100i128);
    assert_eq!(result, Err(Ok(Error::InvalidAmount)));

    let loan = borrowing_client.get_loan(&loan_id);
    assert_eq!(loan.collateral_amount, 8_000);
}

#[test]
fn test_add_collateral_loan_not_found() {
    let (env, borrowing_id, _vault_contract_id, _token, _borrower, _vault_id, _loan_id) =
        setup_with_loan();

    let borrowing_client = borrowing::BorrowingContractClient::new(&env, &borrowing_id);

    let missing_loan_id = BytesN::from_array(&env, &[9u8; 32]);
    let result = borrowing_client.try_add_collateral(&missing_loan_id, &100i128);
    assert_eq!(result, Err(Ok(Error::LoanNotFound)));
}

#[test]
fn test_failed_token_transfer_leaves_collateral_unchanged() {
    let (env, borrowing_id, vault_contract_id, token, _borrower, vault_id, loan_id) =
        setup_with_loan();

    let token_client = TokenMockClient::new(&env, &token);
    let vault_client = MockVaultClient::new(&env, &vault_contract_id);
    let borrowing_client = borrowing::BorrowingContractClient::new(&env, &borrowing_id);

    let loan_before = borrowing_client.get_loan(&loan_id);
    assert_eq!(loan_before.collateral_amount, 8_000);
    assert_eq!(vault_client.get_balance(&vault_id), 8_000);

    // Make the token reject the transfer, simulating a failed token transfer.
    token_client.set_reject_transfers(&true);

    let result = borrowing_client.try_add_collateral(&loan_id, &500i128);
    assert!(
        result.is_err(),
        "add_collateral must fail when the token transfer fails"
    );

    // Loan accounting is unchanged.
    let loan_after = borrowing_client.get_loan(&loan_id);
    assert_eq!(loan_after.collateral_amount, 8_000);
    assert_eq!(loan_after.last_updated, loan_before.last_updated);

    // Vault balance is unchanged.
    assert_eq!(vault_client.get_balance(&vault_id), 8_000);
}

#[test]
fn test_insufficient_balance_leaves_collateral_unchanged() {
    let (env, borrowing_id, vault_contract_id, token, borrower, vault_id, loan_id) =
        setup_with_loan();

    let token_client = TokenMockClient::new(&env, &token);
    let vault_client = MockVaultClient::new(&env, &vault_contract_id);
    let borrowing_client = borrowing::BorrowingContractClient::new(&env, &borrowing_id);

    // The borrower has 12_000 left; try to add more than they hold.
    let too_much = 13_000i128;
    let result = borrowing_client.try_add_collateral(&loan_id, &too_much);
    assert!(
        result.is_err(),
        "add_collateral must fail when the borrower lacks the tokens"
    );

    // Loan accounting and vault balance are unchanged.
    let loan = borrowing_client.get_loan(&loan_id);
    assert_eq!(loan.collateral_amount, 8_000);
    assert_eq!(vault_client.get_balance(&vault_id), 8_000);

    // No tokens were moved out of the borrower's wallet.
    assert_eq!(token_client.balance(&borrower), 12_000);
}

#[test]
fn test_add_collateral_requires_borrower_authorization() {
    let (env, borrowing_id, _vault_contract_id, _token, _borrower, _vault_id, loan_id) =
        setup_with_loan();

    // Disable auth mocking so `require_auth` is actually enforced.
    let empty_auths: Vec<soroban_sdk::xdr::SorobanAuthorizationEntry> = Vec::new();
    env.set_auths(&empty_auths);

    let borrowing_client = borrowing::BorrowingContractClient::new(&env, &borrowing_id);

    // An unauthenticated caller attempts to add collateral to the borrower's
    // loan; `require_auth` must reject it.
    let result = borrowing_client.try_add_collateral(&loan_id, &500i128);
    assert!(
        result.is_err(),
        "add_collateral must require the borrower's authorization"
    );

    let loan = borrowing_client.get_loan(&loan_id);
    assert_eq!(loan.collateral_amount, 8_000);
}
