use crate::events::UserDebited;
use crate::instructions::add_or_update_user_delegate::USER_DELEGATE_SEED;
use crate::state::{MerchantDebitorState, MerchantDestinationState, UserDelegateState};
use crate::ID;
use crate::{MERCHANT_DEBITOR_SEED, MERCHANT_DESTINATION_SEED};
use anchor_lang::prelude::*;
use anchor_spl::token_interface;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

/**
 * Debit tokens from a user's token account via their UserDelegate.
 *
 * This instruction allows an authorized debitor to transfer tokens from a user's token account
 * to an authorized destination token account. The transfer must satisfy multiple security checks:
 *
 * Security Checks:
 * - Debitor must be authorized for the merchant (debitor_state.allowed == true)
 * - Destination must be authorized for the merchant (destination_state.allowed == true)
 * - Transfer amount must not exceed delegate's per-transfer limit
 * - Transfer amount must not exceed delegate's remaining period limit
 * - Source and destination token accounts must use the same mint
 *
 * Account Derivation:
 * - User delegate PDA: [USER_DELEGATE_SEED, merchant_id, mint, user_token_account]
 * - Debitor state PDA: [MERCHANT_DEBITOR_SEED, merchant_id, debitor]
 * - Destination state PDA: [MERCHANT_DESTINATION_SEED, merchant_id, mint, destination_token_account]
 *
 * Transaction Flow:
 * 1. Validate debitor and destination are authorized
 * 2. Check transfer limits and update period tracking
 * 3. Execute token transfer using the delegate PDA as authority
 *
 * Common Errors:
 * - ExceedsMaxTransferLimit: Amount exceeds per-transfer limit
 * - ExceedsTransferLimitPerPeriod: Amount exceeds remaining period limit
 * - MismatchedMint: Source and destination token accounts have different mints
 */
#[derive(Accounts)]
#[instruction(merchant_id: u64)]
pub struct DebitUser<'info> {
    /// Account that pays for the transaction fees and rent
    /// CHECK: Can be any account with sufficient SOL
    pub payer: Signer<'info>,

    /// Program Derived Address (PDA) that stores the delegate's transfer limits and state
    /// This account acts as the authority for the user's token account
    ///
    /// Seeds: [USER_DELEGATE_SEED, merchant_id, mint, user_token_account]
    /// Required permissions: Mutable (updates period tracking)
    #[account(mut,
        seeds = [USER_DELEGATE_SEED, merchant_id.to_le_bytes().as_ref(), mint.key().as_ref(), user_token_account.key().as_ref()],
        bump = user_delegate_account.bump,
        seeds::program = ID
    )]
    pub user_delegate_account: Account<'info, UserDelegateState>,

    /// Account initiating the debit operation
    /// Must be an authorized debitor for the merchant
    /// Required permissions: Signer
    #[account( constraint = debitor_state.allowed)]
    pub debitor: Signer<'info>,

    /// PDA storing the debitor's authorization state for this merchant
    /// Seeds: [MERCHANT_DEBITOR_SEED, merchant_id, debitor]
    /// Required permissions: Read-only
    #[account(seeds = [MERCHANT_DEBITOR_SEED, &merchant_id.to_le_bytes().as_ref(), mint.key().as_ref(), debitor.key().as_ref()], bump = debitor_state.bump, seeds::program = ID)]
    pub debitor_state: Account<'info, MerchantDebitorState>,

    /// Token account that will receive the transferred tokens
    /// Must be an authorized destination for the merchant
    /// Required permissions: Mutable
    #[account(
        mut,
        constraint = destination_state.allowed
    )]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    /// PDA storing the destination's authorization state for this merchant and mint
    /// Seeds: [MERCHANT_DESTINATION_SEED, merchant_id, mint, destination_token_account]
    /// Required permissions: Read-only
    #[account(
        seeds = [MERCHANT_DESTINATION_SEED, &merchant_id.to_le_bytes().as_ref(), mint.key().as_ref(), destination_token_account.key().as_ref()],
        bump = destination_state.bump,
        seeds::program = ID)]
    pub destination_state: Account<'info, MerchantDestinationState>,

    /// User's token account from which tokens will be transferred
    /// Must have the same mint as the destination account
    /// Required permissions: Mutable
    #[account(mut, constraint = user_token_account.mint.key() == mint.key())]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The mint of the tokens being transferred
    /// Used to verify token account compatibility and for PDA derivation
    /// Required permissions: Read-only
    pub mint: InterfaceAccount<'info, Mint>,

    /// Required Solana system programs
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

/**
 * Process a debit operation from a user's token account.
 *
 * @param ctx The instruction context containing all required accounts
 * @param merchant_id Unique identifier for the merchant
 * @param amount Number of tokens to transfer (in smallest units)
 *
 * Security:
 * - Validates transfer limits and updates period tracking
 * - Uses PDA as authority for token transfer
 * - Performs checked transfer to validate amount and mint
 *
 * @return Result indicating success or containing an error
 */
pub fn handler(ctx: Context<DebitUser>, merchant_id: u64, amount: u64) -> Result<()> {
    // Validate transfer limits and update period tracking
    let clock = Clock::get()?;
    ctx.accounts
        .user_delegate_account
        .validate_debit_and_update(amount, clock.unix_timestamp as u64, clock.slot)?;

    // Derive the PDA signer seeds for the delegate account
    let merchant_id_bytes = merchant_id.to_le_bytes();
    let seeds = [
        USER_DELEGATE_SEED,
        merchant_id_bytes.as_ref(),
        ctx.accounts.mint.to_account_info().key.as_ref(),
        ctx.accounts
            .user_token_account
            .to_account_info()
            .key
            .as_ref(),
        &[ctx.accounts.user_delegate_account.bump],
    ];
    let signer_seeds = &[&seeds[..]];

    // Execute the token transfer with amount and decimal validation
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_interface::TransferChecked {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.destination_token_account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                authority: ctx.accounts.user_delegate_account.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    emit!(UserDebited {
        debitor: ctx.accounts.debitor.key(),
        user_delegate: ctx.accounts.user_delegate_account.key(),
        merchant_id,
        user_ata: ctx.accounts.user_token_account.key(),
        destination_ata: ctx.accounts.destination_token_account.key(),
        mint: ctx.accounts.mint.key(),
        amount,
    });

    Ok(())
}
