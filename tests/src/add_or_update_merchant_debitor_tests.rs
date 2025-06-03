use crate::common::*;
use account_data_trait::AccountData;
use anchor_lang::{prelude::*, Event};
use base64;
use bridge_cards::{
    accounts::AddOrUpdateMerchantDebitor, events::MerchantDebitorAddedOrUpdated,
    state::MerchantDebitorState,
};
use solana_program_test::tokio;
use solana_sdk::signature::Signer;

const TEST_MERCHANT_ID: u64 = 1u64;

#[tokio::test]
async fn test_add_merchant_debitor() {
    // Step 1: Create the context and initialize bridge cards
    let mut ctx = setup_and_initialize();

    // Step 2: Create a token mint and accounts
    let (_, debitor_pk) = setup_keypair(&mut ctx);
    let mint_pk = setup_mint(&mut ctx);

    // Step 3: Create the merchant account
    let debitor_pda =
        make_merchant_debitor_pda(TEST_MERCHANT_ID, &debitor_pk, &mint_pk, &ctx.program_id);

    let accounts = AddOrUpdateMerchantDebitor {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        debitor: debitor_pk,
        debitor_state: debitor_pda.pubkey,
        mint: mint_pk,
        system_program: System::id(),
    };

    let ix =
        create_add_or_update_merchant_debitor_instruction(&ctx, &accounts, TEST_MERCHANT_ID, true);
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
        "Failed to create merchant: {:?}",
        result.err()
    );

    // Verify the MerchantDebitorAddedOrUpdated event
    let meta = result.unwrap();
    let mut event_found = false;
    for log in meta.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) =
                        MerchantDebitorAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.merchant_id, TEST_MERCHANT_ID);
                        assert_eq!(parsed_event.debitor, debitor_pk);
                        assert_eq!(parsed_event.state_pda, debitor_pda.pubkey);
                        assert!(parsed_event.new_state);
                        event_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event_found,
        "MerchantDebitorAddedOrUpdated event not found for creation: {}",
        meta.logs.join("\n")
    );

    // Step 5: Verify the merchant state
    let expected_merchant_data = MerchantDebitorState {
        allowed: true,
        bump: debitor_pda.bump,
    }
    .account_data();

    let merchant_account = ctx.svm.get_account(&debitor_pda.pubkey).unwrap();
    assert_eq!(
        merchant_account.data, expected_merchant_data,
        "Merchant account data doesn't match expected data"
    );
}

#[tokio::test]
async fn test_add_second_debitor() {
    // Step 1: Create the context, initialize bridge cards, and create a merchant
    let mut ctx = setup_and_initialize();

    // Step 2: Create a token mint and accounts
    let (_, debitor_pk) = setup_keypair(&mut ctx);
    let mint_pk = setup_mint(&mut ctx);

    // First create the merchant
    let debitor_pda =
        make_merchant_debitor_pda(TEST_MERCHANT_ID, &debitor_pk, &mint_pk, &ctx.program_id);

    let accounts = AddOrUpdateMerchantDebitor {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        debitor: debitor_pk,
        debitor_state: debitor_pda.pubkey,
        mint: mint_pk,
        system_program: System::id(),
    };

    let ix =
        create_add_or_update_merchant_debitor_instruction(&ctx, &accounts, TEST_MERCHANT_ID, true);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );

    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_ok(),
        "Failed to create merchant: {:?}",
        result.err()
    );

    // Verify the event for the first creation (optional, could be removed)
    let meta1 = result.unwrap();
    let mut event1_found = false;
    for log in meta1.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if MerchantDebitorAddedOrUpdated::try_from_slice(event_data).is_ok() {
                        event1_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event1_found,
        "MerchantDebitorAddedOrUpdated event not found for first creation: {}",
        meta1.logs.join("\n")
    );

    // Step 3: Now update the merchant with new debitor
    let (_, new_debitor_pk) = setup_keypair(&mut ctx);
    let new_mint_pk = setup_mint(&mut ctx);
    let new_debitor_pda = make_merchant_debitor_pda(
        TEST_MERCHANT_ID,
        &new_debitor_pk,
        &new_mint_pk,
        &ctx.program_id,
    );
    let update_accounts = AddOrUpdateMerchantDebitor {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        debitor: new_debitor_pk,
        debitor_state: new_debitor_pda.pubkey,
        mint: new_mint_pk,
        system_program: System::id(),
    };

    let update_ix = create_add_or_update_merchant_debitor_instruction(
        &ctx,
        &update_accounts,
        TEST_MERCHANT_ID,
        true,
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
        "Failed to update merchant: {:?}",
        update_result.err()
    );

    // Verify the MerchantDebitorAddedOrUpdated event for the second debitor
    let meta2 = update_result.unwrap();
    let mut event2_found = false;
    for log in meta2.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) =
                        MerchantDebitorAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.merchant_id, TEST_MERCHANT_ID);
                        assert_eq!(parsed_event.debitor, new_debitor_pk);
                        assert_eq!(parsed_event.state_pda, new_debitor_pda.pubkey);
                        assert!(parsed_event.new_state);
                        event2_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event2_found,
        "MerchantDebitorAddedOrUpdated event not found for second debitor: {}",
        meta2.logs.join("\n")
    );

    // Step 5: Verify the merchant state has been updated
    let expected_updated_merchant_data = MerchantDebitorState {
        allowed: true,
        bump: debitor_pda.bump,
    }
    .account_data();

    let merchant_account = ctx.svm.get_account(&debitor_pda.pubkey).unwrap();
    assert_eq!(
        merchant_account.data, expected_updated_merchant_data,
        "Second debitor account data doesn't match expected data"
    );
}

