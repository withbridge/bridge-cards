//! Creates a MerchantDelegateState PDA for a merchant and bulk-initializes its destination allowlist entries.
//! Called by the manager. The merchant_delegate PDA is the signing authority for all subsequent
//! delegate-based transfers under this merchant_id.
//!
//! Remaining accounts must be passed as (destination_token_account, destination_state_pda) pairs,
//! up to 10 pairs per call. Pairs whose destination_state is already initialized are skipped.

use crate::{errors::ErrorCode, events::MerchantDelegateAdded, state::*, ID};
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::ID as TOKEN_PROGRAM_ID;

const TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

pub const DELEGATE_DESTINATION_SEED: &[u8] = b"merchant_destination";
pub const MERCHANT_DELEGATE_SEED: &[u8] = b"merchant_delegate";

#[derive(Accounts)]
#[instruction(merchant_id: [u8; 32])]
pub struct SetupMerchantDelegate<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(constraint = manager.key() == spender_state.manager)]
    pub manager: Signer<'info>,

    #[account(
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    #[account(
        init,
        payer = payer,
        space = MerchantDelegateState::DISCRIMINATOR.len() + MerchantDelegateState::INIT_SPACE,
        seeds = [MERCHANT_DELEGATE_SEED, merchant_id.as_ref()],
        bump
    )]
    pub merchant_delegate_state: Account<'info, MerchantDelegateState>,

    pub system_program: Program<'info, System>,
}

pub fn handler<'info>(
    ctx: Context<'info, SetupMerchantDelegate<'info>>,
    merchant_id: [u8; 32],
    legacy_merchant_id: u64,
) -> Result<()> {
    ctx.accounts.merchant_delegate_state.bump = ctx.bumps.merchant_delegate_state;
    ctx.accounts.merchant_delegate_state.legacy_merchant_id = legacy_merchant_id;

    require!(
        ctx.remaining_accounts.len() % 2 == 0,
        ErrorCode::InvalidRemainingAccounts
    );
    require!(
        ctx.remaining_accounts.len() <= 20,
        ErrorCode::TooManyDestinations
    );

    let rent = Rent::get()?;
    let space = DelegateDestinationState::DISCRIMINATOR.len() + DelegateDestinationState::INIT_SPACE;
    let lamports = rent.minimum_balance(space);

    for pair in ctx.remaining_accounts.chunks(2) {
        let destination = &pair[0];
        let destination_state = &pair[1];

        require!(
            *destination.owner == TOKEN_PROGRAM_ID || *destination.owner == TOKEN_2022_PROGRAM_ID,
            ErrorCode::InvalidDestinationAccount
        );

        let (expected_pda, bump) = Pubkey::find_program_address(
            &[
                DELEGATE_DESTINATION_SEED,
                merchant_id.as_ref(),
                destination.key.as_ref(),
            ],
            ctx.program_id,
        );
        require!(expected_pda == destination_state.key(), ErrorCode::InvalidPda);

        {
            let data = destination_state.try_borrow_data()?;
            if *destination_state.owner == ID
                && data.starts_with(DelegateDestinationState::DISCRIMINATOR)
            {
                continue;
            }
        }

        let bump_bytes = [bump];
        let signer_seeds: &[&[&[u8]]] = &[&[
            DELEGATE_DESTINATION_SEED,
            merchant_id.as_ref(),
            destination.key.as_ref(),
            &bump_bytes,
        ]];

        let current_lamports = destination_state.lamports();
        if current_lamports == 0 {
            system_program::create_account(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info().key(),
                    system_program::CreateAccount {
                        from: ctx.accounts.payer.to_account_info(),
                        to: destination_state.to_account_info(),
                    },
                    signer_seeds,
                ),
                lamports,
                space as u64,
                ctx.program_id,
            )?;
        } else {
            let top_up = lamports.saturating_sub(current_lamports);
            if top_up > 0 {
                system_program::transfer(
                    CpiContext::new(
                        ctx.accounts.system_program.to_account_info().key(),
                        system_program::Transfer {
                            from: ctx.accounts.payer.to_account_info(),
                            to: destination_state.to_account_info(),
                        },
                    ),
                    top_up,
                )?;
            }
            system_program::allocate(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info().key(),
                    system_program::Allocate {
                        account_to_allocate: destination_state.to_account_info(),
                    },
                    signer_seeds,
                ),
                space as u64,
            )?;
            system_program::assign(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info().key(),
                    system_program::Assign {
                        account_to_assign: destination_state.to_account_info(),
                    },
                    signer_seeds,
                ),
                ctx.program_id,
            )?;
        }

        let mut data = destination_state.try_borrow_mut_data()?;
        data[..8].copy_from_slice(DelegateDestinationState::DISCRIMINATOR);
        data[8] = bump;
    }

    let initial_destinations: Vec<Pubkey> = ctx
        .remaining_accounts
        .chunks(2)
        .map(|pair| pair[0].key())
        .collect();

    emit!(MerchantDelegateAdded {
        merchant_id,
        delegate: ctx.accounts.merchant_delegate_state.key(),
        initial_destinations,
    });

    Ok(())
}
