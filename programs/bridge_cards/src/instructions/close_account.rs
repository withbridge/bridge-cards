use crate::{errors::ErrorCode, events::AccountClosed, state::BridgeCardsState, ID, STATE_SEED};
use anchor_lang::{prelude::*, solana_program::system_program};

/**
 * Close a program-derived account and reclaim its rent.
 *
 * This instruction allows the program admin to close any program-derived account (PDA)
 * and recover its rent lamports. This is useful for cleaning up unused accounts and
 * recovering SOL locked in rent payments.
 *
 * Account Validation:
 * - Verifies the account is a valid PDA of this program
 * - Checks that the account is not the program state account
 * - Validates PDA derivation using provided seeds
 *
 * Security Model:
 * - Only the program admin can close accounts
 * - Admin authority verified through state PDA
 * - Prevents closing of critical program state
 * - Atomic closure and rent recovery
 *
 * Rent Recovery:
 * - Transfers all lamports from closed account to payer
 * - Account data is zeroed by runtime after instruction
 * - Rent exempt SOL is fully recovered
 *
 * Events Emitted:
 * - AccountClosed: When an account is successfully closed
 *   Fields: account (pubkey of closed account)
 *
 * Common Use Cases:
 * - Cleaning up unused merchant destinations
 * - Removing revoked user delegates
 * - Recovering rent from deprecated accounts
 * - General program maintenance
 *
 * Required Accounts:
 * - admin: Program admin with closure authority
 * - payer: Account to receive recovered rent
 * - account_to_close: PDA to be closed
 * - state: Global program state PDA
 *
 * @param input_seeds Seeds used to derive and validate the PDA
 */
#[derive(Accounts)]
pub struct CloseAccount<'info> {
    /// Program admin account, must match admin stored in state
    /// Required permissions: Signer
    #[account(mut, constraint = admin.key() == state.admin)]
    pub admin: Signer<'info>,

    /// Account that will receive the recovered rent
    /// Required permissions: Signer, Mutable (for rent receipt)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Program-derived account to be closed
    /// Required permissions: Mutable (for closure)
    /// CHECK: Account validity is verified through PDA derivation
    #[account(mut)]
    pub account_to_close: AccountInfo<'info>,

    /// Global program state storing the admin public key
    /// Seeds: [STATE_SEED]
    /// Required permissions: Read-only
    #[account(
        seeds = [STATE_SEED],
        bump,
        seeds::program = ID
    )]
    pub state: Account<'info, BridgeCardsState>,
}

/**
 * Process the closure of a program-derived account.
 *
 * @param ctx Context containing all required accounts
 * @param input_seeds Seeds used to derive and validate the PDA being closed
 *
 * Flow:
 * 1. Verify admin signature (done via account constraints)
 * 2. Validate account is a valid PDA using input seeds
 * 3. Verify account is not program state
 * 4. Transfer rent lamports to payer
 * 5. Emit closure event
 *
 * Error Handling:
 * - Returns InvalidPda if account is not a valid PDA
 * - Returns InvalidPda if attempting to close state account
 *
 * @return Result indicating success or containing an error
 */
pub fn handler(ctx: Context<CloseAccount>, input_seeds: Vec<Vec<u8>>) -> Result<()> {
    let account_to_close = &ctx.accounts.account_to_close;
    let payer = &ctx.accounts.payer;
    let seeds_slices: Vec<&[u8]> = input_seeds.iter().map(|s| s.as_slice()).collect();

    // Derive the PDA from the seeds and validate it matches
    let pda = Pubkey::find_program_address(&seeds_slices, ctx.program_id).0;
    if pda != account_to_close.key() {
        return Err(ErrorCode::InvalidPda.into());
    }

    // Prevent closing of program state account
    if pda == ctx.accounts.state.key() {
        return Err(ErrorCode::InvalidPda.into());
    }

    // Close account and transfer lamports
    close_account_and_transfer_lamports(account_to_close, payer)?;

    // Emit event for indexing and notifications
    emit!(AccountClosed {
        account: account_to_close.key(),
    });

    Ok(())
}

/// Schedule an account for closure by transferring its rent-exempt balance to the recipient
pub fn close_account_and_transfer_lamports<'info>(
    account_to_close: &AccountInfo<'info>,
    recipient: &AccountInfo<'info>,
) -> Result<()> {
    // Transfer all lamports from the account to the recipient
    let lamports = account_to_close.lamports();
    **account_to_close.try_borrow_mut_lamports()? = 0;
    **recipient.try_borrow_mut_lamports()? += lamports;

    // realloc the account to 0 bytes
    account_to_close.assign(&system_program::ID);
    account_to_close.realloc(0, false).unwrap();

    Ok(())
}
