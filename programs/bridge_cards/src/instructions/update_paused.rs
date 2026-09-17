//! Pauses or unpauses all delegate-based token transfers program-wide. Called by admin or pauser.

use crate::{events::ProgramPauseUpdated, state::SpenderState};
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdatePaused<'info> {
    #[account(constraint = signer.key() == spender_state.admin || signer.key() == spender_state.pauser)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,
}

pub fn handler(ctx: Context<UpdatePaused>, paused: bool) -> Result<()> {
    ctx.accounts.spender_state.paused = paused;
    emit!(ProgramPauseUpdated { paused });
    Ok(())
}
