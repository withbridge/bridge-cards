use std::path::PathBuf;

use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;
use bridge_cards::accounts::DebitUser;
use bridge_cards::accounts::{
    AddOrUpdateMerchantDebitor, AddOrUpdateMerchantDestination, AddOrUpdateMerchantManager,
    AddOrUpdateUserDelegate, Initialize, UpdateAdmin,
};
use bridge_cards::instructions::add_or_update_merchant_debitor::MERCHANT_DEBITOR_SEED;
use bridge_cards::instructions::add_or_update_merchant_destination::MERCHANT_DESTINATION_SEED;
use bridge_cards::instructions::add_or_update_merchant_manager::MERCHANT_MANAGER_SEED;
use bridge_cards::instructions::add_or_update_user_delegate::USER_DELEGATE_SEED;
use litesvm::types::TransactionResult;
use litesvm::LiteSVM;
use litesvm_token::*;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TokenProgram {
    Token,
    Token2022,
}

impl TokenProgram {
    pub fn program_id(&self) -> Pubkey {
        match self {
            TokenProgram::Token => spl_token::id(),
            TokenProgram::Token2022 => spl_token_2022::id(),
        }
    }
}

pub struct Context {
    pub svm: LiteSVM,
    pub payer_kp: Keypair,
    pub payer_pk: Pubkey,
    pub program_id: Pubkey,
    pub bridge_cards_state: PDAWithBump,
    pub merchant_manager_kp: Keypair,
    pub merchant_manager_state: PDAWithBump,
    pub extra_keypair: Keypair,
}

pub struct PDAWithBump {
    pub pubkey: Pubkey,
    pub bump: u8,
}

pub fn read_program(name: &str) -> Vec<u8> {
    let mut so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    so_path.push(format!("../target/deploy/{name}.so"));

    println!("CARGO_MANIFEST_DIR: {}", env!("CARGO_MANIFEST_DIR"));
    println!("so_path: {}", so_path.to_string_lossy());

    std::fs::read(so_path).unwrap()
}

pub fn make_pda(seeds: &[&[u8]], program_id: &Pubkey) -> PDAWithBump {
    let (pda, bump) = Pubkey::find_program_address(seeds, program_id);
    PDAWithBump { pubkey: pda, bump }
}

pub fn setup() -> Context {
    let mut svm = LiteSVM::new();
    let payer_kp = Keypair::new();
    let merchant_manager_kp = Keypair::new();
    let payer_pk = payer_kp.try_pubkey().unwrap();
    let program_id = bridge_cards::ID;
    let bridge_cards_state = make_pda(&[b"state"], &program_id);
    let merchant_id = 1u64; // Default merchant ID for testing
    let merchant_manager_state = make_pda(
        &[MERCHANT_MANAGER_SEED, &merchant_id.to_le_bytes()],
        &program_id,
    );
    let extra_keypair = Keypair::new();

    svm.airdrop(&payer_pk, 1000000000).unwrap();
    svm.add_program(program_id, &read_program("bridge_cards"));

    svm.warp_to_slot(1000000000);

    Context {
        svm,
        payer_kp,
        payer_pk,
        program_id,
        bridge_cards_state,
        merchant_manager_kp,
        merchant_manager_state,
        extra_keypair,
    }
}

pub fn setup_and_initialize() -> Context {
    let mut ctx = setup();
    initialize_bridge_cards(&mut ctx);
    setup_merchant_manager(&mut ctx, 1); // Set up merchant manager for default merchant ID
    ctx
}

pub fn initialize_bridge_cards(ctx: &mut Context) {
    let ix = create_initialize_instruction(ctx);
    let tx = create_transaction_with_payer_and_signers(
        ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.extra_keypair],
    );
    submit_transaction(ctx, tx).unwrap();
}

pub fn setup_merchant_manager(ctx: &mut Context, merchant_id: u64) -> Pubkey {
    let (manager_state, _) = Pubkey::find_program_address(
        &[MERCHANT_MANAGER_SEED, &merchant_id.to_le_bytes()],
        &ctx.program_id,
    );

    let accounts = AddOrUpdateMerchantManager {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        manager_state,
        manager: ctx.merchant_manager_kp.pubkey(),
        system_program: anchor_lang::system_program::ID,
    };

    let ix = create_add_or_update_merchant_manager_instruction(ctx, &accounts, merchant_id);
    let tx = create_transaction_with_payer_and_signers(
        ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp],
    );
    submit_transaction(ctx, tx).unwrap();
    manager_state
}

pub fn create_add_or_update_merchant_manager_instruction(
    ctx: &Context,
    accounts: &AddOrUpdateMerchantManager,
    merchant_id: u64,
) -> Instruction {
    let ix_data = bridge_cards::instruction::AddOrUpdateMerchantManager { merchant_id }.data();

    Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: ix_data,
    }
}

