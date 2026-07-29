#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol,
    Address, BytesN, Env, IntoVal, Symbol, Vec,
};
use shared::errors::Error;
use shared::events::{LoanIssued, LoanRepaid, LoanLiquidated, CollateralLocked, CollateralReleased};
use shared::types::{CollateralConfig, LoanInfo, LoanStatus, PoolAccounting};
use shared::utils::{SafeMath, TimeHelper, ValidationHelper, FixedMath};

/// Storage keys for borrowing contract
#[derive(Clone)]
#[contracttype]
pub enum BorrowingKey {
    Loan(BytesN<32>),
    CollateralConfig(BytesN<32>),
    UserLoans(Address),
    LoanExists(BytesN<32>),
    GlobalLoanCount,
    LendingPoolAddress,
    VaultContractAddress,
    CollateralLocked(BytesN<32>), // Track if collateral is locked for a loan
}

/// Borrowing contract for managing collateralized loans.
#[contract]
pub struct BorrowingContract;

#[cfg(test)]
pub use BorrowingContract;

#[contractimpl]
impl BorrowingContract {
    /// Initialize the borrowing contract with dependent contract addresses
    pub fn initialize(
        env: Env,
        lending_pool_address: Address,
        vault_contract_address: Address,
    ) -> Result<(), Error> {
        // Check if already initialized
        if env.storage().persistent().has(&BorrowingKey::LendingPoolAddress) {
            return Err(Error::AlreadyInitialized);
        }

        env.storage().persistent().set(&BorrowingKey::LendingPoolAddress, &lending_pool_address);
        env.storage().persistent().set(&BorrowingKey::VaultContractAddress, &vault_contract_address);

        Ok(())
    }

    /// Configure collateral parameters for an asset
    pub fn configure_collateral(
        env: Env,
        asset: BytesN<32>,
        liquidation_threshold: i128,
        loan_to_value: i128,
        safety_factor: i128,
    ) -> Result<(), Error> {
        if !ValidationHelper::validate_interest_rate(liquidation_threshold) {
            return Err(Error::InvalidParameters);
        }
        if !ValidationHelper::validate_interest_rate(loan_to_value) {
            return Err(Error::InvalidParameters);
        }
        if !ValidationHelper::validate_interest_rate(safety_factor) {
            return Err(Error::InvalidParameters);
        }

        let config = CollateralConfig {
            asset: asset.clone(),
            liquidation_threshold,
            loan_to_value,
            safety_factor,
        };

        env.storage()
            .persistent()
            .set(&BorrowingKey::CollateralConfig(asset), &config);

        Ok(())
    }

