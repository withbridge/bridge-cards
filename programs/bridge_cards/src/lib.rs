/**
 * Bridge Cards Program
 *
 * A Solana program that enables delegated token transfers between users and merchants.
 * The program implements a permission system where merchants can authorize specific accounts to:
 * - Act as delegates for users (initiating transfers on their behalf)
 * - Receive funds (destination accounts)
 * - Manage merchant settings (merchant managers)
 *
 * Key Features:
 * - Delegated transfers with configurable limits
 * - Per-transaction and time-period based transfer limits
 * - Merchant-specific destination account allowlisting
 * - Hierarchical permission system (admin -> merchant manager -> delegates)
 *
 * Security Model:
 * - Program admin controls merchant managers and destination ATAs for merchants
 * - Merchant managers can configure delegate and debitor settings for their merchant
 * - Debitors can only initiate transfers within a delegate's configured limits
 */
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;
pub use instructions::*;
#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;
#[allow(unused_imports)]
use subscriptions;

// Program ID for the Bridge Cards program
declare_id!("cardWArqhdV5jeRXXjUti7cHAa4mj41Nj3Apc6RPZH2");

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "Bridge Cards",
    project_url: "https://github.com/withbridge/bridge-cards",
    contacts: "email:security@bridge.xyz,email:brendan@bridge.xyz,email:james.wenzel@bridge.xyz",
    policy: "https://github.com/withbridge/bridge-cards/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/withbridge/bridge-cards.git",
    source_revision: "b1acaf849dc7f5622ea25702bc0728e7310c12f2",
    auditors: "zenith"
}

#[program]
pub mod bridge_cards {
    use super::*;

