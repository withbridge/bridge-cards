//! Sets or clears the program-level migrated flag.
//! When migrated is true, all instructions except cpi_transfer are rejected.
//! This instruction is intentionally exempt from the migrated check so the admin
//! can toggle the flag in either direction.

use crate::events::MigrationStateUpdated;
use crate::instructions::initialize::STATE_SEED;
use crate::state::BridgeCardsState;
use crate::ID;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct SetMigrated<'info> {
    #[account(constraint = admin.key() == state.admin)]
    pub admin: Signer<'info>,

    /// Pays any lamport increase from reallocating the state account.
    /// On the first call the state account grows by 1 byte (adding the migrated field);
    /// subsequent calls are a no-op with respect to size.
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        realloc = BridgeCardsState::DISCRIMINATOR.len() + BridgeCardsState::INIT_SPACE,
        realloc::payer = payer,
        realloc::zero = false,
        seeds = [STATE_SEED],
        bump = state.bump,
        seeds::program = ID,
    )]
    pub state: Account<'info, BridgeCardsState>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<SetMigrated>, migrated: bool) -> Result<()> {
    ctx.accounts.state.migrated = migrated;

    emit!(MigrationStateUpdated { migrated });

    Ok(())
}
