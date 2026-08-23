
use soroban_sdk::{contractimpl, Address, BytesN, Env};
use crate::LendingContract;
use shared::errors::Error;
use soroban_sdk::token::Client as TokenClient;

#[contractimpl]
impl LendingContract {
    /// Transfers tokens from the lending contract to the borrower.
    pub(crate) fn transfer_to_borrower(
        env: &Env,
        asset: &BytesN<32>,
        to: &Address,
        amount: &i128,
    ) -> Result<(), Error> {
        let token_client = TokenClient::new(env, asset);
        token_client.transfer(&env.current_contract_address(), to, amount);
        Ok(())
    }

    /// Transfers tokens from the payer to the lending contract.
    /// The payer must have authorized the lending contract to spend their tokens.
    pub(crate) fn transfer_from_payer(
        env: &Env,
        asset: &BytesN<32>,
        from: &Address,
        amount: &i128,
    ) -> Result<(), Error> {
        let token_client = TokenClient::new(env, asset);
        token_client.transfer(from, &env.current_contract_address(), amount);
        Ok(())
    }
}