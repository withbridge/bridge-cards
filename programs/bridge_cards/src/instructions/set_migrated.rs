//! Sets or clears the program-level migrated flag.
//! When migrated is true, all instructions except cpi_transfer are rejected.
//! This instruction is intentionally exempt from the migrated check so the admin
//! can toggle the flag in either direction.
//!
//! # Upgrade safety
//!
//! The state account uses `UncheckedAccount` (rather than `Account<BridgeCardsState>`)
//! so that the realloc can happen *before* Borsh deserialization. Anchor deserializes
//! `Account<T>` before applying constraints, so an existing mainnet account sized for the
//! old layout (without `migrated: bool`) would fail deserialization before the realloc
//! could expand it. We validate the seeds, owner, discriminator, and admin manually.

use crate::errors::ErrorCode;
use crate::events::MigrationStateUpdated;
use crate::instructions::initialize::STATE_SEED;
use crate::state::BridgeCardsState;
use crate::ID;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct SetMigrated<'info> {
    pub admin: Signer<'info>,

    /// Pays any lamport increase from reallocating the state account.
    /// On the first call the state account grows by 1 byte (adding the migrated field);
    /// subsequent calls are a no-op with respect to size.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Validated manually in the handler — seeds, owner, discriminator, and admin
    /// are all checked after the realloc so the upgrade path from the old layout works.
    #[account(
        mut,
        seeds = [STATE_SEED],
        bump,
        seeds::program = ID,
    )]
    pub state: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<SetMigrated>, migrated: bool) -> Result<()> {
    let state_info = ctx.accounts.state.to_account_info();

    // Validate program ownership.
    require_keys_eq!(*state_info.owner, ID, ErrorCode::InvalidPda);

    let target_len = BridgeCardsState::DISCRIMINATOR.len() + BridgeCardsState::INIT_SPACE;

    // Grow the account if it predates the `migrated` field (old layout is 1 byte smaller).
    if state_info.data_len() < target_len {
        let rent = Rent::get()?;
        let required_lamports = rent.minimum_balance(target_len);
        let current_lamports = state_info.lamports();
        if current_lamports < required_lamports {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.payer.to_account_info(),
                        to: state_info.clone(),
                    },
                ),
                required_lamports - current_lamports,
            )?;
        }
        state_info.realloc(target_len, false)?;
    }

    // Deserialize, validate, mutate, re-serialize.
    let mut data = state_info.try_borrow_mut_data()?;

    // Validate discriminator.
    require!(
        data[..8] == *BridgeCardsState::DISCRIMINATOR,
        ErrorCode::InvalidPda
    );

    let mut state = BridgeCardsState::try_deserialize(&mut data.as_ref())?;

    // Validate stored bump matches the PDA we derived.
    require_eq!(state.bump, ctx.bumps.state, ErrorCode::InvalidPda);

    // Validate admin.
    require_keys_eq!(ctx.accounts.admin.key(), state.admin, ErrorCode::InvalidPda);

    state.migrated = migrated;

    let mut slice: &mut [u8] = &mut data;
    state.try_serialize(&mut slice)?;

    emit!(MigrationStateUpdated { migrated });

    Ok(())
}
