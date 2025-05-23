use crate::events::AdminUpdated;
use crate::instructions::initialize::STATE_SEED;
use crate::state::BridgeCardsState;
use crate::ID;
use anchor_lang::prelude::*;

/**
 * Update the admin of the BridgeCards program.
 *
 * This instruction allows the current admin to transfer administrative control to a new account.
 * The admin has the highest level of authority in the program and can manage all merchant-related
 * configurations.
 *
 * Admin Privileges:
 * - Add/update merchant destination token accounts
 * - Add/update merchant managers
 * - Close program accounts and recover rent
 * - Transfer admin authority
 *
 * Account Updates:
 * - Updates the admin pubkey in the global state PDA
 * - Requires both current and new admin signatures
 * - State PDA is derived using [STATE_SEED]
 *
 * Security Model:
 * - Two-party authorization (current and new admin must sign)
 * - Atomic transfer of authority
 * - No admin downtime during transfer
 * - Prevents accidental transfers to invalid accounts
 *
 * Events Emitted:
 * - AdminUpdated: When admin authority is transferred
 *   Fields: admin (new admin's pubkey)
 *
 * Common Use Cases:
 * - Rotating admin keys for security
 * - Transferring program control
 * - Updating admin after key compromise
 *
 * Required Accounts:
 * - admin: Current program admin
 * - payer: Account paying for transaction fees
 * - state: Global program state PDA
 * - new_admin: Account to receive admin authority
 */
#[derive(Accounts)]
pub struct UpdateAdmin<'info> {
    /// Current admin account, must match admin stored in state
    /// Required permissions: Signer
    #[account(constraint = admin.key() == state.admin)]
    pub admin: Signer<'info>,

    /// Account that will pay for transaction fees
    /// Required permissions: Signer, Mutable (for fee payment)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Global program state storing the admin public key
    /// Seeds: [STATE_SEED]
    /// Required permissions: Mutable (for admin update)
    #[account(mut,
        seeds = [STATE_SEED],
        bump,
        seeds::program = ID
    )]
    pub state: Account<'info, BridgeCardsState>,

    /// Account that will become the new admin
    /// Required permissions: Signer (prevents invalid transfers)
    pub new_admin: Signer<'info>,
}

/**
 * Process the update of the program admin.
 *
 * @param ctx Context containing all required accounts
 *
 * Flow:
 * 1. Verify current admin signature (done via account constraints)
 * 2. Verify new admin signature (done via account constraints)
 * 3. Update state PDA with new admin pubkey
 * 4. Emit event with new admin
 *
 * @return Result indicating success or containing an error
 */
pub fn handler(ctx: Context<UpdateAdmin>) -> Result<()> {
    let state = &mut ctx.accounts.state;
    state.admin = ctx.accounts.new_admin.key();
    state.bump = ctx.bumps.state;

    // Emit event for indexing and notifications
    emit!(AdminUpdated {
        admin: ctx.accounts.new_admin.key(),
    });

    Ok(())
}