pub fn create_initialize_instruction(ctx: &Context) -> Instruction {
    let accounts = Initialize {
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        program_account: ctx.extra_keypair.pubkey(),
        system_program: anchor_lang::system_program::ID,
    };
    create_initialize_instruction_with_accounts(ctx, accounts)
}

pub fn create_initialize_instruction_with_accounts(
    ctx: &Context,
    accounts: Initialize,
) -> Instruction {
    let ix_data = bridge_cards::instruction::Initialize {}.data();
    Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: ix_data,
    }
}

pub fn create_update_admin_instruction(ctx: &Context, accounts: UpdateAdmin) -> Instruction {
    let ix_data = bridge_cards::instruction::UpdateAdmin {}.data();
    Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: ix_data,
    }
}

pub fn create_transaction(ctx: &Context, instructions: &[Instruction]) -> Transaction {
    let payer = Some(&ctx.payer_pk);
    let signers = vec![&ctx.payer_kp];
    create_transaction_with_payer_and_signers(ctx, instructions, payer, &signers)
}

pub fn create_transaction_with_payer_and_signers(
    ctx: &Context,
    instructions: &[Instruction],
    payer: Option<&Pubkey>,
    signers: &[&Keypair],
) -> Transaction {
    Transaction::new_signed_with_payer(instructions, payer, signers, ctx.svm.latest_blockhash())
}

pub fn submit_transaction(ctx: &mut Context, tx: Transaction) -> TransactionResult {
    let result = ctx.svm.send_transaction(tx);
    ctx.svm.expire_blockhash();
    result
}

// Added helpers from add_or_update_merchant_tests.rs

pub fn setup_keypair(ctx: &mut Context) -> (Keypair, Pubkey) {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey();
    ctx.svm.airdrop(&pubkey, 1000000000).unwrap();
    (keypair, pubkey)
}

pub fn setup_mint(ctx: &mut Context) -> Pubkey {
    // Use default token program.
    setup_mint_with_program(ctx, TokenProgram::Token)
}

pub fn setup_mint_with_program(ctx: &mut Context, token_program: TokenProgram) -> Pubkey {
    CreateMint::new(&mut ctx.svm, &ctx.payer_kp)
        .token_program_id(&token_program.program_id())
        .decimals(6)
        .send()
        .unwrap()
}

pub fn make_merchant_debitor_pda(
    merchant_id: u64,
    debitor: &Pubkey,
    mint: &Pubkey,
    program_id: &Pubkey,
) -> PDAWithBump {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            MERCHANT_DEBITOR_SEED,
            &merchant_id.to_le_bytes(),
            mint.as_ref(),
            debitor.as_ref(),
        ],
        program_id,
    );
    PDAWithBump { pubkey: pda, bump }
}

pub fn make_merchant_destination_pda(
    merchant_id: u64,
    mint: &Pubkey,
    destination: &Pubkey,
    program_id: &Pubkey,
) -> PDAWithBump {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            MERCHANT_DESTINATION_SEED,
            &merchant_id.to_le_bytes(),
            mint.as_ref(),
            destination.as_ref(),
        ],
        program_id,
    );
    PDAWithBump { pubkey: pda, bump }
}

pub fn make_manager_pda(merchant_id: u64, program_id: &Pubkey) -> PDAWithBump {
    let (key, bump) = Pubkey::find_program_address(
        &[MERCHANT_MANAGER_SEED, &merchant_id.to_le_bytes()],
        program_id,
    );
    PDAWithBump { pubkey: key, bump }
}

pub fn setup_merchant_debitor_and_destination(
    ctx: &mut Context,
    merchant_id: u64,
    debitor_pk: Pubkey,
    mint_pk: &Pubkey,
    destination_owner: &Pubkey,
) -> (Pubkey, Pubkey, Pubkey) {
    setup_merchant_debitor_and_destination_with_program(
        ctx,
        merchant_id,
        debitor_pk,
        mint_pk,
        destination_owner,
        // Use default token program.
        TokenProgram::Token,
    )
}

