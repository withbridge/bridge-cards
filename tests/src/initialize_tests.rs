use crate::common::*;
use account_data_trait::AccountData;
use bridge_cards::state::BridgeCardsState;
use solana_program_test::tokio;

#[tokio::test]
async fn test_initialize() {
    // step 1: create the context
    let mut ctx = setup();

    // step 2: create the transaction
    let ix = create_initialize_instruction(&ctx);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.extra_keypair],
    );
    // step 3: Process the transaction
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_ok(), "Failed to initialize: {:?}", result.err());

    // step 4: verify the state
    let expected_state_data = BridgeCardsState {
        admin: ctx.payer_pk,
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
async fn test_initialize_twice() {
    let mut ctx = setup();
    let ix = create_initialize_instruction(&ctx);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix.clone()],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.extra_keypair],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_ok(), "Failed to initialize: {:?}", result.err());
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix.clone()],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.extra_keypair],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_err(), "Should not be able to initialize twice");
    let err = result.err().unwrap();
    assert_eq!(
        err.err.to_string(),
        "Error processing Instruction 0: custom program error: 0x0"
    );
}
