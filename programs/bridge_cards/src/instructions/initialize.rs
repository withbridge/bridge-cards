use crate::state::BridgeCardsState;
use anchor_lang::prelude::*;

/// Seed used to derive the global state PDA
pub const STATE_SEED: &[u8] = b"state";

/**
 * Initialize the BridgeCards program.
 *
 * This instruction creates the global state Program Derived Address (PDA) and sets up
 * the initial program administrator. This is a one-time operation that must be called
 * before any other program instructions can be used.
 *
 * State Account:
 * - Created as a PDA with seed [STATE_SEED]
 * - Stores the admin public key
 * - Funded by the payer account
 *
 * Admin Privileges:
 * - Add/update merchant destinations (control where tokens can be sent)
 * - Add/update merchant managers (delegate merchant management)
 * - Close program accounts (recover rent)
 * - Update admin authority (transfer admin rights)
 *
 * Security Considerations:
 * - The payer becomes the admin and should be a secure, controlled account
 * - Admin authority is critical and should be managed carefully
 * - State PDA can only be initialized once
 *
 * Required Accounts:
 * - payer: Account that will pay for state account creation and become admin
 * - state: PDA that will store global program state
 * - system_program: Required for account creation
 */
#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Account that will pay for state account creation and become the admin
    /// Required permissions: Signer, Mutable (for rent payment)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Program Derived Address that will store global program state
    /// Seeds: [STATE_SEED]
    /// Space: Discriminator + Admin Pubkey
    /// Required permissions: None (account is being created)
    #[account(
        init,
        payer = payer,
        space = BridgeCardsState::DISCRIMINATOR.len() + BridgeCardsState::INIT_SPACE,
        seeds = [STATE_SEED],
        bump,
    )]
    pub state: Account<'info, BridgeCardsState>,

    #[account(constraint = auth_initialize(program_account.key()))]
    pub program_account: Signer<'info>,

    /// Required for account creation
    pub system_program: Program<'info, System>,
}

/**
 * Initialize the program state and set the admin.
 *
 * @param ctx Context containing the payer (future admin) and state accounts
 * @return Result indicating success or containing an error
 */
pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    let state = &mut ctx.accounts.state;
    state.admin = ctx.accounts.payer.key();
    state.bump = ctx.bumps.state;
    Ok(())
}

#[cfg(feature = "local")]
fn auth_initialize(account: Pubkey) -> bool {
    use crate::ID;

    account == ID
}

#[cfg(not(feature = "local"))]
fn auth_initialize(_: Pubkey) -> bool {
    true
}
