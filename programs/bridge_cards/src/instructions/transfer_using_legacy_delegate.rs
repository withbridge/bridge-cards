//! Transfer tokens via the legacy user-delegate PDA system, validated against the new
//! spender-style access control and destination allowlist.
//!
//! This is the intended successor to debit_user for users who onboarded via the legacy
//! PDA approval path. The user has approved the user_delegate PDA (derived from the
//! legacy merchant_id u64) via SPL approve, and that PDA is used as the signing authority.
//!
//! Unlike debit_user:
//! - Uses the global spender debitor (not a per-merchant debitor).
//! - Validates destination against the new DelegateDestinationState allowlist (keyed by program_id).
//! - init_if_needed on the user_delegate_account — no prior add_or_update_user_delegate call needed.
//! - IS pausable via SpenderState.
//! - No velocity controls.

use crate::instructions::legacy::add_or_update_user_delegate::USER_DELEGATE_SEED;
use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use crate::instructions::setup_merchant_delegate::{
    DELEGATE_DESTINATION_SEED, MERCHANT_DELEGATE_SEED,
};
use crate::{errors::ErrorCode, events::LegacyDelegateTransfer, state::*};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

#[derive(Accounts)]
#[instruction(program_id: [u8; 32], merchant_id: u64)]
pub struct TransferUsingLegacyDelegate<'info> {
    /// Must match SpenderState.debitor.
    #[account(constraint = debitor.key() == spender_state.debitor @ ErrorCode::Unauthorized)]
    pub debitor: Signer<'info>,

    #[account(
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    /// Pays rent for init_if_needed on user_delegate_account.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Legacy user delegate PDA — signing authority for the SPL transfer.
    /// The user must have previously called SPL approve with this PDA's address.
    /// init_if_needed: new users do not need to call add_or_update_user_delegate first.
    /// Velocity fields are initialized to zero and ignored.
    #[account(
        init_if_needed,
        payer = payer,
        space = UserDelegateState::DISCRIMINATOR.len() + UserDelegateState::INIT_SPACE,
        seeds = [
            USER_DELEGATE_SEED,
            merchant_id.to_le_bytes().as_ref(),
            token_mint.key().as_ref(),
            user_ata.key().as_ref(),
        ],
        bump,
    )]
    pub user_delegate_account: Account<'info, UserDelegateState>,

    /// Proves the program_id has been registered as a merchant.
    #[account(
        seeds = [MERCHANT_DELEGATE_SEED, program_id.as_ref()],
        bump = merchant_delegate_state.bump,
    )]
    pub merchant_delegate_state: Account<'info, MerchantDelegateState>,

    #[account(mut)]
    pub user_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub receiver_ata: InterfaceAccount<'info, TokenAccount>,

    /// Proves receiver_ata is allowlisted for this merchant (program_id).
    #[account(
        seeds = [DELEGATE_DESTINATION_SEED, program_id.as_ref(), receiver_ata.key().as_ref()],
        bump = delegate_destination_state.bump,
    )]
    pub delegate_destination_state: Account<'info, DelegateDestinationState>,

    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(constraint = token_program.key() == *token_mint.to_account_info().owner @ ErrorCode::InvalidPda)]
    pub token_program: Interface<'info, TokenInterface>,

    pub system_program: Program<'info, System>,
}

pub fn handler<'info>(
    ctx: Context<'info, TransferUsingLegacyDelegate<'info>>,
    program_id: [u8; 32],
    merchant_id: u64,
    amount: u64,
) -> Result<()> {
    require!(!ctx.accounts.spender_state.paused, ErrorCode::ProgramPaused);
    require!(amount > 0, ErrorCode::InvalidAmount);

    // Validate cross-identifier binding: the [u8;32] program_id must map to this exact
    // u64 merchant_id. Prevents a compromised debitor from using a user's approval for
    // Merchant A to transfer funds to destinations allowlisted under Merchant B.
    require!(
        ctx.accounts.merchant_delegate_state.legacy_merchant_id != 0
            && ctx.accounts.merchant_delegate_state.legacy_merchant_id == merchant_id,
        ErrorCode::LegacyMerchantIdMismatch
    );
    require!(
        ctx.accounts.user_ata.mint == ctx.accounts.token_mint.key(),
        ErrorCode::InvalidPda
    );
    require!(
        ctx.accounts.receiver_ata.mint == ctx.accounts.token_mint.key(),
        ErrorCode::InvalidPda
    );

    // Store bump on init (init_if_needed: already set on re-entry, no-op).
    ctx.accounts.user_delegate_account.bump = ctx.bumps.user_delegate_account;

    let merchant_id_bytes = merchant_id.to_le_bytes();
    let bump_bytes = [ctx.accounts.user_delegate_account.bump];
    let mint_key = ctx.accounts.token_mint.key();
    let user_ata_key = ctx.accounts.user_ata.key();
    let signer_seeds: &[&[&[u8]]] = &[&[
        USER_DELEGATE_SEED,
        merchant_id_bytes.as_ref(),
        mint_key.as_ref(),
        user_ata_key.as_ref(),
        &bump_bytes,
    ]];

    let pre_receiver = ctx.accounts.receiver_ata.amount;

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info().key(),
            TransferChecked {
                from: ctx.accounts.user_ata.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.receiver_ata.to_account_info(),
                authority: ctx.accounts.user_delegate_account.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.token_mint.decimals,
    )?;

    ctx.accounts.receiver_ata.reload()?;
    let received_amount = ctx
        .accounts
        .receiver_ata
        .amount
        .checked_sub(pre_receiver)
        .ok_or(ErrorCode::InvalidAmount)?;

    emit!(LegacyDelegateTransfer {
        program_id,
        legacy_merchant_id: merchant_id,
        user: ctx.accounts.user_ata.owner,
        receiver: ctx.accounts.receiver_ata.key(),
        debitor: ctx.accounts.debitor.key(),
        amount: received_amount,
    });

    Ok(())
}
