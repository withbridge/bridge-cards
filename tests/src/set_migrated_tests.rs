use crate::common::*;
use anchor_lang::{InstructionData, ToAccountMetas};
use bridge_cards::instructions::add_or_update_merchant_manager::MERCHANT_MANAGER_SEED;
use solana_program_test::tokio;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Signer};

fn migration_state_pda(ctx: &Context) -> Pubkey {
    make_migration_state_pda(&ctx.program_id)
}

fn is_migrated(ctx: &Context) -> bool {
    ctx.svm
        .get_account(&migration_state_pda(ctx))
        .map(|a| a.lamports > 0 && a.owner == ctx.program_id)
        .unwrap_or(false)
}

fn create_set_migrated_instruction(ctx: &Context, migrated: bool) -> Instruction {
    Instruction {
        program_id: ctx.program_id,
        accounts: bridge_cards::accounts::SetMigrated {
            admin: ctx.payer_pk,
            payer: ctx.payer_pk,
            state: ctx.bridge_cards_state.pubkey,
            migration_state: migration_state_pda(ctx),
            system_program: anchor_lang::solana_program::system_program::id(),
        }
        .to_account_metas(None),
        data: bridge_cards::instruction::SetMigrated { migrated }.data(),
    }
}

fn do_set_migrated(ctx: &mut Context, migrated: bool) {
    let ix = create_set_migrated_instruction(ctx, migrated);
    let tx = create_transaction_with_payer_and_signers(
        ctx, &[ix], Some(&ctx.payer_pk), &[&ctx.payer_kp.insecure_clone()],
    );
    submit_transaction(ctx, tx).unwrap();
}

#[tokio::test]
async fn test_set_migrated_true_creates_pda() {
    let mut ctx = setup_and_initialize();
    assert!(!is_migrated(&ctx));
    do_set_migrated(&mut ctx, true);
    assert!(is_migrated(&ctx), "migration PDA should exist after set_migrated(true)");
}

#[tokio::test]
async fn test_set_migrated_false_closes_pda() {
    let mut ctx = setup_and_initialize();
    do_set_migrated(&mut ctx, true);
    assert!(is_migrated(&ctx));
    do_set_migrated(&mut ctx, false);
    assert!(!is_migrated(&ctx), "migration PDA should be closed");
}

#[tokio::test]
async fn test_set_migrated_toggle() {
    let mut ctx = setup_and_initialize();
    for expected in [true, false, true, false] {
        do_set_migrated(&mut ctx, expected);
        assert_eq!(is_migrated(&ctx), expected);
    }
}

#[tokio::test]
async fn test_set_migrated_idempotent() {
    let mut ctx = setup_and_initialize();
    do_set_migrated(&mut ctx, true);
    do_set_migrated(&mut ctx, true);
    assert!(is_migrated(&ctx));
    do_set_migrated(&mut ctx, false);
    do_set_migrated(&mut ctx, false);
    assert!(!is_migrated(&ctx));
}

#[tokio::test]
async fn test_set_migrated_wrong_admin_rejected() {
    let mut ctx = setup_and_initialize();
    let fake_admin = ctx.extra_keypair.insecure_clone();
    let ix = Instruction {
        program_id: ctx.program_id,
        accounts: bridge_cards::accounts::SetMigrated {
            admin: fake_admin.pubkey(),
            payer: ctx.payer_pk,
            state: ctx.bridge_cards_state.pubkey,
            migration_state: migration_state_pda(&ctx),
            system_program: anchor_lang::solana_program::system_program::id(),
        }.to_account_metas(None),
        data: bridge_cards::instruction::SetMigrated { migrated: true }.data(),
    };
    let payer_kp = ctx.payer_kp.insecure_clone();
    let tx = create_transaction_with_payer_and_signers(
        &ctx, &[ix], Some(&ctx.payer_pk), &[&payer_kp, &fake_admin],
    );
    assert!(submit_transaction(&mut ctx, tx).is_err(), "wrong admin should be rejected");
}

#[tokio::test]
async fn test_post_migration_instructions_blocked_then_restored() {
    let mut ctx = setup_and_initialize();
    do_set_migrated(&mut ctx, true);

    let manager_pk = Pubkey::new_unique();
    let manager_state_pda = make_pda(&[MERCHANT_MANAGER_SEED, &99u64.to_le_bytes()], &ctx.program_id);
    let accounts = bridge_cards::accounts::AddOrUpdateMerchantManager {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        manager_state: manager_state_pda.pubkey,
        manager: manager_pk,
        migration_state: migration_state_pda(&ctx),
        system_program: anchor_lang::solana_program::system_program::id(),
    };

    let ix = create_add_or_update_merchant_manager_instruction(&ctx, &accounts, 99);
    let payer_kp = ctx.payer_kp.insecure_clone();
    let tx = create_transaction_with_payer_and_signers(
        &ctx, &[ix], Some(&ctx.payer_pk), &[&payer_kp],
    );
    assert!(submit_transaction(&mut ctx, tx).is_err(), "should be blocked when migrated");

    do_set_migrated(&mut ctx, false);

    let ix2 = create_add_or_update_merchant_manager_instruction(&ctx, &accounts, 99);
    let payer_kp2 = ctx.payer_kp.insecure_clone();
    let tx2 = create_transaction_with_payer_and_signers(
        &ctx, &[ix2], Some(&ctx.payer_pk), &[&payer_kp2],
    );
    assert!(submit_transaction(&mut ctx, tx2).is_ok(), "should work again after rollback");
}
