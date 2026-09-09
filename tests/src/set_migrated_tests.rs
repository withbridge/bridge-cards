use crate::common::*;
use account_data_trait::AccountData;
use anchor_lang::{AnchorDeserialize, InstructionData, ToAccountMetas};
use bridge_cards::state::BridgeCardsState;
use solana_account::Account;
use solana_program_test::tokio;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, rent::Rent, signature::Signer};

fn create_set_migrated_instruction(ctx: &Context, migrated: bool) -> Instruction {
    Instruction {
        program_id: ctx.program_id,
        accounts: bridge_cards::accounts::SetMigrated {
            admin: ctx.payer_pk,
            payer: ctx.payer_pk,
            state: ctx.bridge_cards_state.pubkey,
            system_program: anchor_lang::solana_program::system_program::id(),
        }
        .to_account_metas(None),
        data: bridge_cards::instruction::SetMigrated { migrated }.data(),
    }
}

fn read_state(ctx: &Context) -> BridgeCardsState {
    let data = ctx
        .svm
        .get_account(&ctx.bridge_cards_state.pubkey)
        .unwrap()
        .data;
    // skip 8-byte discriminator then borsh-deserialize
    BridgeCardsState::deserialize(&mut &data[8..]).unwrap()
}

// ── Basic functionality ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_set_migrated_true() {
    let mut ctx = setup_and_initialize();

    let ix = create_set_migrated_instruction(&ctx, true);
    let tx =
        create_transaction_with_payer_and_signers(&ctx, &[ix], Some(&ctx.payer_pk), &[&ctx.payer_kp]);
    submit_transaction(&mut ctx, tx).unwrap();

    assert!(read_state(&ctx).migrated);
}

#[tokio::test]
async fn test_set_migrated_toggle() {
    let mut ctx = setup_and_initialize();

    for migrated in [true, false, true] {
        let ix = create_set_migrated_instruction(&ctx, migrated);
        let tx = create_transaction_with_payer_and_signers(
            &ctx,
            &[ix],
            Some(&ctx.payer_pk),
            &[&ctx.payer_kp],
        );
        submit_transaction(&mut ctx, tx).unwrap();
        assert_eq!(read_state(&ctx).migrated, migrated);
    }
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
            system_program: anchor_lang::solana_program::system_program::id(),
        }
        .to_account_metas(None),
        data: bridge_cards::instruction::SetMigrated { migrated: true }.data(),
    };
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &fake_admin],
    );
    assert!(
        submit_transaction(&mut ctx, tx).is_err(),
        "wrong admin should be rejected"
    );
}

// ── Upgrade path ──────────────────────────────────────────────────────────────

/// Simulates deploying the new binary onto a mainnet state account that was
/// created with the old layout (no `migrated` field, 41 bytes total).
/// Verifies that `set_migrated` can expand and migrate it successfully.
#[tokio::test]
async fn test_set_migrated_expands_old_layout_account() {
    let mut ctx = setup_and_initialize();

    let bump = ctx.bridge_cards_state.bump;

    // Build the old 41-byte layout: discriminator(8) + admin(32) + bump(1).
    // This is what every mainnet account looks like before this upgrade.
    let expected_disc = BridgeCardsState {
        admin: ctx.payer_pk,
        bump,
        migrated: false,
    }
    .account_data();
    // account_data() returns discriminator + borsh data (42 bytes).
    // The old layout is the first 41 bytes (no migrated byte).
    let old_data = expected_disc[..41].to_vec();
    assert_eq!(old_data.len(), 41);

    let rent = Rent::default();
    ctx.svm
        .set_account(
            ctx.bridge_cards_state.pubkey,
            Account {
                lamports: rent.minimum_balance(41),
                data: old_data,
                owner: ctx.program_id,
                executable: false,
                rent_epoch: u64::MAX,
            },
        )
        .unwrap();

    // Calling any other instruction would now fail with UnexpectedEof.
    // Verify set_migrated can repair the account.
    let ix = create_set_migrated_instruction(&ctx, true);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );
    submit_transaction(&mut ctx, tx)
        .expect("set_migrated should succeed on old-layout (41-byte) account");

    let account = ctx.svm.get_account(&ctx.bridge_cards_state.pubkey).unwrap();
    assert_eq!(account.data.len(), 42, "account should be expanded to 42 bytes");

    let state = read_state(&ctx);
    assert!(state.migrated);
    assert_eq!(state.admin, ctx.payer_pk, "admin preserved after expansion");
    assert_eq!(state.bump, bump, "bump preserved after expansion");
}

/// After set_migrated expands the old-layout account, subsequent instructions
/// that deserialize state should work normally.
#[tokio::test]
async fn test_instructions_work_after_upgrade_migration() {
    let mut ctx = setup_and_initialize();
    let bump = ctx.bridge_cards_state.bump;

    // Shrink to old layout.
    let full_data = BridgeCardsState {
        admin: ctx.payer_pk,
        bump,
        migrated: false,
    }
    .account_data();
    let old_data = full_data[..41].to_vec();

    let rent = Rent::default();
    ctx.svm
        .set_account(
            ctx.bridge_cards_state.pubkey,
            Account {
                lamports: rent.minimum_balance(41),
                data: old_data,
                owner: ctx.program_id,
                executable: false,
                rent_epoch: u64::MAX,
            },
        )
        .unwrap();

    // Repair with set_migrated(false) — expands without locking the program.
    let ix = create_set_migrated_instruction(&ctx, false);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );
    submit_transaction(&mut ctx, tx).expect("set_migrated(false) should repair the account");

    // add_or_update_merchant_manager also loads state; verify it works.
    let manager_pk = Pubkey::new_unique();
    let manager_state_pda = make_pda(
        &[
            bridge_cards::instructions::add_or_update_merchant_manager::MERCHANT_MANAGER_SEED,
            &99u64.to_le_bytes(),
        ],
        &ctx.program_id,
    );
    let accounts = bridge_cards::accounts::AddOrUpdateMerchantManager {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        manager_state: manager_state_pda.pubkey,
        manager: manager_pk,
        system_program: anchor_lang::solana_program::system_program::id(),
    };
    let ix = create_add_or_update_merchant_manager_instruction(&ctx, &accounts, 99);
    let tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );
    assert!(
        submit_transaction(&mut ctx, tx).is_ok(),
        "add_merchant_manager should work after account expansion"
    );
}
