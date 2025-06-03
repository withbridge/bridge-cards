use crate::common::*;
use anchor_lang::prelude::*;
use bridge_cards::accounts::DebitUser;
use bridge_cards::errors::ErrorCode;
use bridge_cards::state::UserDelegateState;
use litesvm_token::spl_token;
use litesvm_token::CreateAssociatedTokenAccountIdempotent;
use litesvm_token::*;
use solana_program_test::tokio;
use solana_sdk::signature::{Keypair, Signer};

type TestContext = crate::common::Context;

const TEST_MERCHANT_ID: u64 = 1u64;
const MAX_TRANSFER_LIMIT: u64 = 100_000_000; // $100 per transaction
const PERIOD_TRANSFER_LIMIT: u64 = 2_000_000_000; // $2000 per day
const INITIAL_BALANCE: u64 = 5_000_000_000; // $5000 initial balance
const DEBIT_AMOUNT: u64 = 50_000_000; // $50 debit amount
const LIMIT_PERIOD: u32 = 86400; // 1 day in seconds

// Macro to generate parameterized tests for both TOKEN and TOKEN22 programs.
macro_rules! parameterized_token_test {
    ($test_name:ident, $test_body:expr) => {
        paste::paste! {
            #[tokio::test]
            async fn [<$test_name _token>]() {
                ($test_body)(TokenProgram::Token).await;
            }

            #[tokio::test]
            async fn [<$test_name _token22>]() {
                ($test_body)(TokenProgram::Token2022).await;
            }
        }
    };
}

struct DebitUserContext {
    mint_pk: Pubkey,
    debitor_pk: Pubkey,
    debitor_kp: Keypair,
    debitor_state_pda: Pubkey,
    destination_state_pda: Pubkey,
    user_token_account: Pubkey,
    destination_token_account: Pubkey,
    user_delegate_pda: Pubkey,
    token_program: TokenProgram,
}

fn setup_merchant_and_user_delegate(
    ctx: &mut TestContext,
    max_transfer_limit: u64,
    period_transfer_limit: u64,
) -> DebitUserContext {
    setup_merchant_and_user_delegate_with_program(
        ctx,
        max_transfer_limit,
        period_transfer_limit,
        TokenProgram::Token,
    )
}

fn setup_merchant_and_user_delegate_with_program(
    ctx: &mut TestContext,
    max_transfer_limit: u64,
    period_transfer_limit: u64,
    token_program: TokenProgram,
) -> DebitUserContext {
    // Create a token mint
    let mint_pk = setup_mint_with_program(ctx, token_program);

    // Setup merchant
    let (debitor_kp, debitor_pk) = setup_keypair(ctx);
    let (_, destination_pk) = setup_keypair(ctx);

    let (debitor_state_pda, destination_state_pda, destination_token_account) =
        setup_merchant_debitor_and_destination(
            ctx,
            TEST_MERCHANT_ID,
            debitor_pk,
            &mint_pk,
            &destination_pk,
        );

    // Create user and their token account
    let (user_kp, user_pk) = setup_keypair(ctx);

    // Create token account for the user with initial balance
    let user_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&user_pk)
            .send()
            .unwrap();

    // Fund the user's token account
    MintTo::new(
        &mut ctx.svm,
        &ctx.payer_kp,
        &mint_pk,
        &user_token_account,
        INITIAL_BALANCE,
    )
    .send()
    .unwrap();

    // Create the user delegate account
    let user_delegate_pda = make_user_delegate_pda(
        TEST_MERCHANT_ID,
        &mint_pk,
        &user_token_account,
        &ctx.program_id,
    );

    // checked-approve the user delegate pda for the user token account
    ApproveChecked::new(
        &mut ctx.svm,
        &user_kp,
        &user_delegate_pda.pubkey,
        &mint_pk,
        1e18 as u64,
    )
    .send()
    .unwrap();

    let user_delegate_accounts = bridge_cards::accounts::AddOrUpdateUserDelegate {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        user_token_account,
        mint: mint_pk,
        user_delegate_account: user_delegate_pda.pubkey,
        system_program: System::id(),
    };

    let user_delegate_ix = create_add_or_update_user_delegate_instruction(
        ctx,
        &user_delegate_accounts,
        TEST_MERCHANT_ID,
        max_transfer_limit,
        period_transfer_limit,
        LIMIT_PERIOD,
    );
    let user_delegate_tx = create_transaction_with_payer_and_signers(
        ctx,
        &[user_delegate_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );

    submit_transaction(ctx, user_delegate_tx).unwrap();

    DebitUserContext {
        mint_pk,
        debitor_pk,
        debitor_kp,
        debitor_state_pda,
        destination_state_pda,
        user_token_account,
        destination_token_account,
        user_delegate_pda: user_delegate_pda.pubkey,
        token_program,
    }
}

