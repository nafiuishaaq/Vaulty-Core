use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, Map,
};
use vault::{
    interest::{calculate_interest, SECONDS_PER_YEAR},
    VaultContract, VaultContractClient, VaultId,
};

#[contracttype]
#[derive(Clone, Copy)]
pub enum TokenError {
    InsufficientBalance = 1,
    InvalidAmount = 4,
}

#[contract]
pub struct TokenMock;

#[contractimpl]
impl TokenMock {
    fn balances(env: &Env) -> Map<Address, i128> {
        env.storage()
            .persistent()
            .get(&"balances")
            .unwrap_or_else(|| Map::new(env))
    }

    fn save_balances(env: &Env, balances: Map<Address, i128>) {
        env.storage().persistent().set(&"balances", &balances);
    }

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

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        if amount <= 0 {
            panic!("{:?}", TokenError::InvalidAmount);
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
}

fn setup() -> (Env, Address, Address, BytesN<32>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    env.ledger().set(soroban_sdk::LedgerInfo {
        timestamp: 1000,
        protocol_version: 20,
        sequence_number: 1,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });

    let contract_id = env.register_contract(None, VaultContract);
    let owner = Address::generate(&env);
    let token = env.register_contract(None, TokenMock);
    let symbol = BytesN::from_array(&env, &[0u8; 32]);

    (env, contract_id, owner, symbol, token)
}

fn set_ledger_time(env: &Env, timestamp: u64) {
    env.ledger().set(soroban_sdk::LedgerInfo {
        timestamp,
        protocol_version: 20,
        sequence_number: 1234,
        network_id: Default::default(),
        base_reserve: 10,
        min_persistent_entry_ttl: 10,
        min_temp_entry_ttl: 10,
        max_entry_ttl: 31104000,
    });
}

// ---------------------------------------------------------------------------
// 1. Repeated accrual at the same timestamp creates NO additional yield
// ---------------------------------------------------------------------------
#[test]
fn test_repeated_accrual_same_timestamp_creates_no_additional_yield() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64 * 30; // 30 days
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    // Initial last_accrual timestamp should equal vault created_at (1000)
    assert_eq!(client.get_last_accrual_time(&vault_id), 1000);
    assert_eq!(client.get_accrued_interest(&vault_id), 0);

    // Mint and deposit principal
    TokenMock::mint(env.clone(), owner.clone(), 10_000);
    client.deposit(&vault_id, &owner, &10_000);

    // Advance time by 10 days (864,000 seconds)
    set_ledger_time(&env, 1000 + 864_000);

    // First accrual at T = 1000 + 864_000
    let yield_1 = client.accrue_vault_interest(&vault_id);
    assert!(yield_1 > 0, "First accrual should produce interest");
    let accumulated_after_first = client.get_accrued_interest(&vault_id);
    assert_eq!(accumulated_after_first, yield_1);
    assert_eq!(client.get_last_accrual_time(&vault_id), 1000 + 864_000);

    // Second accrual at the EXACT SAME timestamp
    let yield_2 = client.accrue_vault_interest(&vault_id);
    assert_eq!(yield_2, 0, "Repeated accrual at the same timestamp must produce 0 yield");
    assert_eq!(
        client.get_accrued_interest(&vault_id),
        accumulated_after_first,
        "Total accrued interest must remain unchanged"
    );

    // Third accrual at the EXACT SAME timestamp
    let yield_3 = client.accrue_vault_interest(&vault_id);
    assert_eq!(yield_3, 0, "Subsequent accruals at the same timestamp must produce 0 yield");
    assert_eq!(
        client.get_accrued_interest(&vault_id),
        accumulated_after_first
    );
}

// ---------------------------------------------------------------------------
// 2. Sequential accrual periods do not overlap
// ---------------------------------------------------------------------------
#[test]
fn test_sequential_accrual_periods_do_not_overlap() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64 * 365; // 1 year
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    TokenMock::mint(env.clone(), owner.clone(), 100_000);
    client.deposit(&vault_id, &owner, &100_000);

    // Accrue over period 1: [1000, 1000 + 100_000] (elapsed = 100,000s)
    set_ledger_time(&env, 1000 + 100_000);
    let yield_period_1 = client.accrue_vault_interest(&vault_id);
    assert_eq!(client.get_last_accrual_time(&vault_id), 1000 + 100_000);

    // Accrue over period 2: [1000 + 100_000, 1000 + 300_000] (elapsed = 200,000s)
    set_ledger_time(&env, 1000 + 300_000);
    let yield_period_2 = client.accrue_vault_interest(&vault_id);
    assert_eq!(client.get_last_accrual_time(&vault_id), 1000 + 300_000);

    let total_split_yield = yield_period_1 + yield_period_2;
    assert_eq!(client.get_accrued_interest(&vault_id), total_split_yield);

    // Compare with a direct single-period accrual of 300,000s on the same principal:
    let expected_total = calculate_interest(100_000, 500, 300_000).unwrap();
    // Because integer division may have +/- 1 unit difference at most:
    let diff = (total_split_yield - expected_total).abs();
    assert!(
        diff <= 1,
        "Split period accruals must equal combined period accrual without overlapping: split={}, combined={}",
        total_split_yield,
        expected_total
    );
}

