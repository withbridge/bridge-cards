use crate::common::*;
use account_data_trait::AccountData;
use anchor_lang::{prelude::*, Event};
use base64;
use bridge_cards::{
    accounts::AddOrUpdateMerchantDestination, events::MerchantDestinationAddedOrUpdated,
    state::MerchantDestinationState,
};
use litesvm_token::CreateAssociatedTokenAccountIdempotent;

use solana_program_test::tokio;

#[tokio::test]
async fn test_create_merchant_destination() {
    // Step 1: Create the context and initialize bridge cards
    let mut ctx = setup_and_initialize();

    // Step 2: Create a token mint and accounts
    let (_, mint_pk) = setup_mint(&mut ctx);
    let merchant_id = 1u64;
    let (_, destination_owner_pk) = setup_keypair(&mut ctx);

    // Create token accounts for the destination
    // todo: what if this doesn't exist?
    let destination_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&destination_owner_pk)
            .send()
            .unwrap();

    let destination_token_account_key = destination_token_account.key();

    // Step 3: Create the merchant account
    let merchant_destination_pda = make_merchant_destination_pda(
        merchant_id,
        &mint_pk,
        &destination_token_account_key,
        &ctx.program_id,
    );

    let accounts = AddOrUpdateMerchantDestination {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        destination_state: merchant_destination_pda.pubkey,
        destination_token_account,
        mint: mint_pk,
        system_program: System::id(),
    };

    let ix =
        create_add_or_update_merchant_destination_instruction(&ctx, &accounts, merchant_id, true);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );

    // Step 4: Process the transaction
    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_ok(),
        "Failed to create merchant: {:?}",
        result.err()
    );

    // Verify the MerchantDestinationAddedOrUpdated event
    let meta = result.unwrap();
    let mut event_found = false;
    for log in meta.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) =
                        MerchantDestinationAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.merchant_id, merchant_id);
                        assert_eq!(parsed_event.mint, mint_pk);
                        assert_eq!(parsed_event.destination, destination_token_account_key);
                        assert_eq!(parsed_event.state_pda, merchant_destination_pda.pubkey);
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
        "MerchantDestinationAddedOrUpdated event not found for creation: {}",
        meta.logs.join("\n")
    );

    // Step 5: Verify the merchant state
    let expected_merchant_data = MerchantDestinationState {
        allowed: true,
        bump: merchant_destination_pda.bump,
    }
    .account_data();

    let merchant_account = ctx
        .svm
        .get_account(&merchant_destination_pda.pubkey)
        .unwrap();
    assert_eq!(
        merchant_account.data, expected_merchant_data,
        "Merchant account data doesn't match expected data"
    );
}

#[tokio::test]
async fn test_update_merchant_destination() {
    // Step 1: Create the context, initialize bridge cards, and create a merchant
    let mut ctx = setup_and_initialize();

    // Step 2: Create a token mint and accounts
    let (_, mint_pk) = setup_mint(&mut ctx);
    let merchant_id = 2u64;
    let (_, destination_pk) = setup_keypair(&mut ctx);

    // Create token accounts for the destination
    let destination_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&destination_pk)
            .send()
            .unwrap();

    let destination_token_account_key = destination_token_account.key();

    // First create the merchant
    let merchant_destination_pda = make_merchant_destination_pda(
        merchant_id,
        &mint_pk,
        &destination_token_account_key,
        &ctx.program_id,
    );

    let accounts = AddOrUpdateMerchantDestination {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        destination_state: merchant_destination_pda.pubkey,
        destination_token_account,
        mint: mint_pk,
        system_program: System::id(),
    };

    let ix =
        create_add_or_update_merchant_destination_instruction(&ctx, &accounts, merchant_id, true);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );

    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_ok(),
        "Failed to create merchant: {:?}",
        result.err()
    );

    // Verify the event for the initial creation
    let meta1 = result.unwrap();
    let mut event1_found = false;
    for log in meta1.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if MerchantDestinationAddedOrUpdated::try_from_slice(event_data).is_ok() {
                        event1_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event1_found,
        "MerchantDestinationAddedOrUpdated event not found for initial creation: {}",
        meta1.logs.join("\n")
    );

    let update_accounts = AddOrUpdateMerchantDestination {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        destination_state: merchant_destination_pda.pubkey,
        mint: mint_pk,
        destination_token_account,
        system_program: System::id(),
    };

    let update_ix = create_add_or_update_merchant_destination_instruction(
        &ctx,
        &update_accounts,
        merchant_id,
        false,
    );
    let update_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[update_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );

    // Step 4: Process the update transaction
    let update_result = submit_transaction(&mut ctx, update_tx);
    assert!(
        update_result.is_ok(),
        "Failed to update merchant: {:?}",
        update_result.err()
    );

    // Verify the event for the update
    let meta2 = update_result.unwrap();
    let mut event2_found = false;
    for log in meta2.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) =
                        MerchantDestinationAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.merchant_id, merchant_id);
                        assert_eq!(parsed_event.mint, mint_pk);
                        assert_eq!(parsed_event.destination, destination_token_account_key);
                        assert_eq!(parsed_event.state_pda, merchant_destination_pda.pubkey);
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
        "MerchantDestinationAddedOrUpdated event not found for update: {}",
        meta2.logs.join("\n")
    );

    // Step 5: Verify the merchant state has been updated
    let expected_updated_merchant_data = MerchantDestinationState {
        allowed: false,
        bump: merchant_destination_pda.bump,
    }
    .account_data();

    let merchant_account = ctx
        .svm
        .get_account(&merchant_destination_pda.pubkey)
        .unwrap();
    assert_eq!(
        merchant_account.data, expected_updated_merchant_data,
        "Updated merchant account data doesn't match expected data"
    );
}