/// Helper function to verify token account balance based on token program
fn verify_token_account_balance(
    ctx: &TestContext,
    token_account: &Pubkey,
    expected_amount: u64,
    token_program: TokenProgram,
    error_msg: &str,
) {
    match token_program {
        TokenProgram::Token => {
            let account_info = get_spl_account::<spl_token::state::Account>(&ctx.svm, token_account)
                .unwrap();
            assert_eq!(account_info.amount, expected_amount, "{}", error_msg);
        }
        TokenProgram::Token2022 => {
            let account_info = get_spl_account::<spl_token_2022::state::Account>(&ctx.svm, token_account)
                .unwrap();
            assert_eq!(account_info.amount, expected_amount, "{}", error_msg);
        }
    }
}

parameterized_token_test!(test_debit_user_successful, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create the debit user instruction
    let debit_accounts = DebitUser {
        debitor: debit_context.debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: debit_context.destination_token_account,
        mint: debit_context.mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    // Execute the transaction
    let result = submit_transaction(&mut ctx, debit_tx);
    assert!(result.is_ok(), "Failed to debit user: {:?}", result.err());

    // Verify the log message
    let meta = result.unwrap();
    let expected_log = "Instruction: DebitUser"; // Check for instruction log
    assert!(
        meta.logs.iter().any(|log| log.contains(expected_log)),
        "Expected log containing '{}' not found in logs: {}",
        expected_log,
        meta.logs.join("\n")
    );

    // Verify user token account balance decreased
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE - DEBIT_AMOUNT,
        token_program,
        "User token account balance incorrect",
    );

    // Verify destination token account balance increased
    verify_token_account_balance(
        &ctx,
        &debit_context.destination_token_account,
        DEBIT_AMOUNT,
        token_program,
        "Destination token account balance incorrect",
    );

    // Verify user delegate state was updated
    let user_delegate_account = ctx
        .svm
        .get_account(&debit_context.user_delegate_pda)
        .unwrap();
    let user_delegate_state =
        UserDelegateState::try_deserialize(&mut user_delegate_account.data.as_slice()).unwrap();

    assert_eq!(
        user_delegate_state.period_transferred_amount, DEBIT_AMOUNT,
        "User delegate transferred amount incorrect"
    );
});

parameterized_token_test!(test_debit_user_exceeds_max_limit, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Try to debit more than the max transfer limit
    let debit_accounts = DebitUser {
        debitor: debit_context.debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: debit_context.destination_token_account,
        mint: debit_context.mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    let excessive_amount = MAX_TRANSFER_LIMIT + 1;
    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        excessive_amount,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    // Execute the transaction - should fail with ExceedsMaxTransferLimit
    let result = submit_transaction(&mut ctx, debit_tx);
    assert!(
        result.is_err(),
        "Transaction should fail due to exceeding max transfer limit"
    );

    // Verify the error matches ExceedsMaxTransferLimit
    let err = result.err().unwrap();
    let expected_message = ErrorCode::ExceedsMaxTransferLimit.to_string();
    assert!(
        err.meta
            .logs
            .iter()
            .any(|log| log.contains(&expected_message)),
        "Error should contain the expected error message {}, got {}",
        expected_message,
        err.meta.logs.join(", ")
    );

    // Verify user token account balance remains unchanged
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE,
        token_program,
        "User token account balance should remain unchanged",
    );
});