    /// Borrow assets against collateral - complete implementation
    pub fn borrow(
        env: Env,
        loan_id: BytesN<32>,
        borrower: Address,
        collateral_asset: BytesN<32>,
        collateral_vault_id: BytesN<32>, // Vault ID containing the collateral
        borrow_asset: BytesN<32>,
        borrow_pool_id: BytesN<32>,
        borrow_amount: i128,
    ) -> Result<(), Error> {
        // Validate inputs
        if !ValidationHelper::validate_positive_amount(borrow_amount) {
            return Err(Error::InvalidAmount);
        }

        // Check if loan already exists
        if env.storage().persistent().has(&BorrowingKey::LoanExists(loan_id.clone())) {
            return Err(Error::LoanAlreadyExists);
        }

        // Require borrower authorization
        borrower.require_auth();

        // Get collateral configuration
        let collateral_config: CollateralConfig = env
            .storage()
            .persistent()
            .get(&BorrowingKey::CollateralConfig(collateral_asset.clone()))
            .ok_or(Error::InvalidCollateral)?;

        // Get vault contract address and verify collateral ownership
        let vault_contract: Address = env.storage().persistent()
            .get(&BorrowingKey::VaultContractAddress)
            .ok_or(Error::NotInitialized)?;

        // Verify vault belongs to borrower and get collateral amount
        let mut args = Vec::new(&env);
        args.push_back(collateral_vault_id.clone().into_val(&env));
        let vault_metadata: shared::types::VaultMetadata = env.invoke_contract(
            &vault_contract,
            &Symbol::new(&env, "get_vault"),
            args
        );

        // Verify vault owner is the borrower
        if vault_metadata.owner != borrower {
            return Err(Error::Unauthorized);
        }

        // Verify vault is active and can be locked
        if vault_metadata.status != shared::types::VaultStatus::Active {
            return Err(Error::InvalidParameters);
        }

        let collateral_amount = vault_metadata.asset.token.balance(&env, &vault_contract);

        // Calculate required collateral based on LTV
        let required_collateral = SafeMath::div(
            SafeMath::mul(borrow_amount, 10000).ok_or(Error::Overflow)?,
            collateral_config.loan_to_value,
        ).ok_or(Error::Overflow)?;

        if collateral_amount < required_collateral {
            return Err(Error::InsufficientCollateral);
        }

        // Check lending pool has sufficient liquidity
        let lending_pool: Address = env.storage().persistent()
            .get(&BorrowingKey::LendingPoolAddress)
            .ok_or(Error::NotInitialized)?;

        let mut pool_args = Vec::new(&env);
        pool_args.push_back(borrow_pool_id.clone().into_val(&env));
        let pool_accounting: PoolAccounting = env.invoke_contract(
            &lending_pool,
            &Symbol::new(&env, "get_pool_accounting"),
            pool_args
        );

        if pool_accounting.available_liquidity < borrow_amount {
            return Err(Error::InsufficientLiquidity);
        }

        // Lock the vault (prevent withdrawals while debt exists)
        let mut lock_args = Vec::new(&env);
        lock_args.push_back(collateral_vault_id.clone().into_val(&env));
        env.invoke_contract::<()>(
            &vault_contract,
            &Symbol::new(&env, "lock_vault"),
            lock_args
        );

        // Transfer borrowed funds from lending pool to borrower
        let mut borrow_args = Vec::new(&env);
        borrow_args.push_back(borrow_pool_id.clone().into_val(&env));
        borrow_args.push_back(borrower.clone().into_val(&env));
        borrow_args.push_back(borrow_amount.into_val(&env));
        env.invoke_contract::<()>(
            &lending_pool,
            &Symbol::new(&env, "borrow"),
            borrow_args
        );

        // Create and store loan information
        let now = TimeHelper::now(&env);

        let loan_info = LoanInfo {
            loan_id: loan_id.clone(),
            borrower: borrower.clone(),
            collateral_asset: collateral_asset.clone(),
            collateral_amount,
            borrow_asset: borrow_asset.clone(),
            borrow_amount,
            status: LoanStatus::Active,
            created_at: now,
            last_updated: now,
            interest_accrued: 0,
        };

        env.storage()
            .persistent()
            .set(&BorrowingKey::Loan(loan_id.clone()), &loan_info);
        env.storage()
            .persistent()
            .set(&BorrowingKey::LoanExists(loan_id.clone()), &true);
        env.storage()
            .persistent()
            .set(&BorrowingKey::CollateralLocked(loan_id.clone()), &collateral_vault_id);

        // Update user's loan list
        let user_loans_key = BorrowingKey::UserLoans(borrower.clone());
        let mut user_loans: Vec<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&user_loans_key)
            .unwrap_or(Vec::new(&env));
        user_loans.push_back(loan_id.clone());
        env.storage().persistent().set(&user_loans_key, &user_loans);

        // Update global loan count
        let mut count: u64 = env
            .storage()
            .persistent()
            .get(&BorrowingKey::GlobalLoanCount)
            .unwrap_or(0);
        count = count.checked_add(1).ok_or(Error::Overflow)?;
        env.storage()
            .persistent()
            .set(&BorrowingKey::GlobalLoanCount, &count);

        // Emit events
        env.events()
            .publish((LoanIssued::topic(&env), loan_id.clone()), LoanIssued {
                loan_id: loan_id.clone(),
                borrower: borrower.clone(),
                amount: borrow_amount,
                collateral: collateral_amount,
            });

