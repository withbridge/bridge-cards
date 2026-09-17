//! Sets the pauser address in SpenderState. Called by admin.

use crate::{errors::ErrorCode, events::PauserAdded, state::SpenderState};
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdatePauser<'info> {
    #[account(constraint = admin.key() == spender_state.admin)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    /// CHECK: Address is validated against zero address; stored as pubkey in state.
    pub pauser: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<UpdatePauser>) -> Result<()> {
    require!(
        ctx.accounts.pauser.key() != Pubkey::default(),
        ErrorCode::ZeroAddress
    );
    ctx.accounts.spender_state.pauser = ctx.accounts.pauser.key();
    emit!(PauserAdded { pauser: ctx.accounts.pauser.key() });
    Ok(())
}
