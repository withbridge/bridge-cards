use anchor_lang::prelude::*;

/**
 * Error codes for the Bridge Cards program.
 * These errors can occur during instruction execution and should be handled appropriately
 * by client applications.
 */
#[error_code]
pub enum ErrorCode {
    /**
     * The requested transfer would exceed the delegate's period transfer limit.
     *
     * This error occurs when:
     * - A debit_user instruction is called
     * - The total amount transferred in the current period plus the requested amount
     *   would exceed the delegate's period_transfer_limit
     *
     * How to handle:
     * - Wait for the current period to end (period_timestamp_last_reset + transfer_limit_period_seconds)
     * - Request a smaller transfer amount
     * - Request the merchant manager to increase the period transfer limit
     */
    #[msg("Exceeds transfer limit per period")]
    ExceedsTransferLimitPerPeriod,

    /**
     * The requested transfer would exceed the delegate's maximum transfer limit.
     *
     * This error occurs when:
     * - A debit_user instruction is called
     * - The requested amount exceeds the delegate's per_transfer_limit
     *
     * How to handle:
     * - Split the transfer into smaller amounts
     * - Request the merchant manager to increase the max transfer limit
     */
    #[msg("Exceeds max transfer limit")]
    ExceedsMaxTransferLimit,

    /**
     * The provided Program Derived Address (PDA) is invalid.
     *
     * This error occurs when:
     * - The provided PDA does not match the expected seeds
     * - The PDA derivation fails
     *
     * How to handle:
     * - Verify the seeds used to derive the PDA
     * - Check that the correct program ID is being used
     * - Ensure all seed components are correct (merchant_id, etc.)
     */
    #[msg("Invalid PDA")]
    InvalidPda,

    /**
     * The requested transfer would exceed the delegate's max transactions per slot.
     *
     * This error occurs when:
     * - A debit_user instruction is called
     * - There has already been a transaction in the current slot
     */
    #[msg("Exceeds max transactions per slot")]
    ExceedsMaxTransactionsPerSlot,

    /// Returned by transfer instructions when the program is paused.
    #[msg("Program is paused")]
    ProgramPaused,

    #[msg("Too many destination accounts, maximum is 10")]
    TooManyDestinations,

    #[msg("Remaining accounts must be provided in destination/destination_state pairs")]
    InvalidRemainingAccounts,

    #[msg("Role address cannot be the zero address")]
    ZeroAddress,

    #[msg("Delegation account has an unknown type discriminator")]
    InvalidDelegationType,

    #[msg("Destination account must be a token account")]
    InvalidDestinationAccount,

    /// Returned when amount is zero.
    #[msg("Invalid amount, must be greater than zero")]
    InvalidAmount,

    /// Returned when the signer is not authorized for the requested operation.
    #[msg("Unauthorized")]
    Unauthorized,
}
