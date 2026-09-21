//! Sets the debitor address in SpenderState. Called by manager.

use crate::{errors::ErrorCode, events::DebitorAdded, state::SpenderState};
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateDebitor<'info> {
    #[account(constraint = manager.key() == spender_state.manager)]
    pub manager: Signer<'info>,

    #[account(
        mut,
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    /// CHECK: Address is validated against zero address; stored as pubkey in state.
    pub debitor: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<UpdateDebitor>) -> Result<()> {
    require!(
        ctx.accounts.debitor.key() != Pubkey::default(),
        ErrorCode::ZeroAddress
    );
    ctx.accounts.spender_state.debitor = ctx.accounts.debitor.key();
    emit!(DebitorAdded { debitor: ctx.accounts.debitor.key() });
    Ok(())
}