parameterized_token_test!(test_debit_user_non_merchant_debitor, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create a different signer (not the merchant's debitor)
    let (non_debitor_kp, non_debitor_pk) = setup_keypair(&mut ctx);

    // Try to debit with non-merchant debitor
    let debit_accounts = DebitUser {
        debitor: non_debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: debit_context.destination_token_account,
        mint: debit_context.mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &non_debitor_kp],
    );

    // Execute the transaction - should fail with constraint violation
    let result = submit_transaction(&mut ctx, debit_tx);
    assert!(
        result.is_err(),
        "Transaction should fail due to debitor constraint violation"
    );

    // Verify user token account balance remains unchanged
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE,
        token_program,
        "User token account balance should remain unchanged",
    );
});

parameterized_token_test!(test_debit_user_period_limit_reset, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create the debit accounts
    let debit_accounts = DebitUser {
        debitor: debit_context.debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: debit_context.destination_token_account,
        mint: debit_context.mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    // First debit
    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    let result1 = submit_transaction(&mut ctx, debit_tx);
    assert!(result1.is_ok(), "First debit failed: {:?}", result1.err());

    // Verify the log message for first debit
    let meta1 = result1.unwrap();
    let expected_log = "Instruction: DebitUser";
    assert!(
        meta1.logs.iter().any(|log| log.contains(expected_log)),
        "Expected log containing '{}' for first debit not found: {}",
        expected_log,
        meta1.logs.join("\n")
    );

    // Second debit after period reset
    let debit_ix2 = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx2 = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix2],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    // advance clock by 1 day
    let mut new_clock = ctx.svm.get_sysvar::<Clock>();
    new_clock.unix_timestamp += 86401;
    new_clock.slot += 1; // Advance slot for next transaction
    ctx.svm.set_sysvar(&new_clock);

    let result2 = submit_transaction(&mut ctx, debit_tx2);
    assert!(result2.is_ok(), "Second debit failed: {:?}", result2.err());

    // Verify the log message for second debit
    let meta2 = result2.unwrap();
    assert!(
        meta2.logs.iter().any(|log| log.contains(expected_log)),
        "Expected log containing '{}' for second debit not found: {}",
        expected_log,
        meta2.logs.join("\n")
    );

    // Verify user token account balance decreased by 2*DEBIT_AMOUNT
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE - (DEBIT_AMOUNT * 2),
        token_program,
        "User token account balance incorrect after period reset",
    );

    // Verify user delegate state was reset
    let user_delegate_account = ctx
        .svm
        .get_account(&debit_context.user_delegate_pda)
        .unwrap();
    let user_delegate_state =
        UserDelegateState::try_deserialize(&mut user_delegate_account.data.as_slice()).unwrap();

    assert_eq!(
        user_delegate_state.period_transferred_amount, DEBIT_AMOUNT,
        "User delegate transferred amount should be reset and then incremented again"
    );
});

parameterized_token_test!(test_debit_user_exceeds_period_limit, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        PERIOD_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create the debit accounts
    let debit_accounts = DebitUser {
        debitor: debit_context.debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: debit_context.destination_token_account,
        mint: debit_context.mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    // First debit - half of period limit
    let first_amount = PERIOD_TRANSFER_LIMIT / 2;
    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        first_amount,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    submit_transaction(&mut ctx, debit_tx).unwrap();

    // Advance slot for next transaction
    let mut new_clock = ctx.svm.get_sysvar::<Clock>();
    new_clock.slot += 1;
    ctx.svm.set_sysvar(&new_clock);

    // Second debit - should exceed period limit
    let second_amount = (PERIOD_TRANSFER_LIMIT / 2) + 1;
    let debit_ix2 = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        second_amount,
        token_program,
    );
    let debit_tx2 = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix2],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    // Execute the transaction - should fail with ExceedsTransferLimitPerPeriod
    let result = submit_transaction(&mut ctx, debit_tx2);
    assert!(
        result.is_err(),
        "Transaction should fail due to exceeding period transfer limit"
    );
    // Verify the error matches ExceedsTransferLimitPerPeriod
    let expected_message = ErrorCode::ExceedsTransferLimitPerPeriod.to_string();
    let err = result.err().unwrap();
    assert!(
        err.meta
            .logs
            .iter()
            .any(|log| log.contains(&expected_message)),
        "Error should contain the expected error message {}, got {}",
        expected_message,
        err.meta.logs.join(", ")
    );

    // Verify user token account balance only reflects the first debit
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE - first_amount,
        token_program,
        "User token account balance should only reflect the first debit",
    );
});

