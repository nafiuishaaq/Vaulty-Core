#![no_std]
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Map};
use shared::{
    errors::Error,
    types::{Asset, Loan, LoanStatus},
    utils::{SafeMath, ValidationHelper},
};

/// Borrowing contract for managing collateralized loans.
#[contract]
pub struct BorrowingContract;

const COLLATERAL_RATIO_BPS: u32 = 15_000;

#[contractimpl]
impl BorrowingContract {
    pub fn borrow(
        env: Env,
        borrower: Address,
        collateral_asset: BytesN<32>,
        collateral_amount: i128,
        borrow_asset: BytesN<32>,
        borrow_amount: i128,
    ) -> Loan {
        borrower.require_auth();

        if !ValidationHelper::validate_positive_amount(collateral_amount)
            || !ValidationHelper::validate_positive_amount(borrow_amount)
        {
            panic!("{:?}", Error::InvalidAmount);
        }

        let required_collateral = ValidationHelper::required_collateral_for_borrow(borrow_amount, COLLATERAL_RATIO_BPS)
            .unwrap_or(borrow_amount);
        if collateral_amount < required_collateral {
            panic!("{:?}", Error::InsufficientCollateral);
        }

        let loan_id = Self::derive_loan_id(&env, &borrower, &borrow_asset, &collateral_asset);
        let mut loans = Self::get_loans(&env);
        if loans.contains_key(loan_id.clone()) {
            panic!("{:?}", Error::LoanAlreadyExists);
        }

        let loan = Loan {
            loan_id: loan_id.clone(),
            borrower: borrower.clone(),
            collateral_asset: Asset { code: collateral_asset, issuer: None },
            collateral_amount,
            borrow_asset: Asset { code: borrow_asset, issuer: None },
            borrow_amount,
            outstanding_amount: borrow_amount,
            interest_rate_bps: 0,
            created_at: env.ledger().timestamp(),
            status: LoanStatus::Active,
        };
        loans.set(loan_id.clone(), loan.clone());
        Self::set_loans(&env, loans);
        loan
    }

    pub fn repay(env: Env, loan_id: BytesN<32>, repayer: Address, amount: i128) -> Loan {
        repayer.require_auth();
        if !ValidationHelper::validate_positive_amount(amount) {
            panic!("{:?}", Error::InvalidAmount);
        }

        let mut loans = Self::get_loans(&env);
        let mut loan = loans.get(loan_id.clone()).unwrap_or_else(|| panic!("{:?}", Error::LoanNotFound));
        if repayer != loan.borrower {
            panic!("{:?}", Error::Unauthorized);
        }
        if loan.status == LoanStatus::Repaid || loan.status == LoanStatus::Liquidated {
            panic!("{:?}", Error::LoanAlreadyRepaid);
        }

        let next_outstanding = SafeMath::sub(loan.outstanding_amount, amount).unwrap_or(loan.outstanding_amount);
        loan.outstanding_amount = next_outstanding;
        if next_outstanding <= 0 {
            loan.status = LoanStatus::Repaid;
            loan.outstanding_amount = 0;
        }

        loans.set(loan_id.clone(), loan.clone());
        Self::set_loans(&env, loans);
        loan
    }

    pub fn add_collateral(env: Env, loan_id: BytesN<32>, amount: i128) -> Loan {
        if !ValidationHelper::validate_positive_amount(amount) {
            panic!("{:?}", Error::InvalidAmount);
        }

        let mut loans = Self::get_loans(&env);
        let mut loan = loans.get(loan_id.clone()).unwrap_or_else(|| panic!("{:?}", Error::LoanNotFound));
        loan.borrower.require_auth();
        loan.collateral_amount = SafeMath::add(loan.collateral_amount, amount).unwrap_or(loan.collateral_amount);
        loans.set(loan_id.clone(), loan.clone());
        Self::set_loans(&env, loans);
        loan
    }

    pub fn get_loan(env: Env, loan_id: BytesN<32>) -> Loan {
        let loans = Self::get_loans(&env);
        loans.get(loan_id).unwrap_or_else(|| panic!("{:?}", Error::LoanNotFound))
    }

    pub fn is_undercollateralized(env: Env, loan_id: BytesN<32>) -> bool {
        let loans = Self::get_loans(&env);
        let loan = loans.get(loan_id).unwrap_or_else(|| panic!("{:?}", Error::LoanNotFound));
        Self::is_loan_undercollateralized(&loan)
    }

    pub fn liquidate(env: Env, loan_id: BytesN<32>, liquidator: Address) {
        liquidator.require_auth();

        let mut loans = Self::get_loans(&env);
        let mut loan = loans.get(loan_id.clone()).unwrap_or_else(|| panic!("{:?}", Error::LoanNotFound));
        if loan.status != LoanStatus::Active || !Self::is_loan_undercollateralized(&loan) {
            panic!("{:?}", Error::LiquidationNotAllowed);
        }

        loan.status = LoanStatus::Liquidated;
        loan.collateral_amount = 0;
        loans.set(loan_id.clone(), loan);
        Self::set_loans(&env, loans);
    }

    fn is_loan_undercollateralized(loan: &Loan) -> bool {
        if loan.status != LoanStatus::Active {
            return false;
        }

        let required_collateral = ValidationHelper::required_collateral_for_borrow(loan.outstanding_amount, COLLATERAL_RATIO_BPS)
            .unwrap_or(loan.outstanding_amount);
        loan.collateral_amount < required_collateral
    }

    fn derive_loan_id(env: &Env, borrower: &Address, borrow_asset: &BytesN<32>, collateral_asset: &BytesN<32>) -> BytesN<32> {
        let mut bytes = [0u8; 32];
        let timestamp_bytes = env.ledger().timestamp().to_be_bytes();
        for (index, byte) in timestamp_bytes.iter().enumerate() {
            bytes[index] = *byte;
        }
        let borrow_bytes = borrow_asset.to_array();
        let collateral_bytes = collateral_asset.to_array();
        for (index, byte) in borrow_bytes.iter().enumerate() {
            bytes[(index + 8) % 32] ^= *byte;
        }
        for (index, byte) in collateral_bytes.iter().enumerate() {
            bytes[(index + 16) % 32] ^= *byte;
        }
        BytesN::from_array(env, &bytes)
    }

    fn get_loans(env: &Env) -> Map<BytesN<32>, Loan> {
        let key = Self::storage_key(env, b"loans");
        env.storage().persistent().get(&key).unwrap_or_else(|| Map::new(env))
    }

    fn set_loans(env: &Env, loans: Map<BytesN<32>, Loan>) {
        let key = Self::storage_key(env, b"loans");
        env.storage().persistent().set(&key, &loans);
    }

    fn storage_key(env: &Env, label: &[u8]) -> BytesN<32> {
        let mut bytes = [0u8; 32];
        let label_len = label.len().min(32);
        bytes[..label_len].copy_from_slice(&label[..label_len]);
        BytesN::from_array(env, &bytes)
    }
}
