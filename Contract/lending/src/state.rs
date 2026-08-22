//! Lending contract state layout.
//!
//! ## Borrowing Contract Authorization
//!
//! Each lending pool stores the address of its authorized borrowing contract
//! under `PoolKey::BorrowingContract(pool_id)` in persistent storage.
//!
//! Once set via [`LendingContract::initialize_borrowing_contract`], this address
//! is **immutable** — it cannot be changed or removed without a contract upgrade.
//!
//! Only the configured borrowing contract may call:
//! - `borrow` — draw liquidity from the pool
//! - `repay` — return principal and interest
//! - `update_debt` — sync outstanding debt accounting
//!
//! All read-only endpoints (`get_pool_accounting`, `get_pool_balance`,
//! `calculate_interest`, `get_share_balance`, `get_borrowing_contract`)
//! remain public.
//!
//! ### Initialization Order
//!
//! 1. Deploy the lending contract.
//! 2. Deploy the borrowing contract.
//! 3. Create a lending pool via `LendingContract::create_pool`.
//! 4. The pool admin calls `initialize_borrowing_contract` to bind the pool
//!    to the borrowing contract.  This is a one-time, irreversible operation.
//! 5. The pool admin configures the lending pool address inside the borrowing
//!    contract.
//! 6. Suppliers call `deposit` to add liquidity.
//! 7. Borrowers request loans through the borrowing contract, which
//!    internally calls `borrow` and `repay` on the lending contract.
