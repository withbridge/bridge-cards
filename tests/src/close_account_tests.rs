use crate::common::*;
use anchor_lang::prelude::*;
use anchor_lang::{error::ErrorCode, system_program};
use base64;
use bridge_cards::{
    accounts::CloseAccount, errors::ErrorCode as BridgeErrorCode, events::AccountClosed,
    instructions::add_or_update_merchant_debitor::MERCHANT_DEBITOR_SEED,
};
use solana_account::ReadableAccount;
use solana_program_test::tokio;
use solana_sdk::signature::Signer;

const TEST_MERCHANT_ID: u64 = 1;

#[tokio::test]
async fn test_close_account_success() {
    // Step 1: Setup the test environment and create an account to close
    let mut ctx = setup_and_initialize();

    // Create a merchant debitor account to close
    let (debitor_kp, debitor_pk) = setup_keypair(&mut ctx);
    let mint_pk = setup_mint(&mut ctx);
    let debitor_pda =
        make_merchant_debitor_pda(TEST_MERCHANT_ID, &debitor_pk, &mint_pk, &ctx.program_id);

    // First, create the merchant debitor account
    let debitor_accounts = bridge_cards::accounts::AddOrUpdateMerchantDebitor {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        debitor: debitor_pk,
        debitor_state: debitor_pda.pubkey,
        mint: mint_pk,
        system_program: anchor_lang::system_program::ID,
    };
    let ix = create_add_or_update_merchant_debitor_instruction(
        &ctx,
        &debitor_accounts,
        TEST_MERCHANT_ID,
        true,
    );
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );
    submit_transaction(&mut ctx, tx).unwrap();

    // Verify the account exists and has lamports
    let account_before = ctx.svm.get_account(&debitor_pda.pubkey).unwrap();
    assert!(account_before.lamports > 0, "Account should have lamports");

    // Step 2: Create the close account transaction
    let close_accounts = CloseAccount {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        account_to_close: debitor_pda.pubkey,
        state: ctx.bridge_cards_state.pubkey,
    };

    // Prepare the seeds for the debitor PDA
    let input_seeds = vec![
        MERCHANT_DEBITOR_SEED.to_vec(),
        TEST_MERCHANT_ID.to_le_bytes().to_vec(),
        mint_pk.to_bytes().to_vec(),
        debitor_pk.to_bytes().to_vec(),
    ];

    let ix = create_close_account_instruction(&ctx, &close_accounts, input_seeds);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );

    // Step 3: Process the transaction
    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_ok(),
        "Failed to close account: {:?}",
        result.err()
    );

    // Verify the AccountClosed event
    let meta = result.unwrap(); // Get the TransactionMetadata directly

    let mut event_found = false;
    for log in meta.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..]; // Skip the 8-byte discriminator
                    if let Ok(parsed_event) = AccountClosed::try_from_slice(event_data) {
                        // Assert specific event fields if needed
                        assert_eq!(
                            parsed_event.account, debitor_pda.pubkey,
                            "Closed account does not match"
                        );
                        event_found = true;
                        break;
                    }
                }
            }
        }
    }

    assert!(
        event_found,
        "AccountClosed event not found in logs: {}",
        meta.logs.join("\n")
    );

    // Step 4: Verify the account is closed (should return None or have 0 lamports)
    match ctx.svm.get_account(&debitor_pda.pubkey) {
        Some(account) => {
            assert_eq!(account.lamports, 0, "Account should have 0 lamports");
            assert_eq!(account.data().len(), 0, "Account data should be empty");
            assert_eq!(
                account.owner,
                system_program::ID,
                "Account owner should be system program"
            );
        }
        None => {
            // It's also fine if the account is completely removed
        }
    }
}

