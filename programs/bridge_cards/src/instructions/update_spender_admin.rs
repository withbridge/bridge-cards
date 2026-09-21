//! Transfers the admin role within SpenderState to a new keypair.
//! Both the current admin and the incoming admin must sign to prevent transfer to an uncontrolled key.

use crate::{events::SpenderAdminUpdated, state::SpenderState};
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateSpenderAdmin<'info> {
    #[account(constraint = admin.key() == spender_state.admin)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    /// Must sign to confirm the new admin controls this keypair.
    pub new_admin: Signer<'info>,
}

pub fn handler(ctx: Context<UpdateSpenderAdmin>) -> Result<()> {
    ctx.accounts.spender_state.admin = ctx.accounts.new_admin.key();

    emit!(SpenderAdminUpdated {
        admin: ctx.accounts.new_admin.key(),
    });

    Ok(())
}
