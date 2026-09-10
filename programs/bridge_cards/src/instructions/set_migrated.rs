//! Creates or closes the MigrationState PDA to signal the program's migration status.
//!
//! When the MigrationState PDA exists (owned by this program), the contract is
//! migrated: all instructions except cpi_transfer check for it and reject.
//! When it doesn't exist, the contract operates normally.
//!
//! Using a separate PDA instead of a field in BridgeCardsState means the
//! BridgeCardsState account never changes size, so deploying the migration binary
//! works immediately against all existing mainnet accounts with zero downtime.
//!
//! # Deployment strategy
//!
//! Because adding `migration_state` to instruction account lists is a breaking change
//! for existing callers, the deployment order should be:
//!   1. Deploy updated Monorail (passes migration_state PDA to affected instructions).
//!      The old binary ignores the extra account — no failures.
//!   2. Deploy migration binary. New binary validates migration_state. Zero downtime.
//!   3. Call set_migrated(true) when ready to cut over.

use crate::errors::ErrorCode;
use crate::events::MigrationStateUpdated;
use crate::instructions::initialize::STATE_SEED;
use crate::state::{BridgeCardsState, MigrationState};
use crate::ID;
use anchor_lang::prelude::*;

/// Seed for the migration-state PDA.
pub const MIGRATION_STATE_SEED: &[u8] = b"migration";

#[derive(Accounts)]
pub struct SetMigrated<'info> {
    #[account(constraint = admin.key() == state.admin @ ErrorCode::InvalidPda)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [STATE_SEED],
        bump = state.bump,
        seeds::program = ID,
    )]
    pub state: Account<'info, BridgeCardsState>,

    /// CHECK: Created (migrated=true) or closed (migrated=false) in the handler.
    /// Seeds constraint ensures this is the canonical migration PDA.
    #[account(
        mut,
        seeds = [MIGRATION_STATE_SEED],
        bump,
        seeds::program = ID,
    )]
    pub migration_state: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<SetMigrated>, migrated: bool) -> Result<()> {
    if migrated {
        if ctx.accounts.migration_state.lamports() == 0 {
            // Create the MigrationState PDA.
            let bump = ctx.bumps.migration_state;
            let space = MigrationState::DISCRIMINATOR.len() + MigrationState::INIT_SPACE;
            let lamports = Rent::get()?.minimum_balance(space);
            let signer_seeds: &[&[&[u8]]] = &[&[MIGRATION_STATE_SEED, &[bump]]];

            anchor_lang::system_program::create_account(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::CreateAccount {
                        from: ctx.accounts.payer.to_account_info(),
                        to: ctx.accounts.migration_state.to_account_info(),
                    },
                    signer_seeds,
                ),
                lamports,
                space as u64,
                &ID,
            )?;

            let mut data = ctx.accounts.migration_state.try_borrow_mut_data()?;
            data[..MigrationState::DISCRIMINATOR.len()].copy_from_slice(&MigrationState::DISCRIMINATOR);
            data[MigrationState::DISCRIMINATOR.len()] = bump;
        }
    } else {
        let lamports = ctx.accounts.migration_state.lamports();
        if lamports > 0 {
            // Close the MigrationState PDA — return lamports to payer, then reassign
            // to system program and realloc to 0 so the address can be re-created later.
            {
                let mut payer_lamports = ctx.accounts.payer.try_borrow_mut_lamports()?;
                let mut ms_lamports = ctx.accounts.migration_state.try_borrow_mut_lamports()?;
                **payer_lamports += lamports;
                **ms_lamports = 0;
            }
            ctx.accounts
                .migration_state
                .assign(&anchor_lang::solana_program::system_program::id());
            ctx.accounts.migration_state.realloc(0, false)?;
        }
    }

    emit!(MigrationStateUpdated { migrated });

    Ok(())
}