    /**
     * Initialize the Bridge Cards program state.
     * Creates the global state account and sets the program admin.
     *
     * @param ctx Context containing:
     *   - The system program for account creation
     *   - The signer who will become the admin
     *   - The state account to initialize
     */
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        instructions::initialize::handler(ctx)
    }

    /**
     * Add or update a user delegate for a specific merchant.
     * Delegates can initiate transfers on behalf of users within configured limits.
     *
     * @param ctx Context containing required accounts
     * @param merchant_id Unique identifier for the merchant
     * @param max_transfer_limit Maximum amount allowed in a single transfer
     * @param period_transfer_limit Maximum amount allowed within the time period
     * @param transfer_limit_period Duration of the transfer limit period in seconds
     */
    pub fn add_or_update_user_delegate(
        ctx: Context<AddOrUpdateUserDelegate>,
        merchant_id: u64,
        max_transfer_limit: u64,
        period_transfer_limit: u64,
        transfer_limit_period: u32,
    ) -> Result<()> {
        instructions::add_or_update_user_delegate::handler(
            ctx,
            merchant_id,
            max_transfer_limit,
            period_transfer_limit,
            transfer_limit_period,
        )
    }

    /**
     * Add or update a merchant destination account.
     * Destination accounts are token accounts authorized to receive transfers for a merchant.
     *
     * @param ctx Context containing required accounts
     * @param merchant_id Unique identifier for the merchant
     * @param destination_allowed Whether the destination account should be allowed to receive funds
     */
    pub fn add_or_update_merchant_destination(
        ctx: Context<AddOrUpdateMerchantDestination>,
        merchant_id: u64,
        destination_allowed: bool,
    ) -> Result<()> {
        instructions::add_or_update_merchant_destination::handler(
            ctx,
            merchant_id,
            destination_allowed,
        )
    }

    /**
     * Add or update a merchant manager.
     * Managers can configure delegate and destination settings for their merchant.
     *
     * @param ctx Context containing required accounts
     * @param merchant_id Unique identifier for the merchant
     */
    pub fn add_or_update_merchant_manager(
        ctx: Context<AddOrUpdateMerchantManager>,
        merchant_id: u64,
    ) -> Result<()> {
        instructions::add_or_update_merchant_manager::handler(ctx, merchant_id)
    }

    /**
     * Add or update a merchant debitor account.
     * Debitor accounts are authorized to initiate transfers from user delegates.
     *
     * @param ctx Context containing required accounts
     * @param merchant_id Unique identifier for the merchant
     * @param debitor_allowed Whether the debitor account should be allowed to initiate transfers
     */
    pub fn add_or_update_merchant_debitor(
        ctx: Context<AddOrUpdateMerchantDebitor>,
        merchant_id: u64,
        debitor_allowed: bool,
    ) -> Result<()> {
        instructions::add_or_update_merchant_debitor::handler(ctx, merchant_id, debitor_allowed)
    }

    /**
     * Debit tokens from a user's account via their delegate.
     * The transfer must be within the delegate's configured limits.
     *
     * @param ctx Context containing required accounts
     * @param merchant_id Unique identifier for the merchant
     * @param amount Amount of tokens to transfer
     */
    pub fn debit_user(ctx: Context<DebitUser>, merchant_id: u64, amount: u64) -> Result<()> {
        instructions::debit_user::handler(ctx, merchant_id, amount)
    }

    /**
     * Update the program admin.
     * Only the current admin can execute this instruction.
     *
     * @param ctx Context containing required accounts
     */
    pub fn update_admin(ctx: Context<UpdateAdmin>) -> Result<()> {
        instructions::update_admin::handler(ctx)
    }

    /**
     * Close a program account and recover its rent.
     * Only the admin can execute this instruction.
     *
     * @param ctx Context containing required accounts
     * @param input_seeds Seeds used to derive the PDA being closed
     */
    pub fn close_account(ctx: Context<CloseAccount>, input_seeds: Vec<Vec<u8>>) -> Result<()> {
        instructions::close_account::handler(ctx, input_seeds)
    }

    /// Initialize the SpenderState PDA (seed: "spender_state") with governor/manager/debitor/pauser roles.
    pub fn initialize_spender_state(
        ctx: Context<InitializeSpenderState>,
        admin: Pubkey,
        governor: Pubkey,
        manager: Pubkey,
        debitor: Pubkey,
        pauser: Pubkey,
    ) -> Result<()> {
        instructions::initialize_spender_state::handler(ctx, admin, governor, manager, debitor, pauser)
    }

    /// Update the governor in SpenderState. Called by admin.
    pub fn update_governor(ctx: Context<UpdateGovernor>) -> Result<()> {
        instructions::update_governor::handler(ctx)
    }

    /// Update the manager in SpenderState. Called by governor.
    pub fn update_manager(ctx: Context<UpdateManager>) -> Result<()> {
        instructions::update_manager::handler(ctx)
    }

    /// Update the debitor in SpenderState. Called by manager.
    pub fn update_debitor(ctx: Context<UpdateDebitor>) -> Result<()> {
        instructions::update_debitor::handler(ctx)
    }

    /// Pause or unpause delegate-based transfers. Called by admin or pauser.
    pub fn update_paused(ctx: Context<UpdatePaused>, paused: bool) -> Result<()> {
        instructions::update_paused::handler(ctx, paused)
    }

    /// Update the pauser in SpenderState. Called by admin.
    pub fn update_pauser(ctx: Context<UpdatePauser>) -> Result<()> {
        instructions::update_pauser::handler(ctx)
    }

    /// Create a MerchantDelegateState PDA and optionally bulk-initialize destination allowlist entries.
    pub fn setup_merchant_delegate<'info>(
        ctx: Context<'info, SetupMerchantDelegate<'info>>,
        merchant_id: [u8; 32],
    ) -> Result<()> {
        instructions::setup_merchant_delegate::handler(ctx, merchant_id)
    }

    /// Allowlist a token account as a valid delegate transfer destination for a merchant.
    pub fn add_delegate_destination(
        ctx: Context<AddDelegateDestination>,
        merchant_id: [u8; 32],
    ) -> Result<()> {
        instructions::add_delegate_destination::handler(ctx, merchant_id)
    }

    /// Remove a token account from a merchant's delegate transfer destination allowlist.
    pub fn close_delegate_destination(
        ctx: Context<CloseDelegateDestination>,
        merchant_id: [u8; 32],
    ) -> Result<()> {
        instructions::close_delegate_destination::handler(ctx, merchant_id)
    }

    /// Execute a token transfer using an SPL-approve delegation (no subscriptions).
    pub fn transfer_using_single_delegate<'info>(
        ctx: Context<'info, TransferUsingSingleDelegate<'info>>,
        merchant_id: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        instructions::transfer_using_single_delegate::handler(ctx, merchant_id, amount)
    }

    /// Execute a token transfer using a fixed or recurring subscription delegation.
    pub fn transfer_using_subscription_delegate<'info>(
        ctx: Context<'info, TransferUsingSubscriptionDelegate<'info>>,
        merchant_id: [u8; 32],
        delegator: Pubkey,
        mint: Pubkey,
        amount: u64,
    ) -> Result<()> {
        instructions::transfer_using_subscription_delegate::handler(ctx, merchant_id, delegator, mint, amount)
    }
}
