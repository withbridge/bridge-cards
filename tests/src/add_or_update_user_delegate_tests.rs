use crate::common::*;
use account_data_trait::AccountData;
use anchor_lang::{prelude::*, Event};
use base64;
use bridge_cards::{events::UserDelegateAddedOrUpdated, state::UserDelegateState};
use litesvm_token::CreateAssociatedTokenAccountIdempotent;
use solana_program_test::tokio;
use solana_sdk::signature::Signer;

const DEFAULT_MAX_TRANSFER_LIMIT: u64 = 100_000_000; // $100 per transaction
const DEFAULT_PERIOD_TRANSFER_LIMIT: u64 = 2_000_000_000; // $2000 per day
const LIMIT_PERIOD: u32 = 86400; // 1 day in seconds
const TEST_MERCHANT_ID: u64 = 1;

#[tokio::test]
async fn test_create_user_delegate() {
    // Step 1: Create the context and initialize bridge cards
    let mut ctx = setup_and_initialize();

    // Step 2: Create a token mint and accounts
    let mint_pk = setup_mint(&mut ctx);

    // Create user and their token account
    let (_, user_pk) = setup_keypair(&mut ctx);

    // Create token account for the user
    let user_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&user_pk)
            .send()
            .unwrap();

    // Step 3: Create the user delegate account
    let user_delegate_pda = make_user_delegate_pda(
        TEST_MERCHANT_ID,
        &mint_pk,
        &user_token_account,
        &ctx.program_id,
    );

    let accounts = bridge_cards::accounts::AddOrUpdateUserDelegate {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        user_token_account,
        mint: mint_pk,
        user_delegate_account: user_delegate_pda.pubkey,
        system_program: System::id(),
    };

    let ix = create_add_or_update_user_delegate_instruction(
        &ctx,
        &accounts,
        TEST_MERCHANT_ID,
        DEFAULT_MAX_TRANSFER_LIMIT,
        DEFAULT_PERIOD_TRANSFER_LIMIT,
        LIMIT_PERIOD,
    );
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );

    // Step 4: Process the transaction
    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_ok(),
        "Failed to create user delegate: {:?}",
        result.err()
    );

    // Verify the UserDelegateAddedOrUpdated event
    let meta = result.unwrap();
    let mut event_found = false;
    let expected_event = UserDelegateAddedOrUpdated {
        merchant_id: TEST_MERCHANT_ID,
        mint: mint_pk,
        user_ata: user_token_account,
        user_delegate: user_delegate_pda.pubkey,
    };
    for log in meta.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) = UserDelegateAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.merchant_id, expected_event.merchant_id);
                        assert_eq!(parsed_event.mint, expected_event.mint);
                        assert_eq!(parsed_event.user_ata, expected_event.user_ata);
                        assert_eq!(parsed_event.user_delegate, expected_event.user_delegate);
                        event_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event_found,
        "UserDelegateAddedOrUpdated event not found for creation: {}",
        meta.logs.join("\n")
    );

    // Step 5: Verify the user delegate state
    let user_delegate_account = ctx.svm.get_account(&user_delegate_pda.pubkey).unwrap();

    // Verify the PDA was created with the expected settings
    let data = user_delegate_account.data.clone();
    let user_delegate_state = UserDelegateState {
        per_transfer_limit: 100_000_000,      // $100 per transaction
        period_transfer_limit: 2_000_000_000, // $2000 per day
        period_transferred_amount: 0,
        period_timestamp_last_reset: 0,
        slot_last_transferred: 0,
        transfer_limit_period_seconds: anchor_lang::solana_program::clock::SECONDS_PER_DAY as u32,
        bump: user_delegate_pda.bump,
    };
    let expected_data = user_delegate_state.account_data();

    assert_eq!(data, expected_data, "User delegate state data mismatch");
}

#[tokio::test]
async fn test_non_manager_cannot_create_user_delegate() {
    // Step 1: Create the context and initialize bridge cards
    let mut ctx = setup_and_initialize();

    // Step 2: Create a non-admin signer
    let (non_manager_kp, non_manager_pk) = setup_keypair(&mut ctx);

    // Step 3: Create a token mint and accounts
    let mint_pk = setup_mint(&mut ctx);

    // Create user and their token account
    let (_, user_pk) = setup_keypair(&mut ctx);

    // Create token account for the user
    let user_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&user_pk)
            .send()
            .unwrap();

    // Step 3: Create the user delegate account with non-admin
    let user_delegate_pda = make_user_delegate_pda(
        TEST_MERCHANT_ID,
        &mint_pk,
        &user_token_account,
        &ctx.program_id,
    );

    let accounts = bridge_cards::accounts::AddOrUpdateUserDelegate {
        manager: non_manager_pk,
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        user_token_account,
        mint: mint_pk,
        user_delegate_account: user_delegate_pda.pubkey,
        system_program: System::id(),
    };

    let ix = create_add_or_update_user_delegate_instruction(
        &ctx,
        &accounts,
        TEST_MERCHANT_ID,
        DEFAULT_MAX_TRANSFER_LIMIT,
        DEFAULT_PERIOD_TRANSFER_LIMIT,
        LIMIT_PERIOD,
    );
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &non_manager_kp],
    );

    // Step 4: Process the transaction - should fail due to constraint violation
    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_err(),
        "Non-admin should not be able to create user delegate"
    );

    // Verify the error type (constraint error)
    let err = result.err().unwrap();
    println!("Error: {:?}", err);

    // The error should match the constraint violation for admin check
    assert!(
        err.err.to_string().contains("custom program error"),
        "Error doesn't match expected constraint violation"
    );
}

