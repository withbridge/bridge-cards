use crate::errors::ErrorCode;
use account_data_macro_derive::AccountData;
use account_data_trait::AccountData;
use anchor_lang::prelude::*;

/**
 * The global state of the BridgeCards program.
 *
 * This state account holds the admin pubkey which has authority to:
 * - Add/update merchant destinations
 * - Add/update merchant managers
 * - Close accounts
 * - Update the admin
 *
 * The bump field stores the PDA bump seed to avoid recalculation.
 */
#[account]
#[derive(InitSpace, AccountData)]
pub struct BridgeCardsState {
    // Signer allowed to change BridgeCardsState
    pub admin: Pubkey,
    // Bump seed used in PDA derivation
    pub bump: u8,
}

/**
 * State for a user's delegate account that can initiate transfers on their behalf.
 *
 * This account stores transfer limits and tracking for:
 * - Maximum amount per single transfer
 * - Maximum amount within a time period
 * - Amount transferred in current period
 * - Period reset timestamp
 * - Period duration
 *
 * The bump field stores the PDA bump seed to avoid recalculation.
 */
#[account]
#[derive(InitSpace, AccountData)]
pub struct UserDelegateState {
    // Maximum amount of tokens that can be transferred in a single transaction
    pub per_transfer_limit: u64,
    // Maximum amount of tokens that can be transferred within a period
    pub period_transfer_limit: u64,
    // Amount of tokens that have been transferred within the last period
    pub period_transferred_amount: u64,
    // Timestamp of when the period transferred amount was last reset
    pub period_timestamp_last_reset: u64,
    // Duration in seconds of the transfer limit period
    pub transfer_limit_period_seconds: u32,
    // Slot of the last transfer, if any
    pub slot_last_transferred: u64,
    // Bump seed used in PDA derivation
    pub bump: u8,
}

impl UserDelegateState {
    pub fn validate_debit_and_update(
        &mut self,
        amount: u64,
        current_time: u64,
        current_slot: u64,
    ) -> Result<()> {
        if amount > self.per_transfer_limit {
            return Err(ErrorCode::ExceedsMaxTransferLimit.into());
        }

        if self.slot_last_transferred == current_slot {
            return Err(ErrorCode::ExceedsMaxTransactionsPerSlot.into());
        }

        if current_time - self.period_timestamp_last_reset
            > self.transfer_limit_period_seconds as u64
        {
            self.period_transferred_amount = 0;
            self.period_timestamp_last_reset = current_time;
        }
        if self.period_transferred_amount.checked_add(amount).unwrap() > self.period_transfer_limit
        {
            return Err(ErrorCode::ExceedsTransferLimitPerPeriod.into());
        }

        // Only update state after all validations pass
        self.slot_last_transferred = current_slot;
        self.period_transferred_amount += amount;
        Ok(())
    }
}

/**
 * State for a merchant's destination token account.
 *
 * When allowed is true, this token account can receive transfers from user delegates
 * associated with this merchant. This is used to control which token accounts
 * can receive funds on behalf of the merchant.
 *
 * The bump field stores the PDA bump seed to avoid recalculation.
 */
#[account]
#[derive(InitSpace, AccountData)]
pub struct MerchantDestinationState {
    pub allowed: bool,
    // Bump seed used in PDA derivation
    pub bump: u8,
}

/**
 * State for a merchant's debitor account.
 *
 * When allowed is true, this account can initiate transfers from user delegates
 * associated with this merchant. This is used to control which accounts can
 * debit funds from users on behalf of the merchant.
 *
 * The bump field stores the PDA bump seed to avoid recalculation.
 */
#[account]
#[derive(InitSpace, AccountData)]
pub struct MerchantDebitorState {
    pub allowed: bool,
    // Bump seed used in PDA derivation
    pub bump: u8,
}

