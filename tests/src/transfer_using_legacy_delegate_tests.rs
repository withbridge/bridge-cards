use crate::common::*;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_lang::prelude::*;
use bridge_cards::errors::ErrorCode;
use litesvm_token::spl_token;
use litesvm_token::*;
use tokio;
use solana_sdk::signature::{Keypair, Signer};

type TestContext = crate::common::Context;

const TEST_MERCHANT_ID: u64 = 42;
const INITIAL_BALANCE: u64 = 5_000_000_000;
const TRANSFER_AMOUNT: u64 = 50_000_000;

fn merchant_id_bytes(id: u64) -> [u8; 32] {
    let mut b = [0u8; 32];
    b[..8].copy_from_slice(&id.to_le_bytes());
    b
}

struct LegacyDelegateCtx {
    spender_ctx: SpenderContext,
    mint_pk: Pubkey,
    user_kp: Keypair,
    user_ata: Pubkey,
    user_delegate_pda: Pubkey,
    merchant_delegate_pda: Pubkey,
    delegate_dest_pda: Pubkey,
    destination_ata: Pubkey,
}

fn setup_legacy_delegate_ctx(ctx: &mut TestContext) -> LegacyDelegateCtx {
    let spender_ctx = setup_spender_state(ctx);
    let mint_pk = setup_mint(ctx);

    let merchant_id_b32 = merchant_id_bytes(TEST_MERCHANT_ID);
    let (destination_owner_kp, destination_owner_pk) = setup_keypair(ctx);

    let (merchant_delegate_pda, delegate_dest_pda, destination_ata) =
        setup_legacy_merchant_delegate(
            ctx,
            &spender_ctx,
            &merchant_id_b32,
            TEST_MERCHANT_ID,
            &mint_pk,
            &destination_owner_pk,
            TokenProgram::Token,
        );

    // Create user + ATA
    let (user_kp, user_pk) = setup_keypair(ctx);
    let user_ata = CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
        .owner(&user_pk)
        .send()
        .unwrap();

    MintTo::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk, &user_ata, INITIAL_BALANCE)
        .send()
        .unwrap();

    let user_delegate_pda = make_user_delegate_pda(
        TEST_MERCHANT_ID,
        &mint_pk,
        &user_ata,
        &ctx.program_id,
    );

    // User approves the legacy delegate PDA
    ApproveChecked::new(&mut ctx.svm, &user_kp, &user_delegate_pda.pubkey, &mint_pk, 1e18 as u64)
        .send()
        .unwrap();

    LegacyDelegateCtx {
        spender_ctx,
        mint_pk,
        user_kp,
        user_ata,
        user_delegate_pda: user_delegate_pda.pubkey,
        merchant_delegate_pda,
        delegate_dest_pda,
        destination_ata,
    }
}

fn transfer_legacy_ix(
    ctx: &TestContext,
    lctx: &LegacyDelegateCtx,
    program_id: [u8; 32],
    merchant_id: u64,
    amount: u64,
) -> solana_sdk::instruction::Instruction {
    use bridge_cards::accounts::TransferUsingLegacyDelegate;

    let accounts = TransferUsingLegacyDelegate {
        debitor: lctx.spender_ctx.debitor_pk,
        spender_state: lctx.spender_ctx.spender_state.pubkey,
        payer: ctx.payer_pk,
        user_delegate_account: lctx.user_delegate_pda,
        merchant_delegate_state: lctx.merchant_delegate_pda,
        user_ata: lctx.user_ata,
        receiver_ata: lctx.destination_ata,
        delegate_destination_state: lctx.delegate_dest_pda,
        token_mint: lctx.mint_pk,
        token_program: spl_token::id(),
        system_program: anchor_lang::system_program::ID,
    };

    let ix_data = bridge_cards::instruction::TransferUsingLegacyDelegate {
        program_id,
        merchant_id,
        amount,
    }
    .data();

    solana_sdk::instruction::Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: ix_data,
    }
}

#[tokio::test]
async fn test_transfer_using_legacy_delegate_success() {
    let mut ctx = setup_and_initialize();
    let lctx = setup_legacy_delegate_ctx(&mut ctx);
    let program_id = merchant_id_bytes(TEST_MERCHANT_ID);

    let ix = transfer_legacy_ix(&ctx, &lctx, program_id, TEST_MERCHANT_ID, TRANSFER_AMOUNT);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &lctx.spender_ctx.debitor_kp],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_ok(), "transfer failed: {:?}", result.err());

    let dest = get_spl_account::<spl_token::state::Account>(&ctx.svm, &lctx.destination_ata)
        .unwrap();
    assert_eq!(dest.amount, TRANSFER_AMOUNT);
}