parameterized_token_test!(test_debit_user_incorrect_merchant_id, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create the debit accounts but use incorrect merchant_id
    let debit_accounts = DebitUser {
        debitor: debit_context.debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: debit_context.destination_token_account,
        mint: debit_context.mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    // Use incorrect merchant_id (different from TEST_MERCHANT_ID)
    let incorrect_merchant_id = TEST_MERCHANT_ID + 1;
    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        incorrect_merchant_id,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    // Execute the transaction - should fail due to incorrect merchant_id
    let result = submit_transaction(&mut ctx, debit_tx);
    assert!(
        result.is_err(),
        "Transaction should fail due to incorrect merchant_id"
    );

    // Verify user token account balance remains unchanged
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE,
        token_program,
        "User token account balance should remain unchanged",
    );
});

parameterized_token_test!(test_debit_user_invalid_destination, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create a different destination token account
    let (_, invalid_destination_pk) = setup_keypair(&mut ctx);
    let invalid_destination_token_account = CreateAssociatedTokenAccountIdempotent::new(
        &mut ctx.svm,
        &ctx.payer_kp,
        &debit_context.mint_pk,
    )
    .owner(&invalid_destination_pk)
    .send()
    .unwrap();

    // Try to debit with invalid destination
    let debit_accounts = DebitUser {
        debitor: debit_context.debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: invalid_destination_token_account, // Wrong destination
        mint: debit_context.mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    // Execute the transaction - should fail with constraint violation
    let result = submit_transaction(&mut ctx, debit_tx);
    assert!(
        result.is_err(),
        "Transaction should fail due to invalid destination account"
    );

    // Verify user token account balance remains unchanged
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE,
        token_program,
        "User token account balance should remain unchanged",
    );
});

parameterized_token_test!(test_debit_user_insufficient_balance, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    // Create user with a small balance
    let small_initial_balance = DEBIT_AMOUNT / 2; // Half of the debit amount

    // Setup merchant and user delegate with custom balance
    let mint_pk = setup_mint_with_program(&mut ctx, token_program);
    let (debitor_kp, debitor_pk) = setup_keypair(&mut ctx);
    let (_, destination_pk) = setup_keypair(&mut ctx);
    let destination_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&destination_pk)
            .send()
            .unwrap();

    // Save payer details before mutable borrows
    let payer_pk = ctx.payer_pk;
    let payer_kp = ctx.payer_kp.insecure_clone();
    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create user with small balance
    let (user_kp, user_pk) = setup_keypair(&mut ctx);
    let user_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, &mint_pk)
            .owner(&user_pk)
            .send()
            .unwrap();

    // Fund the user's token account with a small balance
    MintTo::new(
        &mut ctx.svm,
        &ctx.payer_kp,
        &mint_pk,
        &user_token_account,
        small_initial_balance,
    )
    .send()
    .unwrap();

    // Create the user delegate account
    let user_delegate_pda = make_user_delegate_pda(
        TEST_MERCHANT_ID,
        &mint_pk,
        &user_token_account,
        &ctx.program_id,
    );

    // checked-approve the user delegate pda for the user token account
    ApproveChecked::new(
        &mut ctx.svm,
        &user_kp,
        &user_delegate_pda.pubkey,
        &mint_pk,
        1e18 as u64,
    )
    .send()
    .unwrap();

    let user_delegate_accounts = bridge_cards::accounts::AddOrUpdateUserDelegate {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: payer_pk,
        user_token_account,
        mint: mint_pk,
        user_delegate_account: user_delegate_pda.pubkey,
        system_program: System::id(),
    };

    let user_delegate_ix = create_add_or_update_user_delegate_instruction(
        &ctx,
        &user_delegate_accounts,
        TEST_MERCHANT_ID,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        LIMIT_PERIOD,
    );
    let user_delegate_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[user_delegate_ix],
        Some(&payer_pk),
        &[&payer_kp, &ctx.merchant_manager_kp],
    );

    submit_transaction(&mut ctx, user_delegate_tx).unwrap();

    // Try to debit more than the available balance
    let debit_accounts = DebitUser {
        debitor: debitor_pk,
        payer: payer_pk,
        user_delegate_account: user_delegate_pda.pubkey,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account,
        destination_token_account,
        mint: mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&payer_pk),
        &[&payer_kp, &debitor_kp],
    );

    // Execute the transaction - should fail due to insufficient funds
    let result = submit_transaction(&mut ctx, debit_tx);
    assert!(
        result.is_err(),
        "Transaction should fail due to insufficient balance"
    );

    // Verify user token account balance remains unchanged
    verify_token_account_balance(
        &ctx,
        &user_token_account,
        small_initial_balance,
        token_program,
        "User token account balance should remain unchanged",
    );
});

