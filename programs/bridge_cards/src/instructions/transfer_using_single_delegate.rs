//! Executes a token transfer using the SPL token delegate mechanism (no subscriptions CPI).
//! The user must have previously called SPL `approve` on their delegator_ata, granting the
//! merchant_delegate PDA authority to spend tokens up to the approved amount.
//!
//! Both SPL Token and Token-2022 mints are supported.

use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use crate::instructions::setup_merchant_delegate::{DELEGATE_DESTINATION_SEED, MERCHANT_DELEGATE_SEED};
use crate::{errors::ErrorCode, events::SingleDelegateTransfer, state::*};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

#[derive(Accounts)]
#[instruction(merchant_id: [u8; 32])]
pub struct TransferUsingSingleDelegate<'info> {
    #[account(constraint = debitor.key() == spender_state.debitor)]
    pub debitor: Signer<'info>,

    #[account(
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    /// Acts as the SPL delegate authority; must be pre-approved on delegator_ata.
    #[account(
        seeds = [MERCHANT_DELEGATE_SEED, merchant_id.as_ref()],
        bump = merchant_delegate_state.bump,
    )]
    pub merchant_delegate_state: Account<'info, MerchantDelegateState>,

    #[account(mut)]
    pub delegator_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub receiver_ata: InterfaceAccount<'info, TokenAccount>,

    /// Proves `receiver_ata` is allowlisted for this merchant.
    #[account(
        seeds = [DELEGATE_DESTINATION_SEED, merchant_id.as_ref(), receiver_ata.key().as_ref()],
        bump = delegate_destination_state.bump,
    )]
    pub delegate_destination_state: Account<'info, DelegateDestinationState>,

    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(constraint = token_program.key() == *token_mint.to_account_info().owner @ ErrorCode::InvalidPda)]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler<'info>(
    ctx: Context<'info, TransferUsingSingleDelegate<'info>>,
    merchant_id: [u8; 32],
    amount: u64,
) -> Result<()> {
    require!(!ctx.accounts.spender_state.paused, ErrorCode::ProgramPaused);
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        ctx.accounts.delegator_ata.key() != ctx.accounts.receiver_ata.key(),
        ErrorCode::SelfTransfer
    );
    require!(
        ctx.accounts.delegator_ata.mint == ctx.accounts.token_mint.key(),
        ErrorCode::InvalidPda
    );
    require!(
        ctx.accounts.receiver_ata.mint == ctx.accounts.token_mint.key(),
        ErrorCode::InvalidPda
    );

    let bump_bytes = [ctx.accounts.merchant_delegate_state.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[MERCHANT_DELEGATE_SEED, merchant_id.as_ref(), &bump_bytes]];

    let pre_receiver = ctx.accounts.receiver_ata.amount;

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info().key(),
            TransferChecked {
                from: ctx.accounts.delegator_ata.to_account_info(),
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.receiver_ata.to_account_info(),
                authority: ctx.accounts.merchant_delegate_state.to_account_info(),
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

    emit!(SingleDelegateTransfer {
        merchant_id,
        user: ctx.accounts.delegator_ata.owner,
        receiver: ctx.accounts.receiver_ata.key(),
        debitor: ctx.accounts.debitor.key(),
        amount: received_amount,
    });

    Ok(())
}
