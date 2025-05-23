use crate::common::*;
use account_data_trait::AccountData;
use anchor_lang::error::ErrorCode;
use anchor_lang::{prelude::*, Event};
use base64;
use bridge_cards::{accounts::UpdateAdmin, events::AdminUpdated, state::BridgeCardsState};
use solana_program_test::tokio;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;

#[tokio::test]
async fn test_update_admin() {
    // step 1: create the context
    let mut ctx = setup_and_initialize();
    let new_admin = Keypair::new();
    let new_admin_pk = new_admin.try_pubkey().unwrap();

    // step 2: create the transaction
    let accounts = UpdateAdmin {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        new_admin: new_admin_pk,
    };
    let ix = create_update_admin_instruction(&ctx, accounts);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &new_admin],
    );

    // step 3: Process the transaction
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_ok(), "Failed to update admin: {:?}", result.err());

    // Verify the AdminUpdated event
    let meta = result.unwrap();

    let mut event_found = false;
    for log in meta.logs.iter() {
        if let Some(data_str) = log.strip_prefix("Program data: ") {
            if let Ok(log_bytes) = base64::decode(data_str) {
                if log_bytes.len() > 8 {
                    let event_data = &log_bytes[8..];
                    if let Ok(parsed_event) = AdminUpdated::try_from_slice(event_data) {
                        assert_eq!(
                            parsed_event.admin, new_admin_pk,
                            "Updated admin does not match"
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
        "AdminUpdated event not found in logs: {}",
        meta.logs.join("\n")
    );

    // step 4: verify the state
    let expected_state_data = BridgeCardsState {
        admin: new_admin_pk,
        bump: ctx.bridge_cards_state.bump,
    }
    .account_data();

    assert_eq!(
        ctx.svm
            .get_account(&ctx.bridge_cards_state.pubkey)
            .unwrap()
            .data,
        expected_state_data
    );
}

#[tokio::test]
async fn test_update_admin_not_admin() {
    let mut ctx = setup_and_initialize();
    let not_admin = Keypair::new();
    let not_admin_pk = not_admin.try_pubkey().unwrap();
    let new_admin = Keypair::new();
    let new_admin_pk = new_admin.try_pubkey().unwrap();
    ctx.svm.airdrop(&not_admin_pk, 1000000000).unwrap();

    let accounts = UpdateAdmin {
        admin: not_admin_pk,
        payer: not_admin_pk,
        state: ctx.bridge_cards_state.pubkey,
        new_admin: new_admin_pk,
    };
    let ix = create_update_admin_instruction(&ctx, accounts);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&not_admin_pk),
        &[&not_admin, &new_admin],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_err(), "Should not be able to update admin");
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