// ---------------------------------------------------------------------------
// 3. Withdrawable balance is strictly backed and NOT inflated by unbacked yield
// ---------------------------------------------------------------------------
#[test]
fn test_withdrawable_balance_unaffected_by_unbacked_yield() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64 * 30; // 30 days
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    TokenMock::mint(env.clone(), owner.clone(), 5_000);
    client.deposit(&vault_id, &owner, &5_000);

    // Verify initial balance equals deposit
    assert_eq!(client.get_balance(&vault_id), 5_000);

    // Advance time past unlock time
    set_ledger_time(&env, 1000 + lock_period + 10_000);

    // Explicitly accrue interest
    let yield_accrued = client.accrue_vault_interest(&vault_id);
    assert!(yield_accrued > 0);

    // Informational yield is recorded
    assert_eq!(client.get_accrued_interest(&vault_id), yield_accrued);

    // CRITICAL: Withdrawable balance must remain strictly equal to 5,000 deposited tokens
    assert_eq!(
        client.get_balance(&vault_id),
        5_000,
        "Withdrawable balance must not be inflated by unbacked interest"
    );

    // Withdraw full principal
    let recipient = Address::generate(&env);
    client.withdraw(&vault_id, &recipient, &5_000);

    // Vault balance should now be 0
    assert_eq!(client.get_balance(&vault_id), 0);
    assert_eq!(TokenMock::balance(env.clone(), recipient), 5_000);
}

// ---------------------------------------------------------------------------
// 4. Multiple deposits update last_accrual timestamp without overlapping
// ---------------------------------------------------------------------------
#[test]
fn test_multiple_deposits_sequential_accrual() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64 * 100;
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    TokenMock::mint(env.clone(), owner.clone(), 20_000);

    // Deposit 10,000 at T = 1000
    client.deposit(&vault_id, &owner, &10_000);
    assert_eq!(client.get_balance(&vault_id), 10_000);
    assert_eq!(client.get_last_accrual_time(&vault_id), 1000);

    // Advance to T = 1000 + 50_000
    set_ledger_time(&env, 1000 + 50_000);

    // Deposit second 10,000 at T = 1000 + 50_000
    // `deposit` accrues interest up to current timestamp
    client.deposit(&vault_id, &owner, &10_000);
    assert_eq!(client.get_balance(&vault_id), 20_000);
    assert_eq!(client.get_last_accrual_time(&vault_id), 1000 + 50_000);

    let interest_after_dep2 = client.get_accrued_interest(&vault_id);
    let expected_interest_dep1 = calculate_interest(10_000, 500, 50_000).unwrap();
    assert_eq!(interest_after_dep2, expected_interest_dep1);

    // Advance to T = 1000 + 100_000
    set_ledger_time(&env, 1000 + 100_000);

    // Accrue interest for second period on new 20,000 balance
    let yield_period_2 = client.accrue_vault_interest(&vault_id);
    let expected_interest_dep2 = calculate_interest(20_000, 500, 50_000).unwrap();
    assert_eq!(yield_period_2, expected_interest_dep2);
    assert_eq!(
        client.get_accrued_interest(&vault_id),
        interest_after_dep2 + yield_period_2
    );

    // Balance remains exact backed principal
    assert_eq!(client.get_balance(&vault_id), 20_000);
}

// ---------------------------------------------------------------------------
// 5. Zero interest rate or zero balance edge cases
// ---------------------------------------------------------------------------
#[test]
fn test_zero_rate_and_zero_balance_edge_cases() {
    let (env, contract_id, owner, symbol, token) = setup();
    let client = VaultContractClient::new(&env, &contract_id);

    let lock_period = 86400u64 * 30;
    let vault_id = client.create_vault(&owner, &token, &symbol, &lock_period);

    // Vault has 0 balance
    set_ledger_time(&env, 1000 + 500_000);
    let zero_bal_yield = client.accrue_vault_interest(&vault_id);
    assert_eq!(zero_bal_yield, 0, "Zero balance must accrue zero interest");
    assert_eq!(client.get_accrued_interest(&vault_id), 0);
    assert_eq!(client.get_last_accrual_time(&vault_id), 1000 + 500_000);

    // Pure math checks
    assert_eq!(calculate_interest(0, 500, 1000).unwrap(), 0);
    assert_eq!(calculate_interest(10_000, 0, 1000).unwrap(), 0);
    assert_eq!(calculate_interest(10_000, 500, 0).unwrap(), 0);
    assert_eq!(calculate_interest(-100, 500, 1000).unwrap(), 0);
    assert_eq!(calculate_interest(10_000, -500, 1000).unwrap(), 0);
}

// ---------------------------------------------------------------------------
// 6. Annualized calculation consistency
// ---------------------------------------------------------------------------
#[test]
fn test_full_year_interest_calculation() {
    // 100,000 tokens at 5.00% (500 bps) for exactly 1 year (31,536,000 seconds)
    // Expected interest: 100,000 * 5% = 5,000 tokens
    let interest = calculate_interest(100_000, 500, SECONDS_PER_YEAR as u64).unwrap();
    assert_eq!(interest, 5_000);
}
