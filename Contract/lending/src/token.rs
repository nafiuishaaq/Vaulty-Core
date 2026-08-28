use soroban_sdk::{xdr::Hash, xdr::ScAddress, Address, BytesN, Env, TryFromVal};
use crate::LendingContract;
use shared::errors::Error;
use soroban_sdk::token::Client as TokenClient;

/// Convert a `BytesN<32>` contract identifier into a Soroban `Address`.
fn asset_to_address(env: &Env, asset: &BytesN<32>) -> Result<Address, Error> {
    let hash = Hash(asset.to_array());
    let sc_addr = ScAddress::Contract(hash);
    Address::try_from_val(env, &sc_addr).map_err(|_| Error::InvalidParameters)
}

impl LendingContract {
    /// Transfers tokens from the lending contract to the borrower.
    pub(crate) fn transfer_to_borrower(
        env: &Env,
        asset: &BytesN<32>,
        to: &Address,
        amount: &i128,
    ) -> Result<(), Error> {
        let asset_addr = asset_to_address(env, asset)?;
        let token_client = TokenClient::new(env, &asset_addr);
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
        let asset_addr = asset_to_address(env, asset)?;
        let token_client = TokenClient::new(env, &asset_addr);
        token_client.transfer(from, &env.current_contract_address(), amount);
        Ok(())
    }
}
