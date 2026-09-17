//! Removes a token account from a merchant's spender-style destination allowlist. Called by governor.
//! Once closed, the destination will be rejected by delegate transfer instructions.

use crate::{events::DelegateDestinationClosed, state::{DelegateDestinationState, SpenderState}};
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use crate::instructions::setup_merchant_delegate::DELEGATE_DESTINATION_SEED;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(merchant_id: [u8; 32])]
pub struct CloseDelegateDestination<'info> {
    #[account(constraint = governor.key() == spender_state.governor)]
    pub governor: Signer<'info>,

    #[account(
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Address is used only as a seed for the destination state PDA.
    pub destination: UncheckedAccount<'info>,

    #[account(
        mut,
        close = payer,
        seeds = [
            DELEGATE_DESTINATION_SEED,
            merchant_id.as_ref(),
            destination.key().as_ref(),
        ],
        bump = delegate_destination_state.bump,
    )]
    pub delegate_destination_state: Account<'info, DelegateDestinationState>,
}

pub fn handler(ctx: Context<CloseDelegateDestination>, merchant_id: [u8; 32]) -> Result<()> {
    emit!(DelegateDestinationClosed {
        merchant_id,
        destination: ctx.accounts.destination.key(),
    });
    Ok(())
}
