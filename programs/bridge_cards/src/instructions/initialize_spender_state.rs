//! Creates the global SpenderState PDA. Can only be called once (Anchor rejects re-init).
//! Holds the governor/manager/debitor/pauser roles for the spender-style delegation system.
//! Uses seed "spender_state" to avoid collision with BridgeCardsState at "state".

use crate::{
    errors::ErrorCode,
    events::SpenderStateInitialized,
    state::SpenderState,
};
use anchor_lang::prelude::*;

pub const SPENDER_STATE_SEED: &[u8] = b"spender_state";

#[derive(Accounts)]
pub struct InitializeSpenderState<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        space = SpenderState::DISCRIMINATOR.len() + SpenderState::INIT_SPACE,
        seeds = [SPENDER_STATE_SEED],
        bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    /// Production: must be the program's own deploy keypair (pubkey == program ID).
    /// Local/test builds: any signer is accepted.
    #[account(constraint = auth_initialize_spender(program_account.key()))]
    pub program_account: Signer<'info>,

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
        governor,
        manager,
        debitor,
        pauser,
    });

    Ok(())
}

#[cfg(feature = "local")]
fn auth_initialize_spender(_: Pubkey) -> bool {
    true
}

#[cfg(not(feature = "local"))]
fn auth_initialize_spender(account: Pubkey) -> bool {
    use crate::ID;
    account == ID
}