#[tokio::test]
async fn test_non_admin_cannot_create_merchant() {
    // Step 1: Create the context and initialize bridge cards
    let mut ctx = setup_and_initialize();

    // Step 2: Create a non-admin signer
    let (non_admin_kp, non_admin_pk) = setup_keypair(&mut ctx);

    // Step 3: Create a token mint and accounts
    let (_, mint_pk) = setup_mint(&mut ctx);
    let merchant_id = 3u64;
    let (_, destination_owner_pk) = setup_keypair(&mut ctx);

    // Create token accounts for the destination
    let destination_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&destination_owner_pk)
            .send()
            .unwrap();

    // Try to create the merchant with non-admin
    let merchant_destination_pda = make_merchant_destination_pda(
        merchant_id,
        &mint_pk,
        &destination_token_account.key(),
        &ctx.program_id,
    );

    let accounts = AddOrUpdateMerchantDestination {
        admin: non_admin_pk, // Non-admin tries to act as admin
        payer: non_admin_pk, // Non-admin is the payer
        state: ctx.bridge_cards_state.pubkey,
        destination_state: merchant_destination_pda.pubkey,
        mint: mint_pk,
        destination_token_account,
        system_program: System::id(),
    };

    let ix =
        create_add_or_update_merchant_destination_instruction(&ctx, &accounts, merchant_id, true);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&non_admin_pk),
        &[&non_admin_kp],
    );

    // Step 4: Process the transaction - should fail due to constraint violation
    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_err(),
        "Non-admin should not be able to create merchant"
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
async fn test_two_merchant_destinations() {
    let mut ctx = setup_and_initialize();

    let merchant_id = 4u64;
    let (_, mint_pk) = setup_mint(&mut ctx);
    let (_, destination_pk) = setup_keypair(&mut ctx);

    // Create token accounts for the destination
    let destination_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&destination_pk)
            .send()
            .unwrap();

    let merchant_destination_pda = make_merchant_destination_pda(
        merchant_id,
        &mint_pk,
        &destination_token_account.key(),
        &ctx.program_id,
    );

    let accounts = AddOrUpdateMerchantDestination {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        destination_state: merchant_destination_pda.pubkey,
        mint: mint_pk,
        destination_token_account,
        system_program: System::id(),
    };

    let ix =
        create_add_or_update_merchant_destination_instruction(&ctx, &accounts, merchant_id, true);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );

    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_ok(),
        "Failed to create merchant: {:?}",
        result.err()
    );

    let (_, destination_pk2) = setup_keypair(&mut ctx);
    let destination_token_account2 =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&destination_pk2)
            .send()
            .unwrap();

    let merchant_destination_pda2 = make_merchant_destination_pda(
        merchant_id,
        &mint_pk,
        &destination_token_account2.key(),
        &ctx.program_id,
    );

    let accounts2 = AddOrUpdateMerchantDestination {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        destination_state: merchant_destination_pda2.pubkey,
        mint: mint_pk,
        destination_token_account: destination_token_account2,
        system_program: System::id(),
    };

    let ix2 =
        create_add_or_update_merchant_destination_instruction(&ctx, &accounts2, merchant_id, true);
    let tx2 = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix2],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );

    let result2 = submit_transaction(&mut ctx, tx2);
    assert!(
        result2.is_ok(),
        "Failed to create merchant: {:?}",
        result2.err()
    );

    let expected_merchant_data = MerchantDestinationState {
        allowed: true,
        bump: merchant_destination_pda2.bump,
    }
    .account_data();

    let merchant_account = ctx
        .svm
        .get_account(&merchant_destination_pda2.pubkey)
        .unwrap();
    assert_eq!(
        merchant_account.data, expected_merchant_data,
        "Merchant account data doesn't match expected data"
    );
    // check existing is still valid
    let merchant_account = ctx
        .svm
        .get_account(&merchant_destination_pda.pubkey)
        .unwrap();
    let expected_merchant_data = MerchantDestinationState {
        allowed: true,
        bump: merchant_destination_pda.bump,
    }
    .account_data();
    assert_eq!(
        merchant_account.data, expected_merchant_data,
        "Merchant account data doesn't match expected data"
    );
}