#[tokio::test]
async fn test_update_user_delegate() {
    // Step 1: Create the context and initialize bridge cards
    let mut ctx = setup_and_initialize();

    // Step 2: Create a token mint and accounts
    let mint_pk = setup_mint(&mut ctx);

    // Create user and their token account
    let (_, user_pk) = setup_keypair(&mut ctx);

    // Create token account for the user
    let user_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&user_pk)
            .send()
            .unwrap();

    // Step 3: Create the user delegate account
    let user_delegate_pda = make_user_delegate_pda(
        TEST_MERCHANT_ID,
        &mint_pk,
        &user_token_account,
        &ctx.program_id,
    );

    let accounts = bridge_cards::accounts::AddOrUpdateUserDelegate {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        user_token_account,
        mint: mint_pk,
        user_delegate_account: user_delegate_pda.pubkey,
        system_program: System::id(),
    };

    // Create initial user delegate account
    let ix = create_add_or_update_user_delegate_instruction(
        &ctx,
        &accounts,
        TEST_MERCHANT_ID,
        DEFAULT_MAX_TRANSFER_LIMIT,
        DEFAULT_PERIOD_TRANSFER_LIMIT,
        LIMIT_PERIOD,
    );
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_ok(),
        "Failed to create initial user delegate: {:?}",
        result.err()
    );

    // Verify the event for the initial creation (optional)
    let meta1 = result.unwrap();
    let mut event1_found = false;
    for log in meta1.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if UserDelegateAddedOrUpdated::try_from_slice(event_data).is_ok() {
                        event1_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event1_found,
        "UserDelegateAddedOrUpdated event not found for initial creation: {}",
        meta1.logs.join("\n")
    );

    // Verify the initial user delegate state
    let initial_user_delegate_account = ctx.svm.get_account(&user_delegate_pda.pubkey).unwrap();
    let initial_data = initial_user_delegate_account.data.clone();
    let initial_state = UserDelegateState {
        per_transfer_limit: 100_000_000,      // $100 per transaction
        period_transfer_limit: 2_000_000_000, // $2000 per day
        period_transferred_amount: 0,
        period_timestamp_last_reset: 0,
        slot_last_transferred: 0,
        transfer_limit_period_seconds: anchor_lang::solana_program::clock::SECONDS_PER_DAY as u32,
        bump: user_delegate_pda.bump,
    };
    let expected_initial_data = initial_state.account_data();
    assert_eq!(
        initial_data, expected_initial_data,
        "Initial user delegate state data mismatch"
    );

    // Step 4: Update the user delegate account
    let update_accounts = bridge_cards::accounts::AddOrUpdateUserDelegate {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        user_token_account,
        mint: mint_pk,
        user_delegate_account: user_delegate_pda.pubkey,
        system_program: System::id(),
    };

    let update_ix = create_add_or_update_user_delegate_instruction(
        &ctx,
        &update_accounts,
        TEST_MERCHANT_ID,
        DEFAULT_MAX_TRANSFER_LIMIT * 2,
        DEFAULT_PERIOD_TRANSFER_LIMIT,
        LIMIT_PERIOD,
    );
    let update_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[update_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );

    // Step 4: Process the update transaction
    let update_result = submit_transaction(&mut ctx, update_tx);
    assert!(
        update_result.is_ok(),
        "Failed to update user delegate: {:?}",
        update_result.err()
    );

    // Verify the UserDelegateAddedOrUpdated event for the update
    let meta2 = update_result.unwrap();
    let mut event2_found = false;
    for log in meta2.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) = UserDelegateAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.user_delegate, user_delegate_pda.pubkey);
                        event2_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event2_found,
        "UserDelegateAddedOrUpdated event not found for update: {}",
        meta2.logs.join("\n")
    );

    // Step 6: Verify the updated user delegate state
    let updated_user_delegate_account = ctx.svm.get_account(&user_delegate_pda.pubkey).unwrap();
    let updated_data = updated_user_delegate_account.data.clone();

    // The account should be reinitialized with the values from the handler
    let expected_updated_state = UserDelegateState {
        per_transfer_limit: 200_000_000,      // $200 per transaction
        period_transfer_limit: 2_000_000_000, // $2000 per day
        period_transferred_amount: 0,         // Reset to 0
        period_timestamp_last_reset: 0,       // Updated timestamp
        slot_last_transferred: 0,
        transfer_limit_period_seconds: anchor_lang::solana_program::clock::SECONDS_PER_DAY as u32,
        bump: user_delegate_pda.bump,
    };
    let expected_updated_data = expected_updated_state.account_data();

    assert_eq!(
        updated_data, expected_updated_data,
        "Updated user delegate state data mismatch"
    );
}