#[tokio::test]
async fn test_close_account_not_admin() {
    // Step 1: Setup the test environment and create an account to close
    let mut ctx = setup_and_initialize();

    // Create a merchant debitor account to close
    let (debitor_kp, debitor_pk) = setup_keypair(&mut ctx);
    let mint_pk = setup_mint(&mut ctx);
    let debitor_pda =
        make_merchant_debitor_pda(TEST_MERCHANT_ID, &debitor_pk, &mint_pk, &ctx.program_id);

    // First, create the merchant debitor account
    let debitor_accounts = bridge_cards::accounts::AddOrUpdateMerchantDebitor {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        debitor: debitor_pk,
        debitor_state: debitor_pda.pubkey,
        mint: mint_pk,
        system_program: anchor_lang::system_program::ID,
    };
    let ix = create_add_or_update_merchant_debitor_instruction(
        &ctx,
        &debitor_accounts,
        TEST_MERCHANT_ID,
        true,
    );
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );
    submit_transaction(&mut ctx, tx).unwrap();

    // Create a non-admin user
    let (not_admin_kp, not_admin_pk) = setup_keypair(&mut ctx);

    // Create the close account transaction with a non-admin user
    let close_accounts = CloseAccount {
        admin: not_admin_pk,
        payer: not_admin_pk,
        account_to_close: debitor_pda.pubkey,
        state: ctx.bridge_cards_state.pubkey,
    };

    // Prepare the seeds for the debitor PDA
    let input_seeds = vec![
        MERCHANT_DEBITOR_SEED.to_vec(),
        TEST_MERCHANT_ID.to_le_bytes().to_vec(),
        debitor_pk.to_bytes().to_vec(),
    ];

    let ix = create_close_account_instruction(&ctx, &close_accounts, input_seeds);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&not_admin_pk),
        &[&not_admin_kp],
    );

    // Process the transaction, which should fail
    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_err(),
        "Non-admin should not be able to close an account"
    );

    // Verify the correct error
    let err = result.err().unwrap();
    let expected_message = ErrorCode::ConstraintRaw.to_string();
    let expected_message2 = "caused by account: admin";
    assert!(
        err.meta
            .logs
            .iter()
            .any(|log| log.contains(&expected_message) && log.contains(expected_message2)),
        "Error should contain the expected error message {}, got {}",
        expected_message,
        err.meta.logs.join(", ")
    );
}

#[tokio::test]
async fn test_close_account_invalid_pda() {
    // Step 1: Setup the test environment and create an account to close
    let mut ctx = setup_and_initialize();

    // Create a merchant debitor account to close
    let (debitor_kp, debitor_pk) = setup_keypair(&mut ctx);
    let mint_pk = setup_mint(&mut ctx);
    let debitor_pda =
        make_merchant_debitor_pda(TEST_MERCHANT_ID, &debitor_pk, &mint_pk, &ctx.program_id);

    // First, create the merchant debitor account
    let debitor_accounts = bridge_cards::accounts::AddOrUpdateMerchantDebitor {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        debitor: debitor_pk,
        debitor_state: debitor_pda.pubkey,
        mint: mint_pk,
        system_program: anchor_lang::system_program::ID,
    };
    let ix = create_add_or_update_merchant_debitor_instruction(
        &ctx,
        &debitor_accounts,
        TEST_MERCHANT_ID,
        true,
    );
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );
    submit_transaction(&mut ctx, tx).unwrap();

    // Create the close account transaction with correct accounts but wrong seeds
    let close_accounts = CloseAccount {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        account_to_close: debitor_pda.pubkey,
        state: ctx.bridge_cards_state.pubkey,
    };

    // Prepare incorrect seeds (using wrong merchant ID)
    let wrong_merchant_id = 54321u64; // Different from the actual merchant ID
    let input_seeds = vec![
        MERCHANT_DEBITOR_SEED.to_vec(),
        wrong_merchant_id.to_le_bytes().to_vec(), // Wrong merchant ID
        debitor_pk.to_bytes().to_vec(),
    ];

    let ix = create_close_account_instruction(&ctx, &close_accounts, input_seeds);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );

    // Process the transaction, which should fail
    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_err(),
        "Should not be able to close with invalid PDA seeds"
    );

    // Verify the correct error is returned
    let err = result.err().unwrap();
    let expected_error = BridgeErrorCode::InvalidPda.to_string();
    assert!(
        err.meta
            .logs
            .iter()
            .any(|log| log.contains(&expected_error)),
        "Error should contain InvalidPda, got {}",
        err.meta.logs.join(", ")
    );
}
