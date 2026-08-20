use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, LedgerInfo};
use borrowing::BorrowingContract;

/// Regression tests for issue #104: the simplified loan-ID derivation must
/// fold the borrower's address into the ID so that two different borrowers
/// requesting the same asset pair at the same ledger timestamp never
/// collide on the same loan ID.

fn fixed_ledger_info(timestamp: u64) -> LedgerInfo {
    LedgerInfo {
        timestamp,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    }
}

#[test]
fn different_borrowers_same_timestamp_and_vault_get_different_loan_ids() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, BorrowingContract);

    // Freeze the ledger timestamp so both borrowers derive their loan ID
    // under identical timestamp/vault inputs - the exact collision
    // scenario described in the issue.
    env.ledger().set(fixed_ledger_info(1_000_000));

    let collateral_vault_id = BytesN::from_array(&env, &[7u8; 32]);
    let borrower_a = Address::generate(&env);
    let borrower_b = Address::generate(&env);

    let loan_a = env.as_contract(&contract_id, || {
        BorrowingContract::create_loan(
            env.clone(),
            borrower_a.clone(),
            collateral_vault_id.clone(),
            2_000i128,
            1_000i128,
        )
    });

    let loan_b = env.as_contract(&contract_id, || {
        BorrowingContract::create_loan(
            env.clone(),
            borrower_b.clone(),
            collateral_vault_id.clone(),
            2_000i128,
            1_000i128,
        )
    });

    assert_ne!(
        loan_a.loan_id, loan_b.loan_id,
        "loan IDs must differ across borrowers even with identical timestamp and vault"
    );
    assert_eq!(loan_a.borrower, borrower_a);
    assert_eq!(loan_b.borrower, borrower_b);
}

#[test]
fn loan_id_derivation_is_deterministic_for_identical_inputs() {
    // Two independent contract instances, given the exact same borrower,
    // vault and ledger timestamp, must derive the exact same loan ID.
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(fixed_ledger_info(2_000_000));

    let collateral_vault_id = BytesN::from_array(&env, &[9u8; 32]);
    let borrower = Address::generate(&env);

    let contract_id_1 = env.register_contract(None, BorrowingContract);
    let loan_1 = env.as_contract(&contract_id_1, || {
        BorrowingContract::create_loan(
            env.clone(),
            borrower.clone(),
            collateral_vault_id.clone(),
            2_000i128,
            1_000i128,
        )
    });

    let contract_id_2 = env.register_contract(None, BorrowingContract);
    let loan_2 = env.as_contract(&contract_id_2, || {
        BorrowingContract::create_loan(
            env.clone(),
            borrower.clone(),
            collateral_vault_id.clone(),
            2_000i128,
            1_000i128,
        )
    });

    assert_eq!(loan_1.loan_id, loan_2.loan_id);
}

#[test]
#[should_panic(expected = "LoanAlreadyExists")]
fn duplicate_loan_protection_is_retained_for_the_same_borrower() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, BorrowingContract);

    env.ledger().set(fixed_ledger_info(3_000_000));

    let collateral_vault_id = BytesN::from_array(&env, &[3u8; 32]);
    let borrower = Address::generate(&env);

    env.as_contract(&contract_id, || {
        BorrowingContract::create_loan(
            env.clone(),
            borrower.clone(),
            collateral_vault_id.clone(),
            2_000i128,
            1_000i128,
        );

        // Same borrower, same vault, same timestamp: must still be
        // rejected as a duplicate loan.
        BorrowingContract::create_loan(
            env.clone(),
            borrower.clone(),
            collateral_vault_id.clone(),
            2_000i128,
            1_000i128,
        );
    });
}
