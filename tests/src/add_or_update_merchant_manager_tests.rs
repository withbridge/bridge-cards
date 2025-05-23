use crate::common::*;
use account_data_trait::AccountData;
use anchor_lang::{prelude::*, Event};
use base64;
use bridge_cards::{
    events::MerchantManagerAddedOrUpdated,
    instructions::add_or_update_merchant_manager::MERCHANT_MANAGER_SEED,
    state::MerchantManagerState,
};
use solana_program_test::tokio;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

#[tokio::test]
async fn test_add_merchant_manager_success() {
    let mut ctx = setup();
    initialize_bridge_cards(&mut ctx);

    let merchant_id = 42u64;
    let manager = Keypair::new();

    // Derive manager state PDA
    let manager_state = make_manager_pda(merchant_id, &ctx.program_id);

    // Build transaction
    let accounts = bridge_cards::accounts::AddOrUpdateMerchantManager {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        manager_state: manager_state.pubkey,
        manager: manager.pubkey(),
        system_program: anchor_lang::system_program::ID,
    };

    let ix = create_add_or_update_merchant_manager_instruction(&ctx, &accounts, merchant_id);
    let tx = create_transaction(&ctx, &[ix]);
    let result = submit_transaction(&mut ctx, tx);

    assert!(result.is_ok(), "Failed to add merchant manager");

    // Verify manager state was created and updated correctly
    let manager_state_account = ctx.svm.get_account(&manager_state.pubkey).unwrap();
    let expected_manager_data = MerchantManagerState {
        manager: manager.pubkey(),
        bump: manager_state.bump,
    }
    .account_data();

    assert_eq!(
        manager_state_account.data, expected_manager_data,
        "Manager state data doesn't match expected data"
    );

    // Verify the MerchantManagerAddedOrUpdated event
    let meta = result.unwrap();
    let mut event_found = false;
    for log in meta.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) =
                        MerchantManagerAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.merchant_id, merchant_id);
                        assert_eq!(parsed_event.manager, manager.pubkey());
                        event_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event_found,
        "MerchantManagerAddedOrUpdated event not found in logs: {}",
        meta.logs.join("\n")
    );
}

#[tokio::test]
async fn test_update_existing_manager() {
    let mut ctx = setup();
    initialize_bridge_cards(&mut ctx);

    let merchant_id = 42u64;
    let old_manager = Keypair::new();
    let new_manager = Keypair::new();

    // First add initial manager
    let manager_state = make_manager_pda(merchant_id, &ctx.program_id);

    let accounts = bridge_cards::accounts::AddOrUpdateMerchantManager {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        manager_state: manager_state.pubkey,
        manager: old_manager.pubkey(),
        system_program: anchor_lang::system_program::ID,
    };

    let ix = create_add_or_update_merchant_manager_instruction(&ctx, &accounts, merchant_id);
    let tx = create_transaction(&ctx, &[ix]);
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_ok(), "Failed to add initial manager");

    // Verify first event
    let meta1 = result.unwrap();
    let mut event1_found = false;
    for log in meta1.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) =
                        MerchantManagerAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.merchant_id, merchant_id);
                        assert_eq!(parsed_event.manager, old_manager.pubkey());
                        event1_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event1_found,
        "First MerchantManagerAddedOrUpdated event not found in logs: {}",
        meta1.logs.join("\n")
    );

    // Now update to new manager
    let accounts = bridge_cards::accounts::AddOrUpdateMerchantManager {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        manager_state: manager_state.pubkey,
        manager: new_manager.pubkey(),
        system_program: anchor_lang::system_program::ID,
    };

    let ix = create_add_or_update_merchant_manager_instruction(&ctx, &accounts, merchant_id);
    let tx = create_transaction(&ctx, &[ix]);
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_ok(), "Failed to update manager");

    // Verify manager was updated
    let manager_state_account = ctx.svm.get_account(&manager_state.pubkey).unwrap();
    let expected_manager_data = MerchantManagerState {
        manager: new_manager.pubkey(),
        bump: manager_state.bump,
    }
    .account_data();

    assert_eq!(
        manager_state_account.data, expected_manager_data,
        "Manager not updated correctly"
    );

    // Verify second event
    let meta2 = result.unwrap();
    let mut event2_found = false;
    for log in meta2.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) =
                        MerchantManagerAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.merchant_id, merchant_id);
                        assert_eq!(parsed_event.manager, new_manager.pubkey());
                        event2_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event2_found,
        "Second MerchantManagerAddedOrUpdated event not found in logs: {}",
        meta2.logs.join("\n")
    );
}

#[tokio::test]
async fn test_non_admin_cannot_add_manager() {
    let mut ctx = setup();
    initialize_bridge_cards(&mut ctx);

    let merchant_id = 42u64;
    let non_admin = Keypair::new();
    let manager = Keypair::new();

    let (manager_state, _) = Pubkey::find_program_address(
        &[MERCHANT_MANAGER_SEED, &merchant_id.to_le_bytes()],
        &ctx.program_id,
    );

    let accounts = bridge_cards::accounts::AddOrUpdateMerchantManager {
        admin: non_admin.pubkey(), // Try with non-admin
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        manager_state,
        manager: manager.pubkey(),
        system_program: anchor_lang::system_program::ID,
    };

    let ix = create_add_or_update_merchant_manager_instruction(&ctx, &accounts, merchant_id);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &non_admin],
    );
    let result = submit_transaction(&mut ctx, tx);

    assert!(
        result.is_err(),
        "Non-admin should not be able to add manager"
    );
}

#[tokio::test]
async fn test_manager_signature_not_required() {
    let mut ctx = setup();
    initialize_bridge_cards(&mut ctx);

    let merchant_id = 42u64;
    let manager = Keypair::new();

    let (manager_state, _) = Pubkey::find_program_address(
        &[MERCHANT_MANAGER_SEED, &merchant_id.to_le_bytes()],
        &ctx.program_id,
    );

    let accounts = bridge_cards::accounts::AddOrUpdateMerchantManager {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        manager_state,
        manager: manager.pubkey(), // Manager doesn't sign
        system_program: anchor_lang::system_program::ID,
    };

    let ix = create_add_or_update_merchant_manager_instruction(&ctx, &accounts, merchant_id);
    let tx = create_transaction(&ctx, &[ix]);
    let result = submit_transaction(&mut ctx, tx);

    assert!(result.is_ok(), "Should succeed without manager signature");

    // Verify event was emitted
    let meta = result.unwrap();
    let mut event_found = false;
    for log in meta.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) =
                        MerchantManagerAddedOrUpdated::try_from_slice(event_data)
                    {
                        assert_eq!(parsed_event.merchant_id, merchant_id);
                        assert_eq!(parsed_event.manager, manager.pubkey());
                        event_found = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(
        event_found,
        "MerchantManagerAddedOrUpdated event not found in logs: {}",
        meta.logs.join("\n")
    );
}
