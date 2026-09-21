//! Creates the global SpenderState PDA. Can only be called once (Anchor rejects re-init).
//! Holds the governor/manager/debitor/pauser roles for the spender-style delegation system.
//! Uses seed "spender_state" to avoid collision with BridgeCardsState at "state".
//!
//! Requires the existing BridgeCardsState admin to sign, ensuring only the party that
//! already controls the program can bootstrap the new role hierarchy.

use crate::{
    errors::ErrorCode,
    events::SpenderStateInitialized,
    state::{BridgeCardsState, SpenderState},
};
use crate::instructions::legacy::initialize::STATE_SEED;
use anchor_lang::prelude::*;

pub const SPENDER_STATE_SEED: &[u8] = b"spender_state";

#[derive(Accounts)]
pub struct InitializeSpenderState<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Must be the current BridgeCardsState admin — proves continuity of control.
    #[account(constraint = admin.key() == bridge_cards_state.admin @ ErrorCode::Unauthorized)]
    pub admin: Signer<'info>,

    /// Existing legacy state, used only to verify the admin's identity.
    #[account(
        seeds = [STATE_SEED],
        bump = bridge_cards_state.bump,
    )]
    pub bridge_cards_state: Account<'info, BridgeCardsState>,

    #[account(
        init,
        payer = payer,
        space = SpenderState::DISCRIMINATOR.len() + SpenderState::INIT_SPACE,
        seeds = [SPENDER_STATE_SEED],
        bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeSpenderState>,
    admin: Pubkey,
    governor: Pubkey,
    manager: Pubkey,
    debitor: Pubkey,
    pauser: Pubkey,
) -> Result<()> {
    require!(admin != Pubkey::default(), ErrorCode::ZeroAddress);
    require!(governor != Pubkey::default(), ErrorCode::ZeroAddress);
    require!(manager != Pubkey::default(), ErrorCode::ZeroAddress);
    require!(debitor != Pubkey::default(), ErrorCode::ZeroAddress);
    require!(pauser != Pubkey::default(), ErrorCode::ZeroAddress);

    let state = &mut ctx.accounts.spender_state;
    state.admin = admin;
    state.governor = governor;
    state.manager = manager;
    state.debitor = debitor;
    state.pauser = pauser;
    state.bump = ctx.bumps.spender_state;
    state.paused = false;

    emit!(SpenderStateInitialized {
        admin,
        governor,
        manager,
        debitor,
        pauser,
    });

    Ok(())
}