pub fn setup_merchant_debitor_and_destination_with_program(
    ctx: &mut Context,
    merchant_id: u64,
    debitor_pk: Pubkey,
    mint_pk: &Pubkey,
    destination_owner: &Pubkey,
    token_program: TokenProgram,
) -> (Pubkey, Pubkey, Pubkey) {
    let debitor_pda = make_merchant_debitor_pda(merchant_id, &debitor_pk, mint_pk, &ctx.program_id);

    let destination_token_account =
        CreateAssociatedTokenAccountIdempotent::new(&mut ctx.svm, &ctx.payer_kp, mint_pk)
            .owner(destination_owner)
            .token_program_id(&token_program.program_id())
            .send()
            .unwrap();

    let destination_pda = make_merchant_destination_pda(
        merchant_id,
        mint_pk,
        &destination_token_account,
        &ctx.program_id,
    );

    // do the add manager call
    let debitor_accounts = AddOrUpdateMerchantDebitor {
        manager: ctx.merchant_manager_kp.pubkey(),
        manager_state: ctx.merchant_manager_state.pubkey,
        payer: ctx.payer_pk,
        debitor: debitor_pk,
        debitor_state: debitor_pda.pubkey,
        mint: *mint_pk,
        system_program: anchor_lang::system_program::ID,
    };
    let ix = create_add_or_update_merchant_debitor_instruction(
        ctx,
        &debitor_accounts,
        merchant_id,
        true,
    );
    let tx = create_transaction_with_payer_and_signers(
        ctx,
        &[ix],
        Some(&ctx.payer_pk),
        &[&ctx.payer_kp, &ctx.merchant_manager_kp],
    );
    submit_transaction(ctx, tx).unwrap();

    // do the add destination call
    let destination_accounts = AddOrUpdateMerchantDestination {
        admin: ctx.payer_pk,
        payer: ctx.payer_pk,
        state: ctx.bridge_cards_state.pubkey,
        destination_state: destination_pda.pubkey,
        destination_token_account,
        mint: *mint_pk,
        system_program: anchor_lang::system_program::ID,
    };
    let ix = create_add_or_update_merchant_destination_instruction(
        ctx,
        &destination_accounts,
        merchant_id,
        true,
    );
    let tx = create_transaction(ctx, &[ix]);
    submit_transaction(ctx, tx).unwrap();

    (
        debitor_pda.pubkey,
        destination_pda.pubkey,
        destination_token_account,
    )
}

pub fn create_add_or_update_merchant_debitor_instruction(
    ctx: &Context,
    accounts: &AddOrUpdateMerchantDebitor,
    merchant_id: u64,
    debitor_allowed: bool,
) -> Instruction {
    let ix_data = bridge_cards::instruction::AddOrUpdateMerchantDebitor {
        merchant_id,
        debitor_allowed,
    }
    .data();

    Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: ix_data,
    }
}

pub fn create_add_or_update_merchant_destination_instruction(
    ctx: &Context,
    accounts: &AddOrUpdateMerchantDestination,
    merchant_id: u64,
    destination_allowed: bool,
) -> Instruction {
    let ix_data = bridge_cards::instruction::AddOrUpdateMerchantDestination {
        merchant_id,
        destination_allowed,
    }
    .data();

    Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: ix_data,
    }
}

pub fn make_user_delegate_pda(
    merchant_id: u64,
    mint: &Pubkey,
    user_token_account: &Pubkey,
    program_id: &Pubkey,
) -> PDAWithBump {
    let (pda, bump) = Pubkey::find_program_address(
        &[
            USER_DELEGATE_SEED,
            &merchant_id.to_le_bytes(),
            mint.as_ref(),
            user_token_account.as_ref(),
        ],
        program_id,
    );
    PDAWithBump { pubkey: pda, bump }
}

pub fn create_add_or_update_user_delegate_instruction(
    ctx: &Context,
    accounts: &AddOrUpdateUserDelegate,
    merchant_id: u64,
    max_transfer_limit: u64,
    period_transfer_limit: u64,
    transfer_limit_period: u32,
) -> Instruction {
    let ix_data = bridge_cards::instruction::AddOrUpdateUserDelegate {
        merchant_id,
        max_transfer_limit,
        period_transfer_limit,
        transfer_limit_period,
    }
    .data();

    Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: ix_data,
    }
}

pub fn create_debit_user_instruction(
    ctx: &Context,
    accounts: &DebitUser,
    merchant_id: u64,
    amount: u64,
) -> Instruction {
    create_debit_user_instruction_with_program(
        ctx,
        accounts,
        merchant_id,
        amount,
        TokenProgram::Token,
    )
}

pub fn create_debit_user_instruction_with_program(
    ctx: &Context,
    accounts: &DebitUser,
    merchant_id: u64,
    amount: u64,
    _token_program: TokenProgram,
) -> Instruction {
    let ix_data = bridge_cards::instruction::DebitUser {
        merchant_id,
        amount,
    }
    .data();

    Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: ix_data,
    }
}

pub fn create_close_account_instruction(
    ctx: &Context,
    accounts: &bridge_cards::accounts::CloseAccount,
    input_seeds: Vec<Vec<u8>>,
) -> Instruction {
    let ix_data = bridge_cards::instruction::CloseAccount { input_seeds }.data();

    Instruction {
        program_id: ctx.program_id,
        accounts: accounts.to_account_metas(None),
        data: ix_data,
    }
}