#[tokio::test]
async fn test_transfer_using_legacy_delegate_inits_user_delegate_if_missing() {
    // Verify init_if_needed: no add_or_update_user_delegate called, but transfer succeeds
    // (the user_delegate_pda will be created by the instruction itself)
    let mut ctx = setup_and_initialize();
    let lctx = setup_legacy_delegate_ctx(&mut ctx);
    let program_id = merchant_id_bytes(TEST_MERCHANT_ID);

    // Confirm the user delegate PDA does NOT exist yet
    assert!(ctx.svm.get_account(&lctx.user_delegate_pda).is_none());

    let ix = transfer_legacy_ix(&ctx, &lctx, program_id, TEST_MERCHANT_ID, TRANSFER_AMOUNT);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &lctx.spender_ctx.debitor_kp],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_ok(), "expected success on first call with uninited delegate: {:?}", result.err());

    // PDA now exists
    assert!(ctx.svm.get_account(&lctx.user_delegate_pda).is_some());
}

#[tokio::test]
async fn test_transfer_using_legacy_delegate_cross_merchant_attack_fails() {
    // Cross-merchant attack scenario:
    // - User approved user_delegate PDA for Merchant A (u64 id = 42)
    // - Merchant B is registered with a different program_id (u64 id = 99)
    // - Attacker calls with merchant_id=42 (user's approval) but program_id for Merchant B
    //   so that the destination comes from Merchant B's allowlist, not Merchant A's.
    // The binding check (legacy_merchant_id == merchant_id) should block this.

    let mut ctx = setup_and_initialize();
    let spender_ctx = setup_spender_state(&mut ctx);
    let mint_pk = setup_mint(&mut ctx);

    const MERCHANT_A_ID: u64 = 42;
    const MERCHANT_B_ID: u64 = 99;

    // Register Merchant A
    let (_, dest_owner_a) = setup_keypair(&mut ctx);
    let (merchant_a_delegate, _, dest_a_ata) = setup_legacy_merchant_delegate(
        &mut ctx, &spender_ctx,
        &merchant_id_bytes(MERCHANT_A_ID), MERCHANT_A_ID,
        &mint_pk, &dest_owner_a, TokenProgram::Token,
    );

    // Register Merchant B (different program_id, different legacy_merchant_id)
    let (_, dest_owner_b) = setup_keypair(&mut ctx);
    let (merchant_b_delegate, dest_b_state, dest_b_ata) = setup_legacy_merchant_delegate(
        &mut ctx, &spender_ctx,
        &merchant_id_bytes(MERCHANT_B_ID), MERCHANT_B_ID,
        &mint_pk, &dest_owner_b, TokenProgram::Token,
    );

    // User approves the Merchant A user_delegate PDA (derived from MERCHANT_A_ID)
    let (user_kp, user_pk) = setup_keypair(&mut ctx);
    let user_ata = litesvm_token::CreateAssociatedTokenAccountIdempotent::new(
        &mut ctx.svm, &ctx.payer_kp, &mint_pk,
    )
    .owner(&user_pk)
    .send()
    .unwrap();
    litesvm_token::MintTo::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk, &user_ata, INITIAL_BALANCE)
        .send()
        .unwrap();

    let user_delegate_pda = make_user_delegate_pda(MERCHANT_A_ID, &mint_pk, &user_ata, &ctx.program_id);
    litesvm_token::ApproveChecked::new(&mut ctx.svm, &user_kp, &user_delegate_pda.pubkey, &mint_pk, 1e18 as u64)
        .send()
        .unwrap();

    // Attack: supply merchant_id=MERCHANT_A_ID (matches the user's PDA) but
    // program_id=Merchant B's program_id (so merchant_delegate_state has legacy_merchant_id=99 ≠ 42)
    let attack_accounts = bridge_cards::accounts::TransferUsingLegacyDelegate {
        debitor: spender_ctx.debitor_pk,
        spender_state: spender_ctx.spender_state.pubkey,
        payer: ctx.payer_pk,
        user_delegate_account: user_delegate_pda.pubkey,
        merchant_delegate_state: merchant_b_delegate, // Merchant B's state (legacy_id=99)
        user_ata,
        receiver_ata: dest_b_ata,                     // Merchant B's destination
        delegate_destination_state: dest_b_state,
        token_mint: mint_pk,
        token_program: litesvm_token::spl_token::id(),
        system_program: anchor_lang::system_program::ID,
    };
    let ix_data = bridge_cards::instruction::TransferUsingLegacyDelegate {
        program_id: merchant_id_bytes(MERCHANT_B_ID), // Merchant B's program_id
        merchant_id: MERCHANT_A_ID,                   // Merchant A's u64 id (user's PDA)
        amount: TRANSFER_AMOUNT,
    }
    .data();
    let ix = solana_sdk::instruction::Instruction {
        program_id: ctx.program_id,
        accounts: attack_accounts.to_account_metas(None),
        data: ix_data,
    };
    let tx = create_transaction_with_payer_and_signers(
        &ctx, &[ix], Some(&ctx.payer_pk), &[&ctx.payer_kp, &spender_ctx.debitor_kp],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(
        result.is_err(),
        "cross-merchant attack should be blocked by LegacyMerchantIdMismatch"
    );
    let err_str = format!("{:?}", result.err());
    assert!(
        err_str.contains(&(ErrorCode::LegacyMerchantIdMismatch as u32).to_string()),
        "expected LegacyMerchantIdMismatch, got: {err_str}"
    );
}

