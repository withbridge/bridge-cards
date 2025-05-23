use crate::events::MerchantDebitorAddedOrUpdated;
use crate::state::{MerchantDebitorState, MerchantManagerState};
use crate::{ID, MERCHANT_MANAGER_SEED};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

/// Seed used to derive merchant debitor PDAs
pub const MERCHANT_DEBITOR_SEED: &[u8] = b"merchant_debitor";

/**
 * Add or update an allowed debitor account for a merchant.
 *
 * This instruction allows a merchant manager to authorize accounts that can initiate
 * transfers from user delegates. Only authorized debitor accounts can call the debit_user
 * instruction for the merchant.
 *
 * Debitor Configuration:
 * - Each debitor is specific to a merchant
 * - Debitors can be enabled or disabled via the debitor_allowed parameter
 * - Multiple debitors can be configured per merchant
 * - Debitors must sign transactions but don't need to sign for revocation
 *
 * Account Creation:
 * - Creates a PDA to store the debitor's state if it doesn't exist
 * - PDA is derived using [MERCHANT_DEBITOR_SEED, merchant_id, debitor]
 * - Funded by the payer account
 *
 * Security Model:
 * - Only merchant managers can add/update debitors
 * - Manager authority is verified through manager_state PDA
 * - Debitor account does not need to sign for revocation (allows manager to revoke access)
 * - State is stored in a PDA unique to the merchant-debitor combination
 *
 * Events Emitted:
 * - MerchantDebitorAddedOrUpdated: When a debitor is set or changed
 *   Fields: merchant_id, debitor, state_pda, previous_state, new_state
 *
 * Common Use Cases:
 * - Initial setup of merchant payment processors
 * - Adding backup/alternate payment processors
 * - Rotating debitor accounts
 * - Disabling compromised debitors
 *
 * Required Accounts:
 * - manager: Merchant manager who can update debitors
 * - payer: Account paying for PDA creation/rent
 * - manager_state: PDA verifying manager authority
 * - debitor_state: PDA storing debitor authorization
 * - debitor: Account to be authorized as debitor
 * - mint: Token mint account that this debitor is authorized for
 * - system_program: Required for account creation
 */
#[derive(Accounts)]
#[instruction(merchant_id: u64, allowed: bool)]
pub struct AddOrUpdateMerchantDebitor<'info> {
    /// The merchant manager account, must match manager in manager_state
    /// Required permissions: Signer
    #[account(constraint = manager.key() == manager_state.manager)]
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

    /// PDA storing the debitor's authorization state
    /// Seeds: [MERCHANT_DEBITOR_SEED, merchant_id, debitor]
    /// Space: Discriminator + Boolean
    /// Required permissions: Mutable if new, Read-only if existing
    #[account(
        init_if_needed,
        payer = payer,
        space = MerchantDebitorState::DISCRIMINATOR.len() + MerchantDebitorState::INIT_SPACE,
        seeds = [
            MERCHANT_DEBITOR_SEED,
            &merchant_id.to_le_bytes(),
            &mint.key().as_ref(),
            &debitor.key().as_ref(),
        ],
        bump
    )]
    pub debitor_state: Account<'info, MerchantDebitorState>,

    /// Account to be authorized as a debitor
    /// Not required to sign to allow manager to revoke access
    /// Required permissions: None (read-only validation)
    /// CHECK: No need to be a signer else debitors can't be revoked by admin if they withhold signature
    pub debitor: AccountInfo<'info>,

    /// The token mint account that this debitor is authorized for
    /// Required permissions: None (read-only validation)
    pub mint: InterfaceAccount<'info, Mint>,

    /// Required for account creation
    pub system_program: Program<'info, System>,
}

/**
 * Process the addition or update of a merchant debitor.
 *
 * @param ctx Context containing all required accounts
 * @param merchant_id Unique identifier for the merchant
 * @param allowed Whether the debitor should be allowed to initiate transfers
 *
 * Flow:
 * 1. Verify manager signature (done via account constraints)
 * 2. Update debitor state PDA with new allowed status
 * 3. Emit event with merchant_id, debitor, and state change
 *
 * @return Result indicating success or containing an error
 */
pub fn handler(
    ctx: Context<AddOrUpdateMerchantDebitor>,
    merchant_id: u64,
    allowed: bool,
) -> Result<()> {
    let debitor_state = &mut ctx.accounts.debitor_state;
    let previous_state = debitor_state.allowed;
    debitor_state.allowed = allowed;
    debitor_state.bump = ctx.bumps.debitor_state;

    // Emit event for indexing and notifications
    emit!(MerchantDebitorAddedOrUpdated {
        merchant_id,
        debitor: ctx.accounts.debitor.key(),
        state_pda: ctx.accounts.debitor_state.key(),
        previous_state,
        new_state: allowed,
    });

    Ok(())
}
