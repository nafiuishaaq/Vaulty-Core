use soroban_sdk::{contracttype, Address, BytesN};

/// Storage keys for vault contract
#[derive(Clone)]
#[contracttype]
pub enum VaultKey {
    Vault(VaultId),
    Balance(VaultId),
    VaultCounter,
    VaultConfig,
    EmergencyStop,
    RateLimit,
    Admin,
    AdminPermissions(Address),
    UserVaults(Address),
    VaultInterest(VaultId),
    LastAccrual(VaultId),
}

/// Configuration settings for the vault contract
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultConfig {
    pub max_vaults_per_user: u64,
    pub min_lock_period: u64,
    pub max_lock_period: u64,
    pub interest_rate: i128, // Basis points (e.g., 500 = 5%)
    pub auto_compound: bool,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            max_vaults_per_user: 10,
            min_lock_period: 1,
            max_lock_period: 157_788_000, // 5 years in seconds
            interest_rate: 500,           // 5%
            auto_compound: true,
        }
    }
}

/// Unique identifier for a vault
#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultId(pub BytesN<32>);

impl VaultId {
    pub fn new(bytes: BytesN<32>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &BytesN<32> {
        &self.0
    }
}
