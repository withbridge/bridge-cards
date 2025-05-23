use crate::events::MerchantDestinationAddedOrUpdated;
use crate::instructions::initialize::STATE_SEED;
use crate::state::{BridgeCardsState, MerchantDestinationState};
use crate::ID;
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;
use anchor_spl::token_interface::Mint;

/// Seed used to derive merchant destination PDAs
pub const MERCHANT_DESTINATION_SEED: &[u8] = b"merchant_destination";

/**
 * Add or update an allowed destination token account for a merchant.
 *
 * This instruction allows the program admin to allowlist token accounts that can receive
 * funds on behalf of a merchant. Only allowlisted destination accounts can receive transfers
 * from user delegates associated with the merchant.
 *
 * Destination Configuration:
 * - Each destination is specific to a merchant-mint combination
 * - Destinations can be enabled or disabled via the destination_allowed parameter
 * - Multiple destinations can be configured per merchant and mint
 *
 * Account Creation:
 * - Creates a PDA to store the destination's state if it doesn't exist
 * - PDA is derived using [MERCHANT_DESTINATION_SEED, merchant_id, mint, destination_token_account]
 * - Funded by the payer account
 *
 * Security Model:
 * - Only the program admin can add/update destinations
 * - Each destination is validated to use the specified mint
 * - Destination account does not need to sign (allows admin to revoke access)
 * - State is stored in a PDA unique to the merchant-mint-destination combination
 *
 * Events Emitted:
 * - MerchantDestinationAddedOrUpdated: When a destination is set or changed
 *   Fields: merchant_id, mint, destination, state_pda, previous_state, new_state
 *
 * Required Accounts:
 * - admin: Program admin who can update destinations
 * - payer: Account paying for PDA creation/rent
 * - state: Global program state storing admin pubkey
 * - destination_state: PDA storing destination authorization
 * - destination_token_account: Token account to be allowlisted
 * - mint: Token mint for the destination account
 * - system_program: Required for account creation
 */
#[derive(Accounts)]
#[instruction(merchant_id: u64)]
pub struct AddOrUpdateMerchantDestination<'info> {
    /// The program admin account, must match admin stored in state
    /// Required permissions: Signer
    #[account(constraint = admin.key() == state.admin)]
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

    /// PDA storing the destination's authorization state
    /// Seeds: [MERCHANT_DESTINATION_SEED, merchant_id, mint, destination_token_account]
    /// Space: Discriminator + Boolean
    /// Required permissions: Mutable if new, Read-only if existing
    #[account(
        init_if_needed,
        payer = payer,
        space = MerchantDestinationState::DISCRIMINATOR.len() + MerchantDestinationState::INIT_SPACE,
        seeds = [
            MERCHANT_DESTINATION_SEED,
            &merchant_id.to_le_bytes(),
            mint.key().as_ref(),
            destination_token_account.key().as_ref(),
        ],
        bump,
    )]
    pub destination_state: Account<'info, MerchantDestinationState>,

    /// Token account to be allowlisted as a destination
    /// Must use the specified mint
    /// Required permissions: None (read-only validation)
    #[account(constraint = destination_token_account.mint.key() == mint.key())]
    pub destination_token_account: Account<'info, TokenAccount>,

    /// Mint of the destination token account
    /// Used for PDA derivation and account validation
    /// Required permissions: None (read-only validation)
    pub mint: InterfaceAccount<'info, Mint>,

    /// Required for account creation
    pub system_program: Program<'info, System>,
}

/**
 * Process the addition or update of a merchant destination.
 *
 * @param ctx Context containing all required accounts
 * @param merchant_id Unique identifier for the merchant
 * @param destination_allowed Whether the destination should be allowed to receive funds
 *
 * Flow:
 * 1. Verify admin signature (done via account constraints)
 * 2. Update destination state PDA with new allowed status
 * 3. Emit event with merchant_id, mint, destination, and state change
 *
 * @return Result indicating success or containing an error
 */
pub fn handler(
    ctx: Context<AddOrUpdateMerchantDestination>,
    merchant_id: u64,
    destination_allowed: bool,
) -> Result<()> {
    let destination_state = &mut ctx.accounts.destination_state;
    let previous_state = destination_state.allowed;
    destination_state.allowed = destination_allowed;
    destination_state.bump = ctx.bumps.destination_state;

    // Emit event for indexing and notifications
    emit!(MerchantDestinationAddedOrUpdated {
        merchant_id,
        mint: ctx.accounts.mint.key(),
        destination: ctx.accounts.destination_token_account.key(),
        state_pda: ctx.accounts.destination_state.key(),
        previous_state,
        new_state: destination_allowed,
    });

    Ok(())
}
