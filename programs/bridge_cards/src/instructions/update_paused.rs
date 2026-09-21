//! Pauses or unpauses all delegate-based token transfers program-wide.
//! Pausing: admin, governor, or pauser.
//! Unpausing: admin or governor only (pauser cannot unpause).

use crate::{errors::ErrorCode, events::ProgramPauseUpdated, state::SpenderState};
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdatePaused<'info> {
    pub signer: Signer<'info>,

    #[account(
        mut,
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,
}

pub fn handler(ctx: Context<UpdatePaused>, paused: bool) -> Result<()> {
    let state = &ctx.accounts.spender_state;
    let signer = ctx.accounts.signer.key();

    if paused {
        // Pausing: admin, governor, or pauser may pause.
        require!(
            signer == state.admin || signer == state.governor || signer == state.pauser,
            ErrorCode::Unauthorized
        );
    } else {
        // Unpausing: only admin or governor.
        require!(
            signer == state.admin || signer == state.governor,
            ErrorCode::Unauthorized
        );
    }

    ctx.accounts.spender_state.paused = paused;
    emit!(ProgramPauseUpdated { paused });
    Ok(())
}
