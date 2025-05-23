use anchor_lang::prelude::*;

/**
 * Event emitted when a merchant's debitor account is added or its state is updated.
 * This event is emitted by the add_or_update_merchant_debitor instruction.
 *
 * Fields:
 * @field merchant_id - Unique identifier of the merchant
 * @field state_pda - Public key of the debitor state PDA
 * @field debitor - Public key of the debitor account
 * @field previous_state - Previous authorization state (true if was allowed)
 * @field new_state - New authorization state (true if now allowed)
 */
#[event]
pub struct MerchantDebitorAddedOrUpdated {
    pub merchant_id: u64,
    pub state_pda: Pubkey,
    pub debitor: Pubkey,
    pub previous_state: bool,
    pub new_state: bool,
}

/**
 * Event emitted when a merchant's destination account is added or its state is updated.
 * This event is emitted by the add_or_update_merchant_destination instruction.
 *
 * Fields:
 * @field merchant_id - Unique identifier of the merchant
 * @field mint - Public key of the token mint
 * @field destination - Public key of the destination token account
 * @field state_pda - Public key of the destination state PDA
 * @field previous_state - Previous authorization state (true if was allowed)
 * @field new_state - New authorization state (true if now allowed)
 */
#[event]
pub struct MerchantDestinationAddedOrUpdated {
    pub merchant_id: u64,
    pub mint: Pubkey,
    pub destination: Pubkey,
    pub state_pda: Pubkey,
    pub previous_state: bool,
    pub new_state: bool,
}

/**
 * Event emitted when the program admin is updated.
 * This event is emitted by the update_admin instruction.
 *
 * Fields:
 * @field admin - Public key of the new admin account
 */
#[event]
pub struct AdminUpdated {
    pub admin: Pubkey,
}

/**
 * Event emitted when a user delegate is added or updated for a merchant.
 * This event is emitted by the add_or_update_user_delegate instruction.
 *
 * Fields:
 * @field merchant_pda - Public key of the merchant's PDA
 * @field user_delegate - Public key of the delegate account being added/updated
 */
#[event]
pub struct UserDelegateAddedOrUpdated {
    pub merchant_id: u64,
    pub mint: Pubkey,
    pub user_ata: Pubkey,
    pub user_delegate: Pubkey,
}

/**
 * Event emitted when a program account is closed.
 * This event is emitted by the close_account instruction.
 *
 * Fields:
 * @field account - Public key of the account that was closed
 */
#[event]
pub struct AccountClosed {
    pub account: Pubkey,
}

/**
 * Event emitted when a merchant manager is added or updated.
 * This event is emitted by the add_or_update_merchant_manager instruction.
 *
 * Fields:
 * @field merchant_id - Unique identifier of the merchant
 * @field manager - Public key of the manager account
 */
#[event]
pub struct MerchantManagerAddedOrUpdated {
    pub merchant_id: u64,
    pub manager: Pubkey,
}

/**
 * Event emitted when a user is debited by a merchant.
 * This event is emitted by the debit_user instruction.
 *
 * Fields:
 * @field debitor - Public key of the merchant debitor account that initiated the debit
 * @field user_delegate - Public key of the user's delegate account that authorized the debit
 * @field merchant_id - Unique identifier of the merchant
 * @field user_ata - Public key of the user's associated token account being debited
 * @field destination_ata - Public key of the destination associated token account receiving the funds
 * @field mint - Public key of the token mint being transferred
 * @field amount - Amount of tokens being transferred
 * @field user_nonce - Unique nonce to prevent replay attacks
 */
#[event]
pub struct UserDebited {
    pub debitor: Pubkey,
    pub user_delegate: Pubkey,
    pub merchant_id: u64,
    pub user_ata: Pubkey,
    pub destination_ata: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}