#[tokio::test]
async fn test_transfer_using_legacy_delegate_wrong_debitor_fails() {
    let mut ctx = setup_and_initialize();
    let lctx = setup_legacy_delegate_ctx(&mut ctx);
    let program_id = merchant_id_bytes(TEST_MERCHANT_ID);

    let (impostor_kp, impostor_pk) = setup_keypair(&mut ctx);

    let accounts = bridge_cards::accounts::TransferUsingLegacyDelegate {
        debitor: impostor_pk,
        spender_state: lctx.spender_ctx.spender_state.pubkey,
        payer: ctx.payer_pk,
        user_delegate_account: lctx.user_delegate_pda,
        merchant_delegate_state: lctx.merchant_delegate_pda,
        user_ata: lctx.user_ata,
        receiver_ata: lctx.destination_ata,
        delegate_destination_state: lctx.delegate_dest_pda,
        token_mint: lctx.mint_pk,
        token_program: spl_token::id(),
        system_program: anchor_lang::system_program::ID,
    };
    let ix_data = bridge_cards::instruction::TransferUsingLegacyDelegate {
        program_id,
        merchant_id: TEST_MERCHANT_ID,
        amount: TRANSFER_AMOUNT,
    }
    .data();
    let ix = solana_sdk::instruction::Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: ix_data,
    };
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &impostor_kp],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_err(), "expected failure with wrong debitor");
}

#[tokio::test]
async fn test_transfer_using_legacy_delegate_paused_fails() {
    let mut ctx = setup_and_initialize();
    let lctx = setup_legacy_delegate_ctx(&mut ctx);
    let program_id = merchant_id_bytes(TEST_MERCHANT_ID);

    // Pause (admin = payer)
    let pause_accounts = bridge_cards::accounts::UpdatePaused {
        signer: ctx.payer_pk,
        spender_state: lctx.spender_ctx.spender_state.pubkey,
    };
    let pause_ix = solana_sdk::instruction::Instruction {
        program_id: ctx.program_id,
        accounts: pause_accounts.to_account_metas(None),
        data: bridge_cards::instruction::UpdatePaused { paused: true }.data(),
    };
    let tx = create_transaction(&ctx, &[pause_ix]);
    submit_transaction(&mut ctx, tx).unwrap();

    let ix = transfer_legacy_ix(&ctx, &lctx, program_id, TEST_MERCHANT_ID, TRANSFER_AMOUNT);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &lctx.spender_ctx.debitor_kp],
    );
    let result = submit_transaction(&mut ctx, tx);
    assert!(result.is_err(), "expected failure when paused");
    let err_str = format!("{:?}", result.err());
    assert!(
        err_str.contains(&(ErrorCode::ProgramPaused as u32).to_string()),
        "expected ProgramPaused, got: {err_str}"
    );
}