/**
 * State for a merchant's manager account.
 *
 * The manager pubkey stored here has authority to:
 * - Add/update user delegates for this merchant
 * - Add/update debitors for this merchant
 *
 * The bump field stores the PDA bump seed to avoid recalculation.
 */
#[account]
#[derive(InitSpace, AccountData)]
pub struct MerchantManagerState {
    pub manager: Pubkey,
    // Bump seed used in PDA derivation
    pub bump: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_delegate_state() -> UserDelegateState {
        UserDelegateState {
            per_transfer_limit: 1000,
            period_transfer_limit: 2000,
            period_transferred_amount: 0,
            period_timestamp_last_reset: 100,
            transfer_limit_period_seconds: 3600, // 1 hour
            slot_last_transferred: 0,
            bump: 0,
        }
    }

    #[test]
    fn test_successful_transfer() {
        let mut state = setup_delegate_state();
        assert!(state.validate_debit_and_update(500, 200, 1).is_ok());
        assert_eq!(state.period_transferred_amount, 500);
    }

    #[test]
    fn test_exceeds_per_transfer_limit() {
        let mut state = setup_delegate_state();
        let result = state.validate_debit_and_update(1500, 200, 1);
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error, ErrorCode::ExceedsMaxTransferLimit.into());
        }
    }

    #[test]
    fn test_exceeds_period_transfer_limit() {
        let mut state = setup_delegate_state();
        // First transfer
        assert!(state.validate_debit_and_update(900, 200, 1).is_ok());
        assert!(state.validate_debit_and_update(200, 300, 2).is_ok());

        // Second transfer that would exceed period limit
        let result = state.validate_debit_and_update(1000, 300, 1);
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error, ErrorCode::ExceedsTransferLimitPerPeriod.into());
        }
    }

    #[test]
    fn test_period_reset() {
        let mut state = setup_delegate_state();
        // Initial transfer
        assert!(state.validate_debit_and_update(900, 200, 1).is_ok());
        assert_eq!(state.period_transferred_amount, 900);

        // Transfer after period reset (3600 seconds + initial timestamp)
        assert!(state.validate_debit_and_update(900, 4000, 2).is_ok());
        assert_eq!(state.period_transferred_amount, 900);
        assert_eq!(state.period_timestamp_last_reset, 4000);
    }

    #[test]
    fn test_multiple_transfers_within_period() {
        let mut state = setup_delegate_state();
        // First transfer
        assert!(state.validate_debit_and_update(500, 200, 1).is_ok());
        assert_eq!(state.period_transferred_amount, 500);

        // Second transfer
        assert!(state.validate_debit_and_update(300, 300, 2).is_ok());
        assert_eq!(state.period_transferred_amount, 800);

        // Third transfer
        assert!(state.validate_debit_and_update(200, 400, 3).is_ok());
        assert_eq!(state.period_transferred_amount, 1000);
    }

    #[test]
    fn test_exceeds_max_transactions_per_slot() {
        let mut state = setup_delegate_state();
        // First transfer in slot 1
        assert!(state.validate_debit_and_update(500, 200, 1).is_ok());
        assert_eq!(state.slot_last_transferred, 1);

        // Second transfer in same slot should fail
        let result = state.validate_debit_and_update(300, 300, 1);
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error, ErrorCode::ExceedsMaxTransactionsPerSlot.into());
        }

        // Transfer in different slot should succeed
        assert!(state.validate_debit_and_update(200, 400, 2).is_ok());
        assert_eq!(state.slot_last_transferred, 2);
    }

    #[test]
    fn test_state_updates() {
        let DEFAULT_STATE: UserDelegateState = setup_delegate_state();
        let mut state = setup_delegate_state();

        // First transfer
        assert!(state.validate_debit_and_update(500, 200, 1).is_ok());

        // Verify all state updates after first transfer
        assert_eq!(
            state.per_transfer_limit, DEFAULT_STATE.per_transfer_limit,
            "per_transfer_limit should remain unchanged"
        );
        assert_eq!(
            state.period_transfer_limit, DEFAULT_STATE.period_transfer_limit,
            "period_transfer_limit should remain unchanged"
        );
        assert_eq!(
            state.period_transferred_amount, 500,
            "period_transferred_amount should be updated"
        );
        assert_eq!(
            state.period_timestamp_last_reset, 100,
            "period_timestamp should remain unchanged as period not elapsed"
        );
        assert_eq!(
            state.transfer_limit_period_seconds, DEFAULT_STATE.transfer_limit_period_seconds,
            "transfer_limit_period should remain unchanged"
        );
        assert_eq!(
            state.slot_last_transferred, 1,
            "slot_last_transferred should be updated"
        );

        // Second transfer in new slot but same period
        assert!(state.validate_debit_and_update(300, 300, 2).is_ok());

        // Verify all state updates after second transfer
        assert_eq!(
            state.per_transfer_limit, 1000,
            "per_transfer_limit should remain unchanged"
        );
        assert_eq!(
            state.period_transfer_limit, 2000,
            "period_transfer_limit should remain unchanged"
        );
        assert_eq!(
            state.period_transferred_amount, 800,
            "period_transferred_amount should accumulate"
        );
        assert_eq!(
            state.period_timestamp_last_reset, 100,
            "period_timestamp should remain unchanged as period not elapsed"
        );
        assert_eq!(
            state.transfer_limit_period_seconds, 3600,
            "transfer_limit_period should remain unchanged"
        );
        assert_eq!(
            state.slot_last_transferred, 2,
            "slot_last_transferred should be updated"
        );

        // Transfer after period reset
        assert!(state.validate_debit_and_update(400, 4000, 3).is_ok());

        // Verify all state updates after period reset
        assert_eq!(
            state.per_transfer_limit, 1000,
            "per_transfer_limit should remain unchanged"
        );
        assert_eq!(
            state.period_transfer_limit, 2000,
            "period_transfer_limit should remain unchanged"
        );
        assert_eq!(
            state.period_transferred_amount, 400,
            "period_transferred_amount should reset and reflect new transfer"
        );
        assert_eq!(
            state.period_timestamp_last_reset, 4000,
            "period_timestamp should update to new period"
        );
        assert_eq!(
            state.transfer_limit_period_seconds, 3600,
            "transfer_limit_period should remain unchanged"
        );
        assert_eq!(
            state.slot_last_transferred, 3,
            "slot_last_transferred should be updated"
        );

        let result = state.validate_debit_and_update(1001, 4100, 4);
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error, ErrorCode::ExceedsMaxTransferLimit.into());
        }

        // Verify state unchanged after failed transfer
        assert_eq!(
            state.per_transfer_limit, DEFAULT_STATE.per_transfer_limit,
            "per_transfer_limit should remain unchanged after failed transfer"
        );
        assert_eq!(
            state.period_transfer_limit, DEFAULT_STATE.period_transfer_limit,
            "period_transfer_limit should remain unchanged after failed transfer"
        );
        assert_eq!(
            state.period_transferred_amount, 400,
            "period_transferred_amount should remain unchanged after failed transfer"
        );
        assert_eq!(
            state.period_timestamp_last_reset, 4000,
            "period_timestamp should remain unchanged after failed transfer"
        );
        assert_eq!(
            state.transfer_limit_period_seconds, DEFAULT_STATE.transfer_limit_period_seconds,
            "transfer_limit_period should remain unchanged after failed transfer"
        );
        assert_eq!(
            state.slot_last_transferred, 3,
            "slot_last_transferred should remain unchanged after failed transfer"
        );

        // Verify we can still make a transfer within the remaining period limit
        assert!(state.validate_debit_and_update(500, 4200, 5).is_ok());
        assert_eq!(
            state.period_transferred_amount, 900,
            "period_transferred_amount should reflect accumulated transfers"
        );
        assert_eq!(
            state.slot_last_transferred, 5,
            "slot_last_transferred should be updated"
        );
    }
}
