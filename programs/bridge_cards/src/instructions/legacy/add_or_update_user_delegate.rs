use crate::events::UserDelegateAddedOrUpdated;
use crate::state::{MerchantManagerState, UserDelegateState};
use crate::{ID, MERCHANT_MANAGER_SEED};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};

/// Seed used to derive user delegate PDAs
pub const USER_DELEGATE_SEED: &[u8] = b"user_delegate";

/**
 * Add or update a user delegate account for token transfers.
 *
 * This instruction allows a merchant manager to create or update a delegate account
 * that can initiate transfers from a user's token account. The delegate is configured
 * with specific transfer limits to control transaction amounts.
 *
 * Delegate Configuration:
 * - Per-transaction limit: Maximum amount for a single transfer
 * - Period limit: Maximum amount within a time window
 * - Period duration: Length of the time window in seconds
 *
 * Account Creation:
 * - Creates a PDA to store delegate parameters if it doesn't exist
 * - PDA is derived using [USER_DELEGATE_SEED, merchant_id, mint, user_token_account]
 * - Funded by the payer account
 *
 * Security Model:
 * - Only merchant managers can create/update delegates
 * - Each delegate is specific to a merchant-user-mint combination
 * - Transfer limits provide spending controls
 * - Period tracking prevents excessive transfers
 *
 * Transfer Limit Examples:
 * - Per-transaction: $100 maximum per transfer
 * - Period limit: $2000 maximum per day
 * - Period: 86400 seconds (24 hours)
 *
 * Events Emitted:
 * - UserDelegateAddedOrUpdated: When a delegate is created or updated
 *   Fields: merchant_pda, user_delegate
 *
 * Common Use Cases:
 * - Setting up new merchant-user relationships
 * - Adjusting transfer limits
 * - Enabling recurring payments
 *
 * Required Accounts:
 * - manager: Merchant manager who can create delegates
 * - payer: Account paying for PDA creation/rent
 * - manager_state: PDA verifying manager authority
 * - user_token_account: Token account to delegate
 * - mint: Token mint for the delegation
 * - user_delegate_account: PDA storing delegate parameters
 * - system_program: Required for account creation
 */
#[derive(Accounts)]
#[instruction(merchant_id: u64)]
pub struct AddOrUpdateUserDelegate<'info> {
    /// Merchant manager account, must match manager in manager_state
    /// Required permissions: Signer
    #[account( constraint = manager.key() == manager_state.manager)]
    pub manager: Signer<'info>,

    /// Account that will pay for PDA creation and rent
    /// Required permissions: Signer, Mutable (for rent payment)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// PDA storing the merchant manager's authorization
    /// Seeds: [MERCHANT_MANAGER_SEED, merchant_id]
    /// Required permissions: Read-only
    #[account(
        seeds = [MERCHANT_MANAGER_SEED, &merchant_id.to_le_bytes()],
        bump = manager_state.bump,
        seeds::program = ID
    )]
    pub manager_state: Account<'info, MerchantManagerState>,

    /// Token account that will be controlled by the delegate
    /// Required permissions: Read-only
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Mint of the tokens that can be transferred by this delegate
    /// Required permissions: Read-only
    pub mint: InterfaceAccount<'info, Mint>,

    /// PDA storing the delegate's transfer limits and state
    /// Seeds: [USER_DELEGATE_SEED, merchant_id, mint, user_token_account]
    /// Space: Discriminator + Delegate parameters
    /// Required permissions: Mutable if new, Read-only if existing
    #[account(init_if_needed,
        payer=payer,
        space = UserDelegateState::DISCRIMINATOR.len() + UserDelegateState::INIT_SPACE,
        seeds = [
            USER_DELEGATE_SEED,
            merchant_id.to_le_bytes().as_ref(),
            mint.key().as_ref(),
            user_token_account.key().as_ref(),
        ],
        bump
    )]
    pub user_delegate_account: Account<'info, UserDelegateState>,

    /// Required for account creation
    pub system_program: Program<'info, System>,
}

/**
 * Process the addition or update of a user delegate.
 *
 * @param ctx Context containing all required accounts
 * @param _merchant_id Unique identifier for the merchant (used in constraints)
 * @param max_transfer_limit Maximum amount allowed in a single transfer
 * @param period_transfer_limit Maximum amount allowed within the time period
 * @param transfer_limit_period Duration of the transfer limit period in seconds
 *
 * Flow:
 * 1. Verify manager signature (done via account constraints)
 * 2. Update delegate parameters in PDA
 * 3. Emit event with delegate information
 *
 * Note: Period tracking (transferred amount and reset timestamp) is managed
 * during the debit_user instruction, not during setup.
 *
 * @return Result indicating success or containing an error
 */
pub fn handler(
    ctx: Context<AddOrUpdateUserDelegate>,
    merchant_id: u64,
    max_transfer_limit: u64,
    period_transfer_limit: u64,
    transfer_limit_period: u32,
) -> Result<()> {
    let user_delegate_account = &mut ctx.accounts.user_delegate_account;

    // Set the maximum amount allowed per transaction
    user_delegate_account.per_transfer_limit = max_transfer_limit;

    // Set the maximum amount allowed within the time period
    user_delegate_account.period_transfer_limit = period_transfer_limit;

    // Set the duration of the transfer limit period in seconds
    user_delegate_account.transfer_limit_period_seconds = transfer_limit_period;

    user_delegate_account.bump = ctx.bumps.user_delegate_account;

    // Emit event for indexing and notifications
    emit!(UserDelegateAddedOrUpdated {
        merchant_id,
        mint: ctx.accounts.mint.key(),
        user_ata: ctx.accounts.user_token_account.key(),
        user_delegate: ctx.accounts.user_delegate_account.key(),
    });

    Ok(())
}