parameterized_token_test!(test_debit_user_incorrect_mint, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create a different mint
    let different_mint_pk = setup_mint_with_program(&mut ctx, token_program);

    // Try to debit with incorrect mint
    let debit_accounts = DebitUser {
        debitor: debit_context.debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: debit_context.destination_token_account,
        mint: different_mint_pk, // Wrong mint
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    // Execute the transaction - should fail with constraint or seed derivation error
    let result = submit_transaction(&mut ctx, debit_tx);
    assert!(
        result.is_err(),
        "Transaction should fail due to incorrect mint"
    );

    // Verify user token account balance remains unchanged
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE,
        token_program,
        "User token account balance should remain unchanged",
    );
});

parameterized_token_test!(test_debit_user_exact_period_boundary, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        PERIOD_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create the debit accounts
    let debit_accounts = DebitUser {
        debitor: debit_context.debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: debit_context.destination_token_account,
        mint: debit_context.mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    // First debit - half of period limit
    let first_amount = PERIOD_TRANSFER_LIMIT / 2;
    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        first_amount,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    submit_transaction(&mut ctx, debit_tx).unwrap();
    // Advance clock to exactly the period boundary
    let mut new_clock = ctx.svm.get_sysvar::<Clock>();
    new_clock.unix_timestamp += LIMIT_PERIOD as i64;
    new_clock.slot += 1; // Advance slot for next transaction
    ctx.svm.set_sysvar(&new_clock);

    // Second debit - should work because we're exactly at the period boundary
    let debit_ix2 = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        first_amount,
        token_program,
    );
    let debit_tx2 = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix2],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    let result = submit_transaction(&mut ctx, debit_tx2);
    assert!(
        result.is_ok(),
        "Transaction should succeed at exactly the period boundary: {:?}",
        result.err()
    );

    // Verify the log message for second debit
    let meta2 = result.unwrap();
    let expected_log = "Instruction: DebitUser";
    assert!(
        meta2.logs.iter().any(|log| log.contains(expected_log)),
        "Expected log containing '{}' for second debit at boundary not found: {}",
        expected_log,
        meta2.logs.join("\n")
    );

    // Verify user token account balance decreased by 2*first_amount
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE - (first_amount * 2),
        token_program,
        "User token account balance incorrect after period boundary reset",
    );
});

parameterized_token_test!(test_debit_user_multiple_transactions_within_period, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create the debit accounts
    let debit_accounts = DebitUser {
        debitor: debit_context.debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: debit_context.destination_token_account,
        mint: debit_context.mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    // Perform multiple small debits within the period
    let debit_amount_small = DEBIT_AMOUNT / 5; // Small enough for multiple transactions
    let num_transactions = 5;
    let total_debit_amount = debit_amount_small * num_transactions;

    // Ensure total is within period limit
    assert!(
        total_debit_amount < PERIOD_TRANSFER_LIMIT,
        "Test setup error: total debit amount exceeds period limit"
    );

    for i in 0..num_transactions {
        let debit_ix = create_debit_user_instruction_with_program(
            &ctx,
            &debit_accounts,
            TEST_MERCHANT_ID,
            debit_amount_small,
            token_program,
        );
        let debit_tx = create_transaction_with_payer_and_signers(
            &ctx,
            &[debit_ix],
            Some(&ctx.payer_pk),
            &[&ctx.payer_kp, &debit_context.debitor_kp],
        );

        let result = submit_transaction(&mut ctx, debit_tx);
        assert!(
            result.is_ok(),
            "Transaction {} should succeed: {:?}",
            i + 1,
            result.err()
        );

        // Verify the log message for this debit
        let meta = result.unwrap();
        let expected_log = "Instruction: DebitUser";
        assert!(
            meta.logs.iter().any(|log| log.contains(expected_log)),
            "Expected log containing '{}' for transaction {} not found: {}",
            expected_log,
            i + 1,
            meta.logs.join("\n")
        );

        // Advance both clock and slot to allow next transaction
        let mut new_clock = ctx.svm.get_sysvar::<Clock>();
        new_clock.unix_timestamp += 60; // 1 minute
        new_clock.slot += 1; // Advance slot
        ctx.svm.set_sysvar(&new_clock);
    }

    // Verify user token account balance decreased by total amount
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE - total_debit_amount,
        token_program,
        "User token account balance incorrect after multiple transactions",
    );

    // Verify user delegate state accumulated all transactions
    let user_delegate_account = ctx
        .svm
        .get_account(&debit_context.user_delegate_pda)
        .unwrap();
    let user_delegate_state =
        UserDelegateState::try_deserialize(&mut user_delegate_account.data.as_slice()).unwrap();

    assert_eq!(
        user_delegate_state.period_transferred_amount, total_debit_amount,
        "User delegate transferred amount should accumulate all transactions"
    );
});

