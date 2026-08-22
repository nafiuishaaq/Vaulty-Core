use shared::storage::StorageHelper;
use soroban_sdk::Env;

pub struct Ttl;

impl Ttl {
    /// Refresh TTL for the authorized-callers state.
    pub fn refresh_authorized_callers(env: &Env) {
        StorageHelper::touch_instance(env);
    }
}
