use crate::events::MerchantManagerAddedOrUpdated;
use crate::instructions::initialize::STATE_SEED;
use crate::state::{BridgeCardsState, MerchantManagerState};
use crate::ID;
use anchor_lang::prelude::*;

/// Seed used to derive merchant manager PDAs
pub const MERCHANT_MANAGER_SEED: &[u8] = b"merchant_manager";

/**
 * Add or update a merchant manager account.
 *
 * This instruction allows the program admin to designate an account as a manager for a specific
 * merchant. Merchant managers can:
 * - Add/update user delegates for their merchant
 * - Configure delegate transfer limits
 * - Add/update debitor accounts for their merchant
 *
 * Permission Hierarchy:
 * Admin -> Merchant Manager -> User Delegates/Debitors
 *
 * Account Creation:
 * - Creates a PDA to store the manager's state if it doesn't exist
 * - PDA is derived using [MERCHANT_MANAGER_SEED, merchant_id]
 * - Funded by the payer account
 *
 * Security Model:
 * - Only the program admin can add/update managers
 * - Manager account does not need to sign (allows admin to revoke access)
 * - Each merchant can have one active manager at a time
 * - Manager state is stored in a PDA unique to the merchant
 *
 * Events Emitted:
 * - MerchantManagerAddedOrUpdated: When a manager is set or changed
 *   Fields: merchant_id, manager pubkey
 *
 * Common Use Cases:
 * - Initial manager setup for a new merchant
 * - Rotating merchant managers
 * - Revoking manager access
 *
 * Required Accounts:
 * - admin: Program admin who can update managers
 * - payer: Account paying for PDA creation/rent
 * - state: Global program state storing admin pubkey
 * - manager_state: PDA storing manager authorization
 * - manager: Account to be set as manager (not a signer)
 * - system_program: Required for account creation
 */
#[derive(Accounts)]
#[instruction(merchant_id: u64)]
pub struct AddOrUpdateMerchantManager<'info> {
    /// The program admin account, must match admin stored in state
    /// Required permissions: Signer
    #[account( constraint = admin.key() == state.admin)]
    pub admin: Signer<'info>,

    /// Account that will pay for PDA creation and rent
    /// Required permissions: Signer, Mutable (for rent payment)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Global program state storing the admin public key
    /// Seeds: [STATE_SEED]
    /// Required permissions: Read-only
    #[account(
        seeds = [STATE_SEED],
        bump = state.bump,
        seeds::program = ID
    )]
    pub state: Account<'info, BridgeCardsState>,

    /// PDA storing the merchant manager's authorization state
    /// Seeds: [MERCHANT_MANAGER_SEED, merchant_id]
    /// Space: Discriminator + Pubkey
    /// Required permissions: Mutable if new, Read-only if existing
    #[account(
        init_if_needed,
        payer = payer,
        space = MerchantManagerState::DISCRIMINATOR.len() + MerchantManagerState::INIT_SPACE,
        seeds = [
            MERCHANT_MANAGER_SEED,
            &merchant_id.to_le_bytes(),
        ],
        bump
    )]
    pub manager_state: Account<'info, MerchantManagerState>,

    /// Account to be set as the merchant manager
    /// Not a signer to allow admin to revoke access without manager cooperation
    /// Required permissions: None
    /// CHECK: Account is only stored as a pubkey, no account data validation needed
    pub manager: AccountInfo<'info>,

    /// Required for account creation
    pub system_program: Program<'info, System>,
}

/**
 * Process the addition or update of a merchant manager.
 *
 * @param ctx Context containing all required accounts
 * @param merchant_id Unique identifier for the merchant
 *
 * Flow:
 * 1. Verify admin signature (done via account constraints)
 * 2. Update manager state PDA with new manager pubkey
 * 3. Emit event with merchant_id and new manager
 *
 * @return Result indicating success or containing an error
 */
pub fn handler(ctx: Context<AddOrUpdateMerchantManager>, merchant_id: u64) -> Result<()> {
    let manager_state = &mut ctx.accounts.manager_state;
    manager_state.manager = ctx.accounts.manager.key();
    manager_state.bump = ctx.bumps.manager_state;

    // Emit event for indexing and notifications
    emit!(MerchantManagerAddedOrUpdated {
        merchant_id,
        manager: ctx.accounts.manager.key(),
    });

    Ok(())
}