        env.events()
            .publish((CollateralLocked::topic(&env), loan_id.clone()), CollateralLocked {
                loan_id,
                vault_id: collateral_vault_id,
                amount: collateral_amount,
            });

        Ok(())
    }

    /// Accrue interest on an active loan
    pub fn accrue_interest(
        env: Env,
        loan_id: BytesN<32>,
    ) -> Result<(), Error> {
        let mut loan_info: LoanInfo = env
            .storage()
            .persistent()
            .get(&BorrowingKey::Loan(loan_id.clone()))
            .ok_or(Error::LoanNotFound)?;

        if loan_info.status != LoanStatus::Active {
            return Err(Error::InvalidParameters);
        }

        let lending_pool: Address = env.storage().persistent()
            .get(&BorrowingKey::LendingPoolAddress)
            .ok_or(Error::NotInitialized)?;

        // Get interest rate from lending pool
        let mut rate_args = Vec::new(&env);
        rate_args.push_back(loan_info.borrow_asset.clone().into_val(&env));
        let interest_rate: i128 = env.invoke_contract(
            &lending_pool,
            &Symbol::new(&env, "get_interest_rate"),
            rate_args
        );

        let now = TimeHelper::now(&env);
        let time_elapsed = now - loan_info.last_updated;
        
        // Calculate interest: principal * rate * time / (seconds per year * 10000 bps)
        let seconds_per_year = 31536000i128;
        let interest = SafeMath::div(
            SafeMath::mul(
                SafeMath::mul(loan_info.borrow_amount, interest_rate).ok_or(Error::Overflow)?,
                time_elapsed as i128
            ).ok_or(Error::Overflow)?,
            SafeMath::mul(seconds_per_year, 10000i128).ok_or(Error::Overflow)?
        ).ok_or(Error::Overflow)?;

        loan_info.interest_accrued = SafeMath::add(loan_info.interest_accrued, interest)
            .ok_or(Error::Overflow)?;
        loan_info.last_updated = now;

        env.storage()
            .persistent()
            .set(&BorrowingKey::Loan(loan_id), &loan_info);

        Ok(())
    }

    /// Repay a loan - complete implementation with interest handling
    pub fn repay(
        env: Env,
        loan_id: BytesN<32>,
        repayer: Address,
        principal_amount: i128,
        interest_amount: i128,
    ) -> Result<(), Error> {
        if !ValidationHelper::validate_positive_amount(principal_amount) {
            return Err(Error::InvalidAmount);
        }

        let mut loan_info: LoanInfo = env
            .storage()
            .persistent()
            .get(&BorrowingKey::Loan(loan_id.clone()))
            .ok_or(Error::LoanNotFound)?;

        if loan_info.status != LoanStatus::Active {
            return Err(Error::InvalidParameters);
        }

        if loan_info.borrower != repayer {
            return Err(Error::Unauthorized);
        }

        repayer.require_auth();

        // Accrue interest before processing repayment
        Self::accrue_interest(env.clone(), loan_id.clone())?;

        // Refresh loan info after interest accrual
        loan_info = env.storage().persistent()
            .get(&BorrowingKey::Loan(loan_id.clone()))
            .ok_or(Error::LoanNotFound)?;

        let total_payment = SafeMath::add(principal_amount, interest_amount)
            .ok_or(Error::Overflow)?;

        // Validate repayment doesn't exceed owed amount
        let total_owed = SafeMath::add(loan_info.borrow_amount, loan_info.interest_accrued)
            .ok_or(Error::Overflow)?;

        if total_payment > total_owed {
            return Err(Error::InvalidAmount);
        }

        let lending_pool: Address = env.storage().persistent()
            .get(&BorrowingKey::LendingPoolAddress)
            .ok_or(Error::NotInitialized)?;

        // Transfer repayment to lending pool
        let mut repay_args = Vec::new(&env);
        repay_args.push_back(loan_info.borrow_asset.clone().into_val(&env));
        repay_args.push_back(repayer.clone().into_val(&env));
        repay_args.push_back(principal_amount.into_val(&env));
        repay_args.push_back(interest_amount.into_val(&env));
        env.invoke_contract::<()>(
            &lending_pool,
            &Symbol::new(&env, "repay"),
            repay_args
        );

        // Update loan state
        loan_info.borrow_amount = SafeMath::sub(loan_info.borrow_amount, principal_amount)
            .ok_or(Error::Underflow)?;
        loan_info.interest_accrued = SafeMath::sub(loan_info.interest_accrued, interest_amount)
            .ok_or(Error::Underflow)?;
        loan_info.last_updated = TimeHelper::now(&env);

        let is_fully_repaid = loan_info.borrow_amount == 0 && loan_info.interest_accrued == 0;
        
        if is_fully_repaid {
            loan_info.status = LoanStatus::Repaid;

            // Release collateral - unlock the vault
            let collateral_vault_id: BytesN<32> = env.storage().persistent()
                .get(&BorrowingKey::CollateralLocked(loan_id.clone()))
                .ok_or(Error::LoanNotFound)?;

            let vault_contract: Address = env.storage().persistent()
                .get(&BorrowingKey::VaultContractAddress)
                .ok_or(Error::NotInitialized)?;

            let mut unlock_args = Vec::new(&env);
            unlock_args.push_back(collateral_vault_id.clone().into_val(&env));
            env.invoke_contract::<()>(
                &vault_contract,
                &Symbol::new(&env, "unlock_collateral_vault"),
                unlock_args
            );

            // Remove collateral lock tracking
            env.storage().persistent().remove(&BorrowingKey::CollateralLocked(loan_id.clone()));

            // Emit collateral released event
            env.events()
                .publish((CollateralReleased::topic(&env), loan_id.clone()), CollateralReleased {
                    loan_id: loan_id.clone(),
                    vault_id: collateral_vault_id,
                });
        }

        env.storage()
            .persistent()
            .set(&BorrowingKey::Loan(loan_id.clone()), &loan_info);

        // Emit repayment event
        env.events()
            .publish((LoanRepaid::topic(&env), loan_id.clone()), LoanRepaid {
                loan_id,
                borrower: repayer,
                amount_repaid: total_payment,
            });

        Ok(())
    }

    /// Add additional collateral to a loan
    pub fn add_collateral(
        env: Env,
        loan_id: BytesN<32>,
        additional_amount: i128,
    ) -> Result<(), Error> {
        if !ValidationHelper::validate_positive_amount(additional_amount) {
            return Err(Error::InvalidAmount);
        }

        let mut loan_info: LoanInfo = env
            .storage()
            .persistent()
            .get(&BorrowingKey::Loan(loan_id.clone()))
            .ok_or(Error::LoanNotFound)?;

        if loan_info.status != LoanStatus::Active {
            return Err(Error::InvalidParameters);
        }

        loan_info.borrower.require_auth();

        loan_info.collateral_amount = SafeMath::add(loan_info.collateral_amount, additional_amount)
            .ok_or(Error::Overflow)?;
        loan_info.last_updated = TimeHelper::now(&env);

        env.storage()
            .persistent()
            .set(&BorrowingKey::Loan(loan_id), &loan_info);

        Ok(())
    }

    /// Get loan details
    pub fn get_loan(env: Env, loan_id: BytesN<32>) -> Result<LoanInfo, Error> {
        let loan_info: LoanInfo = env
            .storage()
            .persistent()
            .get(&BorrowingKey::Loan(loan_id))
            .ok_or(Error::LoanNotFound)?;

        Ok(loan_info)
    }

    /// Check if a loan is undercollateralized
    pub fn is_undercollateralized(env: Env, loan_id: BytesN<32>) -> Result<bool, Error> {
        let mut loan_info: LoanInfo = env
            .storage()
            .persistent()
            .get(&BorrowingKey::Loan(loan_id))
            .ok_or(Error::LoanNotFound)?;

        // Accrue interest to get current total debt
        if loan_info.status == LoanStatus::Active {
            Self::accrue_interest(env.clone(), loan_id.clone())?;
            loan_info = env.storage().persistent().get(&BorrowingKey::Loan(loan_id)).unwrap();
        }

        let collateral_config: CollateralConfig = env
            .storage()
            .persistent()
            .get(&BorrowingKey::CollateralConfig(
                loan_info.collateral_asset.clone(),
            ))
            .ok_or(Error::InvalidCollateral)?;

        let total_debt = SafeMath::add(loan_info.borrow_amount, loan_info.interest_accrued)
            .ok_or(Error::Overflow)?;

        if total_debt == 0 {
            return Ok(false);
        }

        let current_ratio = SafeMath::div(
            SafeMath::mul(loan_info.collateral_amount, 10000).ok_or(Error::Overflow)?,
            total_debt,
        ).ok_or(Error::Overflow)?;

        Ok(current_ratio < collateral_config.liquidation_threshold)
    }

    /// Liquidate an undercollateralized loan
    pub fn liquidate(
        env: Env,
        loan_id: BytesN<32>,
        liquidator: Address,
    ) -> Result<(), Error> {
        let mut loan_info: LoanInfo = env
            .storage()
            .persistent()
            .get(&BorrowingKey::Loan(loan_id.clone()))
            .ok_or(Error::LoanNotFound)?;

        if loan_info.status != LoanStatus::Active {
            return Err(Error::InvalidParameters);
        }

        if !Self::is_undercollateralized(env.clone(), loan_id.clone())? {
            return Err(Error::InvalidParameters);
        }

        liquidator.require_auth();

        // Mark loan as liquidated
        loan_info.status = LoanStatus::Liquidated;
        loan_info.last_updated = TimeHelper::now(&env);

        // Get collateral vault and release it to liquidator
        let collateral_vault_id: BytesN<32> = env.storage().persistent()
            .get(&BorrowingKey::CollateralLocked(loan_id.clone()))
            .ok_or(Error::LoanNotFound)?;

        let vault_contract: Address = env.storage().persistent()
            .get(&BorrowingKey::VaultContractAddress)
            .ok_or(Error::NotInitialized)?;

        // Transfer vault ownership to liquidator
        let mut transfer_args = Vec::new(&env);
        transfer_args.push_back(collateral_vault_id.clone().into_val(&env));
        transfer_args.push_back(liquidator.clone().into_val(&env));
        env.invoke_contract::<()>(
            &vault_contract,
            &Symbol::new(&env, "transfer_vault_ownership"),
            transfer_args
        );

        // Remove collateral lock tracking
        env.storage().persistent().remove(&BorrowingKey::CollateralLocked(loan_id.clone()));

        env.storage()
            .persistent()
            .set(&BorrowingKey::Loan(loan_id.clone()), &loan_info);

        // Emit liquidation event
        env.events()
            .publish((LoanLiquidated::topic(&env), loan_id.clone()), LoanLiquidated {
                loan_id,
                liquidator,
                collateral_seized: loan_info.collateral_amount,
                remaining_debt: loan_info.borrow_amount,
            });

        Ok(())
    }
}

        env.storage()
            .persistent()
            .set(&BorrowingKey::Loan(loan_id.clone()), &loan_info);

        Ok(())
    }

    /// Get user's loans
    pub fn get_user_loans(env: Env, user: Address) -> Result<Vec<BytesN<32>>, Error> {
        let user_loans: Vec<BytesN<32>> = env
            .storage()
            .persistent()
            .get(&BorrowingKey::UserLoans(user))
            .unwrap_or(Vec::new(&env));

        Ok(user_loans)
    }

    /// Get global loan count
    pub fn get_loan_count(env: Env) -> Result<u64, Error> {
        let count: u64 = env
            .storage()
            .persistent()
            .get(&BorrowingKey::GlobalLoanCount)
            .unwrap_or(0);

        Ok(count)
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
            collateral_asset: Asset { code: collateral_asset, issuer: borrower.clone() },
            collateral_amount,
            borrow_asset: Asset { code: borrow_asset, issuer: borrower.clone() },
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

    fn derive_loan_id(env: &Env, _borrower: &Address, borrow_asset: &BytesN<32>, collateral_asset: &BytesN<32>) -> BytesN<32> {
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