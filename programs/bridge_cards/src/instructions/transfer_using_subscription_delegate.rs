//! Executes a token transfer using either a fixed or recurring delegation from the
//! subscriptions program. The delegation type is determined at runtime by reading
//! the discriminator byte from the delegation_pda account data.

use crate::instructions::initialize_spender_state::SPENDER_STATE_SEED;
use crate::instructions::setup_merchant_delegate::{DELEGATE_DESTINATION_SEED, MERCHANT_DELEGATE_SEED};
use crate::{
    errors::ErrorCode,
    events::{FixedDelegationTransfer, RecurringDelegationTransfer},
    state::*,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use subscriptions::instructions::{TransferFixedCpiBuilder, TransferRecurringCpiBuilder};
use subscriptions::types::TransferData;

pub const SUBSCRIPTIONS_PROGRAM: Pubkey =
    pubkey!("De1egAFMkMWZSN5rYXRj9CAdheBamobVNubTsi9avR44");

// AccountDiscriminator::FixedDelegation = 2, RecurringDelegation = 3
const FIXED_DELEGATION_DISCRIMINATOR: u8 = 2;
const RECURRING_DELEGATION_DISCRIMINATOR: u8 = 3;

#[derive(Accounts)]
#[instruction(merchant_id: [u8; 32])]
pub struct TransferUsingSubscriptionDelegate<'info> {
    #[account(constraint = debitor.key() == spender_state.debitor)]
    pub debitor: Signer<'info>,

    #[account(
        seeds = [SPENDER_STATE_SEED],
        bump = spender_state.bump,
    )]
    pub spender_state: Account<'info, SpenderState>,

    /// Signs the subscriptions CPI as the delegatee.
    #[account(
        seeds = [MERCHANT_DELEGATE_SEED, merchant_id.as_ref()],
        bump = merchant_delegate_state.bump,
    )]
    pub merchant_delegate_state: Account<'info, MerchantDelegateState>,

    /// CHECK: Verified by the subscriptions program; first byte is read to determine fixed vs recurring.
    #[account(mut)]
    pub delegation_pda: UncheckedAccount<'info>,

    /// CHECK: Subscription-authority PDA on the subscriptions program.
    pub subscription_authority: UncheckedAccount<'info>,

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

    /// CHECK: Event authority PDA on the subscriptions program.
    pub event_authority: UncheckedAccount<'info>,

    /// CHECK: Subscriptions program; also passed as self_program for the internal CPI.
    #[account(constraint = subscriptions_program.key() == SUBSCRIPTIONS_PROGRAM @ ErrorCode::InvalidPda)]
    pub subscriptions_program: UncheckedAccount<'info>,
}

pub fn handler<'info>(
    ctx: Context<'info, TransferUsingSubscriptionDelegate<'info>>,
    merchant_id: [u8; 32],
    delegator: Pubkey,
    mint: Pubkey,
    amount: u64,
) -> Result<()> {
    require!(!ctx.accounts.spender_state.paused, ErrorCode::ProgramPaused);
    require!(amount > 0, ErrorCode::InvalidAmount);
    require!(
        ctx.accounts.delegator_ata.owner == delegator,
        ErrorCode::InvalidPda
    );
    require!(ctx.accounts.token_mint.key() == mint, ErrorCode::InvalidPda);
    require!(
        ctx.accounts.receiver_ata.mint == ctx.accounts.token_mint.key(),
        ErrorCode::InvalidPda
    );
    require!(
        ctx.accounts.delegator_ata.mint == ctx.accounts.token_mint.key(),
        ErrorCode::InvalidPda
    );
    require!(
        ctx.accounts.delegation_pda.owner == &SUBSCRIPTIONS_PROGRAM,
        ErrorCode::InvalidPda
    );

    let discriminator = {
        let data = ctx.accounts.delegation_pda.data.borrow();
        require!(!data.is_empty(), ErrorCode::InvalidDelegationType);
        data[0]
    };

    let bump_bytes = [ctx.accounts.merchant_delegate_state.bump];
    let signer_seeds: &[&[&[u8]]] =
        &[&[MERCHANT_DELEGATE_SEED, merchant_id.as_ref(), &bump_bytes]];

    let pre_receiver = ctx.accounts.receiver_ata.amount;

    match discriminator {
        FIXED_DELEGATION_DISCRIMINATOR => {
            TransferFixedCpiBuilder::new(&ctx.accounts.subscriptions_program.to_account_info())
                .delegation_pda(&ctx.accounts.delegation_pda.to_account_info())
                .subscription_authority(&ctx.accounts.subscription_authority.to_account_info())
                .delegator_ata(&ctx.accounts.delegator_ata.to_account_info())
                .receiver_ata(&ctx.accounts.receiver_ata.to_account_info())
                .token_mint(&ctx.accounts.token_mint.to_account_info())
                .token_program(&ctx.accounts.token_program.to_account_info())
                .delegatee(&ctx.accounts.merchant_delegate_state.to_account_info())
                .event_authority(&ctx.accounts.event_authority.to_account_info())
                .self_program(&ctx.accounts.subscriptions_program.to_account_info())
                .transfer_data(TransferData {
                    amount,
                    delegator,
                    mint,
                })
                .invoke_signed(signer_seeds)?;

            ctx.accounts.receiver_ata.reload()?;
            let received_amount = ctx
                .accounts
                .receiver_ata
                .amount
                .checked_sub(pre_receiver)
                .ok_or(ErrorCode::InvalidAmount)?;

            emit!(FixedDelegationTransfer {
                merchant_id,
                user: delegator,
                receiver: ctx.accounts.receiver_ata.key(),
                debitor: ctx.accounts.debitor.key(),
                amount: received_amount,
            });
        }
        RECURRING_DELEGATION_DISCRIMINATOR => {
            TransferRecurringCpiBuilder::new(&ctx.accounts.subscriptions_program.to_account_info())
                .delegation_pda(&ctx.accounts.delegation_pda.to_account_info())
                .subscription_authority(&ctx.accounts.subscription_authority.to_account_info())
                .delegator_ata(&ctx.accounts.delegator_ata.to_account_info())
                .receiver_ata(&ctx.accounts.receiver_ata.to_account_info())
                .token_mint(&ctx.accounts.token_mint.to_account_info())
                .token_program(&ctx.accounts.token_program.to_account_info())
                .delegatee(&ctx.accounts.merchant_delegate_state.to_account_info())
                .event_authority(&ctx.accounts.event_authority.to_account_info())
                .self_program(&ctx.accounts.subscriptions_program.to_account_info())
                .transfer_data(TransferData {
                    amount,
                    delegator,
                    mint,
                })
                .invoke_signed(signer_seeds)?;

            ctx.accounts.receiver_ata.reload()?;
            let received_amount = ctx
                .accounts
                .receiver_ata
                .amount
                .checked_sub(pre_receiver)
                .ok_or(ErrorCode::InvalidAmount)?;

            emit!(RecurringDelegationTransfer {
                merchant_id,
                user: delegator,
                receiver: ctx.accounts.receiver_ata.key(),
                debitor: ctx.accounts.debitor.key(),
                amount: received_amount,
            });
        }
        _ => return err!(ErrorCode::InvalidDelegationType),
    }

    Ok(())
}
