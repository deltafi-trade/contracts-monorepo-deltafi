#![cfg(feature = "test-bpf")]

mod utils;

use std::convert::TryInto;

use deltafi_swap::{
    error::SwapError,
    instruction::{initialize, InitializeData},
    math::{Decimal, TryDiv},
    processor::process,
    state::{OraclePriorityFlag, SwapType},
};
use solana_program_test::*;
use solana_sdk::{
    instruction::InstructionError,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use utils::*;

#[tokio::test]
async fn test_success() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(150_000);

    let swap_config = add_swap_config(&mut test);
    let sol_oracle = add_sol_oracle(&mut test);
    let srm_oracle = add_srm_oracle(&mut test);
    let srm_mint = add_srm_mint(&mut test);
    let (serum_market, serum_bids, serum_asks) = add_srm_sol_serum_market(&mut test);
    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let user_accounts_owner = Keypair::new();
    let sol_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        spl_token::native_mint::id(),
        None,
        &payer,
        user_accounts_owner.pubkey(),
        42_000_000_000,
    )
    .await;
    let srm_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        srm_mint.pubkey,
        Some(&srm_mint.authority),
        &payer,
        user_accounts_owner.pubkey(),
        800_000_000_000,
    )
    .await;

    let admin_fee_accounts = Keypair::new();
    let sol_admin_account = create_and_mint_to_token_account(
        &mut banks_client,
        spl_token::native_mint::id(),
        None,
        &payer,
        admin_fee_accounts.pubkey(),
        0,
    )
    .await;
    let srm_admin_account = create_and_mint_to_token_account(
        &mut banks_client,
        srm_mint.pubkey,
        Some(&srm_mint.authority),
        &payer,
        admin_fee_accounts.pubkey(),
        0,
    )
    .await;

    let admin = read_keypair_file("tests/fixtures/deltafi-owner.json").unwrap();
    // initialize a SOL-SRM pool with PYTH_ONLY
    let test_swap_info = TestSwapInfo::init(
        SwapType::Normal,
        &mut banks_client,
        &swap_config,
        &sol_oracle,
        &srm_oracle,
        spl_token::native_mint::id(),
        srm_mint.pubkey,
        sol_user_account,
        srm_user_account,
        sol_admin_account,
        srm_admin_account,
        &user_accounts_owner,
        &admin,
        serum_market,
        serum_bids,
        serum_asks,
        &payer,
        &SwapInitArgs {
            mid_price: 0, // ignored for stable swap
            slope: Decimal::one()
                .try_div(2)
                .unwrap()
                .to_scaled_val()
                .unwrap()
                .try_into()
                .unwrap(),
            token_a_amount: 42_000_000_000,
            token_b_amount: 800_000_000_000,
            oracle_priority_flags: OraclePriorityFlag::PYTH_ONLY.bits(),
        },
    )
    .await;

    test_swap_info.validate_state(&mut banks_client).await;

    // initialize a SRM-SOL pool with SERUM_ONLY
    let sol_user_account_serum_only = create_and_mint_to_token_account(
        &mut banks_client,
        spl_token::native_mint::id(),
        None,
        &payer,
        user_accounts_owner.pubkey(),
        42_000_000_000,
    )
    .await;
    let srm_user_account_serum_only = create_and_mint_to_token_account(
        &mut banks_client,
        srm_mint.pubkey,
        Some(&srm_mint.authority),
        &payer,
        user_accounts_owner.pubkey(),
        800_000_000_000,
    )
    .await;
    let test_swap_info_serum_only = TestSwapInfo::init(
        SwapType::Normal,
        &mut banks_client,
        &swap_config,
        &srm_oracle,
        &sol_oracle,
        srm_mint.pubkey,
        spl_token::native_mint::id(),
        srm_user_account_serum_only,
        sol_user_account_serum_only,
        srm_admin_account,
        sol_admin_account,
        &user_accounts_owner,
        &admin,
        serum_market,
        serum_bids,
        serum_asks,
        &payer,
        &SwapInitArgs {
            mid_price: 0, // ignored for stable swap
            slope: Decimal::one()
                .try_div(2)
                .unwrap()
                .to_scaled_val()
                .unwrap()
                .try_into()
                .unwrap(),
            token_a_amount: 800_000_000_000,
            token_b_amount: 42_000_000_000,
            oracle_priority_flags: OraclePriorityFlag::SERUM_ONLY.bits(),
        },
    )
    .await;

    test_swap_info_serum_only
        .validate_state(&mut banks_client)
        .await;
}

#[tokio::test]
async fn test_already_initialized() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    let swap_config = add_swap_config(&mut test);

    let user_account_owner = Keypair::new();
    let admin_account_owner = Keypair::new();

    let sol_oracle = add_sol_oracle(&mut test);
    let srm_oracle = add_srm_oracle(&mut test);
    let srm_mint = add_srm_mint(&mut test);

    let (serum_market, serum_bids, serum_asks) = add_srm_sol_serum_market(&mut test);

    let existing_swap = add_swap_info(
        SwapType::Stable,
        &mut test,
        &swap_config,
        &user_account_owner,
        &admin_account_owner,
        AddSwapInfoArgs {
            token_a_mint: spl_token::native_mint::id(),
            token_b_mint: srm_mint.pubkey,
            token_a_amount: 42_000_000_000,
            token_b_amount: 800_000_000_000,
            oracle_a: sol_oracle.price_pubkey,
            oracle_b: srm_oracle.price_pubkey,
            market_price: sol_oracle.price.try_div(srm_oracle.price).unwrap(),
            slope: Decimal::one().try_div(2).unwrap(),
            last_market_price: sol_oracle.price.try_div(srm_oracle.price).unwrap(),
            last_valid_market_price_slot: 0,
            serum_market,
            serum_bids,
            serum_asks,
            swap_out_limit_percentage: 10u8,
            oracle_priority_flags: 0,
        },
    );

    let (mut banks_client, payer, recent_blockhash) = test.start().await;

    let admin = read_keypair_file("tests/fixtures/deltafi-owner.json").unwrap();
    let mut transaction = Transaction::new_with_payer(
        &[initialize(
            deltafi_swap::id(),
            swap_config.pubkey,
            existing_swap.pubkey,
            existing_swap.authority,
            existing_swap.admin_fee_a_key,
            existing_swap.admin_fee_b_key,
            existing_swap.token_a,
            existing_swap.token_b,
            existing_swap.pool_mint,
            existing_swap.pool_token,
            sol_oracle.product_pubkey,
            sol_oracle.price_pubkey,
            srm_oracle.product_pubkey,
            srm_oracle.price_pubkey,
            admin.pubkey(),
            serum_market,
            serum_bids,
            serum_asks,
            InitializeData {
                nonce: existing_swap.nonce,
                mid_price: Decimal::from(20u64).to_scaled_val().unwrap(),
                slope: Decimal::one()
                    .try_div(2)
                    .unwrap()
                    .to_scaled_val()
                    .unwrap()
                    .try_into()
                    .unwrap(),
                token_a_decimals: 9u8,
                token_b_decimals: 9u8,
                token_a_amount: 42_000_000_000,
                token_b_amount: 800_000_000_000,
                oracle_priority_flags: 0,
            },
        )
        .unwrap()],
        Some(&payer.pubkey()),
    );

    transaction.sign(&[&payer, &existing_swap.keypair, &admin], recent_blockhash);
    assert_eq!(
        banks_client
            .process_transaction(transaction)
            .await
            .unwrap_err()
            .unwrap(),
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(SwapError::AlreadyInUse as u32)
        )
    );
}
