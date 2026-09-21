// TODO (brendanryan): Fix import namespacing so that this is no longer required.
#![allow(ambiguous_glob_reexports)]

pub mod legacy;
pub use legacy::*;

pub mod initialize_spender_state;
pub use initialize_spender_state::*;

pub mod update_governor;
pub use update_governor::*;

pub mod update_manager;
pub use update_manager::*;

pub mod update_debitor;
pub use update_debitor::*;

pub mod update_paused;
pub use update_paused::*;

pub mod update_pauser;
pub use update_pauser::*;

pub mod setup_merchant_delegate;
pub use setup_merchant_delegate::*;

pub mod add_delegate_destination;
pub use add_delegate_destination::*;

pub mod close_delegate_destination;
pub use close_delegate_destination::*;

pub mod transfer_using_single_delegate;
pub use transfer_using_single_delegate::*;

pub mod transfer_using_subscription_delegate;
pub use transfer_using_subscription_delegate::*;

pub mod transfer_using_legacy_delegate;
pub use transfer_using_legacy_delegate::*;

pub mod update_spender_admin;
pub use update_spender_admin::*;