#[tokio::test]
async fn test_update_debitor_to_false_then_back() {
    let mut ctx = setup_and_initialize();

    let (_, debitor_pk) = setup_keypair(&mut ctx);
    let mint_pk = setup_mint(&mut ctx);

    let debitor_pda =
        make_merchant_debitor_pda(TEST_MERCHANT_ID, &debitor_pk, &mint_pk, &ctx.program_id);

    let accounts = AddOrUpdateMerchantDebitor {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        debitor: debitor_pk,
        debitor_state: debitor_pda.pubkey,
        mint: mint_pk,
        system_program: System::id(),
    };

    let ix =
        create_add_or_update_merchant_debitor_instruction(&ctx, &accounts, TEST_MERCHANT_ID, true);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );

    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_ok(),
        "Failed to create merchant: {:?}",
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
                    if MerchantDebitorAddedOrUpdated::try_from_slice(event_data).is_ok() {
                        event1_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event1_found,
        "MerchantDebitorAddedOrUpdated event not found for initial creation: {}",
        meta1.logs.join("\n")
    );

    let expected_merchant_data = MerchantDebitorState {
        allowed: true,
        bump: debitor_pda.bump,
    }
    .account_data();
    let merchant_account = ctx.svm.get_account(&debitor_pda.pubkey).unwrap();
    assert_eq!(
        merchant_account.data, expected_merchant_data,
        "Merchant account data doesn't match expected data"
    );

    // now do the same thing but with false

    let ix =
        create_add_or_update_merchant_debitor_instruction(&ctx, &accounts, TEST_MERCHANT_ID, false);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );

    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_ok(),
        "Failed to update merchant: {:?}",
        result.err()
    );

    // Verify the MerchantDebitorAddedOrUpdated event for the update to false
    let meta2 = result.unwrap();
    let mut event2_found = false;
    for log in meta2.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) =
                        MerchantDebitorAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.merchant_id, TEST_MERCHANT_ID);
                        assert_eq!(parsed_event.debitor, debitor_pk);
                        assert_eq!(parsed_event.state_pda, debitor_pda.pubkey);
                        assert!(parsed_event.previous_state);
                        assert!(!parsed_event.new_state);
                        event2_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event2_found,
        "MerchantDebitorAddedOrUpdated event not found for update to false: {}",
        meta2.logs.join("\n")
    );

    let expected_merchant_data = MerchantDebitorState {
        allowed: false,
        bump: debitor_pda.bump,
    }
    .account_data();
    let merchant_account = ctx.svm.get_account(&debitor_pda.pubkey).unwrap();
    assert_eq!(
        merchant_account.data, expected_merchant_data,
        "Merchant account data doesn't match expected data"
    );
}

#[tokio::test]
async fn test_non_manager_cannot_add_debitor() {
    // Step 1: Create the context and initialize bridge cards
    let mut ctx = setup_and_initialize();

    // Step 2: Create a non-manager signer
    let (non_manager_kp, non_manager_pk) = setup_keypair(&mut ctx);

    // Step 3: Create a token mint and accounts
    let (_, debitor_pk) = setup_keypair(&mut ctx);
    let mint_pk = setup_mint(&mut ctx);

    // Try to create the merchant with non-manager
    let debitor_pda =
        make_merchant_debitor_pda(TEST_MERCHANT_ID, &debitor_pk, &mint_pk, &ctx.program_id);

    let accounts = AddOrUpdateMerchantDebitor {
        manager: non_manager_pk, // Non-manager tries to act as manager
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: non_manager_pk, // Non-manager is the payer
        debitor: debitor_pk,
        debitor_state: debitor_pda.pubkey,
        mint: mint_pk,
        system_program: System::id(),
    };

    let ix =
        create_add_or_update_merchant_debitor_instruction(&ctx, &accounts, TEST_MERCHANT_ID, true);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&non_manager_pk),
        &[&non_manager_kp],
    );

    // Step 4: Process the transaction - should fail due to constraint violation
    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_err(),
        "Non-manager should not be able to create merchant"
    );

    // Verify the error type (constraint error)
    let err = result.err().unwrap();
    println!("Error: {:?}", err);
    // The error should match the constraint violation for manager check
    assert!(
        err.err.to_string().contains("custom program error"),
        "Error doesn't match expected constraint violation"
    );
}
