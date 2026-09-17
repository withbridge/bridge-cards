//! Sets the governor address in SpenderState. Called by admin.

use crate::{errors::ErrorCode, events::GovernorAdded, state::SpenderState};
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateGovernor<'info> {
    #[account(constraint = admin.key() == spender_state.admin)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    /// CHECK: Address is validated against zero address; stored as pubkey in state.
    pub governor: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<UpdateGovernor>) -> Result<()> {
    require!(
        ctx.accounts.governor.key() != Pubkey::default(),
        ErrorCode::ZeroAddress
    );
    ctx.accounts.spender_state.governor = ctx.accounts.governor.key();
    emit!(GovernorAdded { governor: ctx.accounts.governor.key() });
    Ok(())
}
