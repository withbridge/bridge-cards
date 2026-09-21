//! Legacy instructions — kept for production continuity while debit_user is still live.
//! These instructions are candidates for removal in a future migration once debit_user
//! is fully replaced by transfer_using_legacy_delegate.
//!
//! Do not add new functionality here.

pub mod add_or_update_merchant_debitor;
pub mod add_or_update_merchant_destination;
pub mod add_or_update_merchant_manager;
pub mod add_or_update_user_delegate;
pub mod close_account;
pub mod debit_user;
pub mod initialize;
pub mod update_admin;

pub use add_or_update_merchant_debitor::*;
pub use add_or_update_merchant_destination::*;
pub use add_or_update_merchant_manager::*;
pub use add_or_update_user_delegate::*;
pub use close_account::*;
pub use debit_user::*;
pub use initialize::*;
pub use update_admin::*;
