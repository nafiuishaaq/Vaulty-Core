use soroban_sdk::{Address, BytesN, Env, IntoVal, Val};

/// Event emitted when a new vault is created
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultCreated {
    pub vault_id: BytesN<32>,
    pub owner: Address,
    pub asset: BytesN<32>,
    pub lock_period: u64,
}

impl IntoVal<Env, Val> for VaultCreated {
    fn into_val(&self, env: &Env) -> Val {
        (self.vault_id.clone(), self.owner.clone(), self.asset.clone(), self.lock_period).into_val(env)
    }
}

/// Event emitted when a deposit is made to a vault
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositMade {
    pub vault_id: BytesN<32>,
    pub depositor: Address,
    pub amount: i128,
}

impl IntoVal<Env, Val> for DepositMade {
    fn into_val(&self, env: &Env) -> Val {
        (self.vault_id.clone(), self.depositor.clone(), self.amount).into_val(env)
    }
}

/// Event emitted when a withdrawal is completed
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithdrawalCompleted {
    pub vault_id: BytesN<32>,
    pub withdrawer: Address,
    pub amount: i128,
}

impl IntoVal<Env, Val> for WithdrawalCompleted {
    fn into_val(&self, env: &Env) -> Val {
        (self.vault_id.clone(), self.withdrawer.clone(), self.amount).into_val(env)
    }
}

/// Event emitted when a vault is unlocked after lock period
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultUnlocked {
    pub vault_id: BytesN<32>,
    pub unlock_time: u64,
}

impl IntoVal<Env, Val> for VaultUnlocked {
    fn into_val(&self, env: &Env) -> Val {
        (self.vault_id.clone(), self.unlock_time).into_val(env)
    }
}

/// Event emitted when a user's streak is updated
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreakUpdated {
    pub user: Address,
    pub streak_count: u32,
    pub last_activity: u64,
}

impl IntoVal<Env, Val> for StreakUpdated {
    fn into_val(&self, env: &Env) -> Val {
        (self.user.clone(), self.streak_count, self.last_activity).into_val(env)
    }
}

/// Event emitted when a loan is issued
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoanIssued {
    pub loan_id: BytesN<32>,
    pub borrower: Address,
    pub amount: i128,
    pub collateral: i128,
}

impl IntoVal<Env, Val> for LoanIssued {
    fn into_val(&self, env: &Env) -> Val {
        (self.loan_id.clone(), self.borrower.clone(), self.amount, self.collateral).into_val(env)
    }
}

/// Event emitted when a loan is repaid
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoanRepaid {
    pub loan_id: BytesN<32>,
    pub borrower: Address,
    pub amount_repaid: i128,
}

impl IntoVal<Env, Val> for LoanRepaid {
    fn into_val(&self, env: &Env) -> Val {
        (self.loan_id.clone(), self.borrower.clone(), self.amount_repaid).into_val(env)
    }
}

/// Event emitted when a reward is granted to a user
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardGranted {
    pub recipient: Address,
    pub reward_amount: i128,
    pub reward_type: u32,
    pub milestone_id: BytesN<32>,
}

impl IntoVal<Env, Val> for RewardGranted {
    fn into_val(&self, env: &Env) -> Val {
        (self.recipient.clone(), self.reward_amount, self.reward_type, self.milestone_id.clone()).into_val(env)
    }
}

/// Event emitted when a user claims their reward
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardClaimed {
    pub user: Address,
    pub amount: i128,
    pub milestone_id: BytesN<32>,
}

impl IntoVal<Env, Val> for RewardClaimed {
    fn into_val(&self, env: &Env) -> Val {
        (self.user.clone(), self.amount, self.milestone_id.clone()).into_val(env)
    }
}

/// Event emitted when a streak freeze is used
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreakFreezeUsed {
    pub user: Address,
    pub remaining_freezes: u32,
}

impl IntoVal<Env, Val> for StreakFreezeUsed {
    fn into_val(&self, env: &Env) -> Val {
        (self.user.clone(), self.remaining_freezes).into_val(env)
    }
}

/// Event emitted when a milestone is reached by a user
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneReached {
    pub user: Address,
    pub streak: u32,
    pub milestone_id: BytesN<32>,
}

impl IntoVal<Env, Val> for MilestoneReached {
    fn into_val(&self, env: &Env) -> Val {
        (self.user.clone(), self.streak, self.milestone_id.clone()).into_val(env)
    }
}

/// Event emitted when the rewards pool is funded
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardsPoolFunded {
    pub admin: Address,
    pub amount: i128,
    pub total_pool: i128,
}

impl IntoVal<Env, Val> for RewardsPoolFunded {
    fn into_val(&self, env: &Env) -> Val {
        (self.admin.clone(), self.amount, self.total_pool).into_val(env)
    }
}