parameterized_token_test!(test_debit_user_same_slot, |token_program: TokenProgram| async move {
    // Setup the test environment
    let mut ctx = setup_and_initialize();

    let debit_context = setup_merchant_and_user_delegate_with_program(
        &mut ctx,
        MAX_TRANSFER_LIMIT,
        PERIOD_TRANSFER_LIMIT,
        token_program,
    );

    // Create the debit accounts
    let debit_accounts = DebitUser {
        debitor: debit_context.debitor_pk,
        payer: ctx.payer_pk,
        user_delegate_account: debit_context.user_delegate_pda,
        debitor_state: debit_context.debitor_state_pda,
        destination_state: debit_context.destination_state_pda,
        user_token_account: debit_context.user_token_account,
        destination_token_account: debit_context.destination_token_account,
        mint: debit_context.mint_pk,
        system_program: System::id(),
        token_program: token_program.program_id(),
    };

    // First debit
    let debit_ix = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    let result1 = submit_transaction(&mut ctx, debit_tx);
    assert!(result1.is_ok(), "First debit failed: {:?}", result1.err());

    // Second debit in same slot
    let debit_ix2 = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx2 = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix2],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    // Execute second transaction - should fail with ExceedsMaxTransactionsPerSlot
    let result2 = submit_transaction(&mut ctx, debit_tx2);
    assert!(
        result2.is_err(),
        "Transaction should fail due to same slot transaction"
    );

    // Verify the error matches ExceedsMaxTransactionsPerSlot
    let err = result2.err().unwrap();
    let expected_message = ErrorCode::ExceedsMaxTransactionsPerSlot.to_string();
    assert!(
        err.meta
            .logs
            .iter()
            .any(|log| log.contains(&expected_message)),
        "Error should contain the expected error message {}, got {}",
        expected_message,
        err.meta.logs.join("\n")
    );

    // Verify user token account balance only reflects the first debit
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE - DEBIT_AMOUNT,
        token_program,
        "User token account balance should only reflect the first debit",
    );

    // Third debit in different slot should succeed
    let mut new_clock = ctx.svm.get_sysvar::<Clock>();
    new_clock.slot += 1;
    ctx.svm.set_sysvar(&new_clock);

    let debit_ix3 = create_debit_user_instruction_with_program(
        &ctx,
        &debit_accounts,
        TEST_MERCHANT_ID,
        DEBIT_AMOUNT,
        token_program,
    );
    let debit_tx3 = create_transaction_with_payer_and_signers(
        &ctx,
        &[debit_ix3],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &debit_context.debitor_kp],
    );

    let result3 = submit_transaction(&mut ctx, debit_tx3);
    assert!(
        result3.is_ok(),
        "Third debit in different slot failed: {:?}",
        result3.err()
    );

    // Verify final balance reflects two successful debits
    verify_token_account_balance(
        &ctx,
        &debit_context.user_token_account,
        INITIAL_BALANCE - (DEBIT_AMOUNT * 2),
        token_program,
        "User token account balance should reflect two successful debits",
    );
});
