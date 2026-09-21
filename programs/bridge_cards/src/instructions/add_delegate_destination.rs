//! Allowlists a single token account as a valid transfer destination for a merchant.
//! Called by governor. Uses spender-style seeds (no mint in PDA, [u8;32] merchant_id).
//! Uses `init` (not init_if_needed) so that re-calling for an already-allowlisted
//! destination fails explicitly rather than silently emitting a duplicate event.

use crate::{
    errors::ErrorCode,
    events::DelegateDestinationAdded,
    state::{DelegateDestinationState, MerchantDelegateState, SpenderState},
};
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use crate::instructions::setup_merchant_delegate::{DELEGATE_DESTINATION_SEED, MERCHANT_DELEGATE_SEED};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};

#[derive(Accounts)]
#[instruction(merchant_id: [u8; 32])]
pub struct AddDelegateDestination<'info> {
    #[account(constraint = governor.key() == spender_state.governor)]
    pub governor: Signer<'info>,

    #[account(
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub destination: InterfaceAccount<'info, TokenAccount>,

    /// Must exist — prevents creating orphan destination PDAs for merchants with no active delegate.
    #[account(
        seeds = [MERCHANT_DELEGATE_SEED, merchant_id.as_ref()],
        bump = merchant_delegate_state.bump,
    )]
    pub merchant_delegate_state: Account<'info, MerchantDelegateState>,

    #[account(
        init,
        payer = payer,
        space = DelegateDestinationState::DISCRIMINATOR.len() + DelegateDestinationState::INIT_SPACE,
        seeds = [
            DELEGATE_DESTINATION_SEED,
            merchant_id.as_ref(),
            destination.key().as_ref(),
        ],
        bump
    )]
    pub delegate_destination_state: Account<'info, DelegateDestinationState>,

    #[account(constraint = token_program.key() == *destination.to_account_info().owner @ ErrorCode::InvalidPda)]
    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<AddDelegateDestination>, merchant_id: [u8; 32]) -> Result<()> {
    ctx.accounts.delegate_destination_state.bump =
        ctx.bumps.delegate_destination_state;

    emit!(DelegateDestinationAdded {
        merchant_id,
        destination: ctx.accounts.destination.key(),
    });

    Ok(())
}
