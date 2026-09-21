use crate::common::*;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_lang::prelude::*;
use bridge_cards::errors::ErrorCode;
use tokio;
use solana_sdk::signature::{Keypair, Signer};

type TestContext = crate::common::Context;

fn pause_ix(
    ctx: &TestContext,
    spender_state: Pubkey,
    signer_pk: Pubkey,
    paused: bool,
) -> solana_sdk::instruction::Instruction {
    let accounts = bridge_cards::accounts::UpdatePaused {
        signer: signer_pk,
        spender_state,
    };
    solana_sdk::instruction::Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: bridge_cards::instruction::UpdatePaused { paused }.data(),
    }
}

#[tokio::test]
async fn test_admin_can_pause_and_unpause() {
    let mut ctx = setup_and_initialize();
    let spender_ctx = setup_spender_state(&mut ctx);

    // Admin pauses
    let ix = pause_ix(&ctx, spender_ctx.spender_state.pubkey, ctx.payer_pk, true);
    let tx = create_transaction(&ctx, &[ix]);
    assert!(submit_transaction(&mut ctx, tx).is_ok());

    // Admin unpauses
    let ix = pause_ix(&ctx, spender_ctx.spender_state.pubkey, ctx.payer_pk, false);
    let tx = create_transaction(&ctx, &[ix]);
    assert!(submit_transaction(&mut ctx, tx).is_ok());
}

#[tokio::test]
async fn test_pauser_can_pause_but_not_unpause() {
    let mut ctx = setup_and_initialize();
    let spender_ctx = setup_spender_state(&mut ctx);
    // In test setup payer == admin == governor == manager == pauser
    // We need a separate pauser key. Set one via update_pauser.
    let (pauser_kp, pauser_pk) = setup_keypair(&mut ctx);

    // update_pauser: admin sets pauser
    let update_accounts = bridge_cards::accounts::UpdatePauser {
        admin: ctx.payer_pk,
        spender_state: spender_ctx.spender_state.pubkey,
        pauser: pauser_pk,
    };
    let update_ix = solana_sdk::instruction::Instruction {
        program_id: ctx.program_id,
        accounts: update_accounts.to_account_metas(None),
        data: bridge_cards::instruction::UpdatePauser {}.data(),
    };
    let tx = create_transaction(&ctx, &[update_ix]);
    submit_transaction(&mut ctx, tx).unwrap();

    // Pauser can pause
    let ix = pause_ix(&ctx, spender_ctx.spender_state.pubkey, pauser_pk, true);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &pauser_kp],
    );
    assert!(submit_transaction(&mut ctx, tx).is_ok(), "pauser should be able to pause");

    // Pauser cannot unpause
    let ix = pause_ix(&ctx, spender_ctx.spender_state.pubkey, pauser_pk, false);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &pauser_kp],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_err(), "pauser should NOT be able to unpause");
    let err_str = format!("{:?}", result.err());
    assert!(
        err_str.contains(&(ErrorCode::Unauthorized as u32).to_string()),
        "expected Unauthorized, got: {err_str}"
    );
}

#[tokio::test]
async fn test_unauthorized_cannot_pause() {
    let mut ctx = setup_and_initialize();
    let spender_ctx = setup_spender_state(&mut ctx);
    let (stranger_kp, stranger_pk) = setup_keypair(&mut ctx);

    let ix = pause_ix(&ctx, spender_ctx.spender_state.pubkey, stranger_pk, true);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &stranger_kp],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_err(), "stranger should not be able to pause");
}
