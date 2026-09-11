//! Executes a token transfer as a CPI target for the spender program.
//! The spender program is responsible for destination whitelist validation before calling this;
//! this instruction only verifies the CPI origin and executes the SPL transfer.
//!
//! # Velocity limits
//!
//! Velocity limits (per_transfer_limit, period_transfer_limit, one-tx-per-slot) are
//! intentionally NOT enforced here. The spender program enforces its own spend controls
//! before invoking this instruction, and adding an on-chain check here would duplicate
//! that enforcement. UserDelegateState tracking fields are therefore not updated on this path.

use crate::errors::ErrorCode;
use crate::events::CpiTransferExecuted;
use crate::instructions::add_or_update_user_delegate::USER_DELEGATE_SEED;
use crate::state::UserDelegateState;
use crate::ID;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

/// Deployed address of the spender program authorized to call this instruction.
const SPENDER_PROGRAM_ID: Pubkey = pubkey!("StripeNSMZs61AUf3mP6ny4qyWMPgWQ91DaVsLNgqFE");

/// Seed used by the spender program to derive its global SpenderState PDA.
const SPENDER_STATE_SEED: &[u8] = b"state";

#[derive(Accounts)]
#[instruction(merchant_id: u64)]
pub struct CpiTransfer<'info> {
    /// The SpenderState PDA of the spender program, passed as a signer via invoke_signed.
    /// Its presence as a signer proves this call originated from the spender program,
    /// which already validated the destination against its own whitelist before calling.
    #[account(
        constraint = caller_proof.key() == Pubkey::find_program_address(
            &[SPENDER_STATE_SEED],
            &SPENDER_PROGRAM_ID,
        ).0 @ ErrorCode::UnauthorizedCaller
    )]
    pub caller_proof: Signer<'info>,

    /// The UserDelegateState PDA that acts as the SPL delegate authority on user_token_account.
    /// Seeds verify this is the legitimate bridge-cards PDA for this merchant/mint/ATA.
    #[account(
        seeds = [
            USER_DELEGATE_SEED,
            merchant_id.to_le_bytes().as_ref(),
            mint.key().as_ref(),
            user_token_account.key().as_ref(),
        ],
        bump = user_delegate_account.bump,
        seeds::program = ID,
    )]
    pub user_delegate_account: Account<'info, UserDelegateState>,

    #[account(mut)]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    pub mint: InterfaceAccount<'info, Mint>,

    #[account(constraint = token_program.key() == *mint.to_account_info().owner @ ErrorCode::InvalidPda)]
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handler(ctx: Context<CpiTransfer>, merchant_id: u64, amount: u64) -> Result<()> {
    let merchant_id_bytes = merchant_id.to_le_bytes();
    let bump_bytes = [ctx.accounts.user_delegate_account.bump];
    let signer_seeds: &[&[&[u8]]] = &[&[
        USER_DELEGATE_SEED,
        merchant_id_bytes.as_ref(),
        ctx.accounts.mint.to_account_info().key.as_ref(),
        ctx.accounts
            .user_token_account
            .to_account_info()
            .key
            .as_ref(),
        &bump_bytes,
    ]];

    let pre_receiver = ctx.accounts.destination_token_account.amount;

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_token_account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                to: ctx.accounts.destination_token_account.to_account_info(),
                authority: ctx.accounts.user_delegate_account.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    ctx.accounts.destination_token_account.reload()?;
    let received_amount = ctx
        .accounts
        .destination_token_account
        .amount
        .checked_sub(pre_receiver)
        .ok_or(ErrorCode::InvalidAmount)?;

    emit!(CpiTransferExecuted {
        merchant_id,
        user_delegate: ctx.accounts.user_delegate_account.key(),
        user_ata: ctx.accounts.user_token_account.key(),
        destination_ata: ctx.accounts.destination_token_account.key(),
        mint: ctx.accounts.mint.key(),
        amount: received_amount,
    });

    Ok(())
}
