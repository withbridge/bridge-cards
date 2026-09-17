//! Sets the manager address in SpenderState. Called by governor.

use crate::{errors::ErrorCode, events::ManagerAdded, state::SpenderState};
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateManager<'info> {
    #[account(constraint = governor.key() == spender_state.governor)]
    pub governor: Signer<'info>,

    #[account(
        mut,
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    /// CHECK: Address is validated against zero address; stored as pubkey in state.
    pub manager: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<UpdateManager>) -> Result<()> {
    require!(
        ctx.accounts.manager.key() != Pubkey::default(),
        ErrorCode::ZeroAddress
    );
    ctx.accounts.spender_state.manager = ctx.accounts.manager.key();
    emit!(ManagerAdded { manager: ctx.accounts.manager.key() });
    Ok(())
}
