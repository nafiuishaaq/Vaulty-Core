use shared::errors::Error;
use soroban_sdk::{Address, Env};

use crate::state::State;

pub struct Authorization;

impl Authorization {
    /// Verify that the caller is an authorized vault contract.
    ///
    /// Uses `require_auth` on the current contract address to verify
    /// that an authorized contract has initiated this call.
    ///
    /// # Errors
    /// Returns `Error::Unauthorized` if:
    /// - The contract has not been initialized
    /// - The caller is not in the authorized callers list
    pub fn require_vault_authorization(env: &Env) -> Result<(), Error> {
        let authorized = State::get_authorized_callers(env)?;

        for i in 0..authorized.len() {
            if let Some(addr) = authorized.get(i) {
                addr.require_auth();
                return Ok(());
            }
        }

        Err(Error::Unauthorized)
    }

    /// Verify that the user has authorized this call via `require_auth`.
    ///
    /// # Errors
    /// Returns `Error::Unauthorized` if the user has not authorized the call.
    pub fn require_user_authorization(user: &Address) -> Result<(), Error> {
        user.require_auth();
        Ok(())
    }

    /// Check if the contract has been initialized.
    ///
    /// # Errors
    /// Returns `Error::NotInitialized` if the authorized callers list has not been set.
    pub fn ensure_initialized(env: &Env) -> Result<(), Error> {
        if !State::is_initialized(env) {
            return Err(Error::NotInitialized);
        }
        Ok(())
    }
}
