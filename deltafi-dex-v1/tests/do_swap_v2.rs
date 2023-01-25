#![cfg(feature = "test-bpf")]

mod utils;

use deltafi_swap::{
    math::{Decimal, TryDiv},
    processor::{get_referrer_data_pubkey, process},
    state::{OraclePriorityFlag, SwapType},
};

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use utils::*;

#[tokio::test]
async fn test_success_pyth_only() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(400_000);

    let swap_config = add_swap_config(&mut test);

    let sol_oracle = add_sol_oracle(&mut test);
    let srm_oracle = add_srm_oracle(&mut test);
    let srm_mint = add_srm_mint(&mut test);
    let (serum_market, serum_bids, serum_asks) = add_srm_sol_serum_market(&mut test);
    let user_account_owner = Keypair::new();
    let admin_account_owner = Keypair::new();

    let swap_info = add_swap_info(
        SwapType::Normal,
        &mut test,
        &swap_config,
        &user_account_owner,
        &admin_account_owner,
        AddSwapInfoArgs {
            token_a_mint: spl_token::native_mint::id(),
            token_b_mint: srm_mint.pubkey,
            token_a_amount: 4_200_000_000_000,
            token_b_amount: 80_000_000_000_000,
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
            oracle_priority_flags: 0u8,
        },
    );

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let user_account_owner = Keypair::new();
    let sol_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        spl_token::native_mint::id(),
        None,
        &payer,
        user_account_owner.pubkey(),
        10_000_000_000,
    )
    .await;

    let srm_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        srm_mint.pubkey,
        Some(&srm_mint.authority),
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    let deltafi_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        swap_config.deltafi_mint,
        None,
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    let referrer_account = Keypair::new();
    let deltafi_referrer_account = create_and_mint_to_token_account(
        &mut banks_client,
        swap_config.deltafi_mint,
        None,
        &payer,
        referrer_account.pubkey(),
        0,
    )
    .await;

    let user_referrer_data_pubkey = get_referrer_data_pubkey(
        &user_account_owner.pubkey(),
        &swap_config.pubkey,
        &deltafi_swap::id(),
    )
    .unwrap();
    swap_info
        .set_referrer(
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            user_referrer_data_pubkey,
            deltafi_referrer_account,
            &payer,
        )
        .await;

    swap_info
        .swap_v2(
            SwapType::Normal,
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            sol_user_account,
            srm_user_account,
            deltafi_user_account,
            2_000_000_000,
            15_000_000_000,
            &payer,
            Some(user_referrer_data_pubkey),
            Some(deltafi_referrer_account),
        )
        .await;

    assert_eq!(
        get_token_balance(&mut banks_client, sol_user_account).await,
        8_000_000_000,
    );
    assert!(get_token_balance(&mut banks_client, srm_user_account).await > 15_000_000_000);
    assert_eq!(
        get_token_balance(&mut banks_client, deltafi_user_account).await,
        1414
    );
    assert_eq!(
        get_token_balance(&mut banks_client, deltafi_referrer_account).await,
        70
    );

    // Swap without referrer should still work.
    swap_info
        .swap_v2(
            SwapType::Normal,
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            sol_user_account,
            srm_user_account,
            deltafi_user_account,
            2_000_000,
            15_000_000,
            &payer,
            None,
            None,
        )
        .await;
}

#[tokio::test]
async fn test_success_serum_only() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(400_000);

    let swap_config = add_swap_config(&mut test);

    let sol_oracle = add_sol_oracle(&mut test);
    let srm_oracle = add_srm_oracle(&mut test);
    let srm_mint = add_srm_mint(&mut test);
    let (serum_market, serum_bids, serum_asks) = add_srm_sol_serum_market(&mut test);
    let user_account_owner = Keypair::new();
    let admin_account_owner = Keypair::new();

    // SERUM_ONLY pool: SRM-SOL
    // token_a is SRM, token_b is SOL
    let swap_info = add_swap_info(
        SwapType::Normal,
        &mut test,
        &swap_config,
        &user_account_owner,
        &admin_account_owner,
        AddSwapInfoArgs {
            token_a_mint: srm_mint.pubkey,
            token_b_mint: spl_token::native_mint::id(),
            token_a_amount: 80_000_000_000_000,
            token_b_amount: 4_200_000_000_000,
            oracle_a: srm_oracle.price_pubkey,
            oracle_b: sol_oracle.price_pubkey,
            market_price: Decimal::from(25u64).try_div(1000u64).unwrap(), //SRM-SOl price is 0.025
            slope: Decimal::one().try_div(2).unwrap(),
            last_market_price: Decimal::from(25u64).try_div(1000u64).unwrap(), //assume last SRM-SOl price is 0.025
            last_valid_market_price_slot: 0,
            serum_market,
            serum_bids,
            serum_asks,
            swap_out_limit_percentage: 10u8,
            oracle_priority_flags: OraclePriorityFlag::SERUM_ONLY.bits(),
        },
    );

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let user_account_owner = Keypair::new();
    let sol_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        spl_token::native_mint::id(),
        None,
        &payer,
        user_account_owner.pubkey(),
        10_000_000_000,
    )
    .await;

    let srm_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        srm_mint.pubkey,
        Some(&srm_mint.authority),
        &payer,
        user_account_owner.pubkey(),
        3_000_000_000,
    )
    .await;

    let deltafi_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        swap_config.deltafi_mint,
        None,
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    let referrer_account = Keypair::new();
    let deltafi_referrer_account = create_and_mint_to_token_account(
        &mut banks_client,
        swap_config.deltafi_mint,
        None,
        &payer,
        referrer_account.pubkey(),
        0,
    )
    .await;

    let user_referrer_data_pubkey = get_referrer_data_pubkey(
        &user_account_owner.pubkey(),
        &swap_config.pubkey,
        &deltafi_swap::id(),
    )
    .unwrap();
    swap_info
        .set_referrer(
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            user_referrer_data_pubkey,
            deltafi_referrer_account,
            &payer,
        )
        .await;

    swap_info
        .swap_v2(
            SwapType::Normal,
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            srm_user_account,
            sol_user_account,
            deltafi_user_account,
            2_000_000_000,
            100_000_000,
            &payer,
            Some(user_referrer_data_pubkey),
            Some(deltafi_referrer_account),
        )
        .await;

    assert_eq!(
        get_token_balance(&mut banks_client, srm_user_account).await,
        1_000_000_000,
    );
    assert!(get_token_balance(&mut banks_client, sol_user_account).await > 10_000_000_000);
    assert_eq!(
        get_token_balance(&mut banks_client, deltafi_user_account).await,
        1414
    );
    assert_eq!(
        get_token_balance(&mut banks_client, deltafi_referrer_account).await,
        70
    );

    // Swap without referrer should still work.
    swap_info
        .swap_v2(
            SwapType::Normal,
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            srm_user_account,
            sol_user_account,
            deltafi_user_account,
            2_000_000,
            100_000,
            &payer,
            None,
            None,
        )
        .await;
}

#[tokio::test]
async fn test_swap_with_different_decimals() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(400_000);

    let swap_config = add_swap_config(&mut test);

    let sol_oracle = add_sol_oracle(&mut test);
    let srm_oracle = add_srm_oracle(&mut test);
    let srm_mint = add_new_mint(&mut test, 6u8);
    let (serum_market, serum_bids, serum_asks) = add_srm_sol_serum_market(&mut test);
    let user_account_owner = Keypair::new();
    let admin_account_owner = Keypair::new();

    let swap_info = add_swap_info(
        SwapType::Normal,
        &mut test,
        &swap_config,
        &user_account_owner,
        &admin_account_owner,
        AddSwapInfoArgs {
            token_a_mint: spl_token::native_mint::id(),
            token_b_mint: srm_mint.pubkey,
            token_a_amount: 4_200_000_000_000,
            token_b_amount: 80_000_000_000,
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
            oracle_priority_flags: 0u8,
        },
    );

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let user_account_owner = Keypair::new();
    let sol_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        spl_token::native_mint::id(),
        None,
        &payer,
        user_account_owner.pubkey(),
        10_000_000_000,
    )
    .await;

    let srm_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        srm_mint.pubkey,
        Some(&srm_mint.authority),
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    let deltafi_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        swap_config.deltafi_mint,
        None,
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    let referrer_account = Keypair::new();
    let deltafi_referrer_account = create_and_mint_to_token_account(
        &mut banks_client,
        swap_config.deltafi_mint,
        None,
        &payer,
        referrer_account.pubkey(),
        0,
    )
    .await;

    let user_referrer_data_pubkey = get_referrer_data_pubkey(
        &user_account_owner.pubkey(),
        &swap_config.pubkey,
        &deltafi_swap::id(),
    )
    .unwrap();
    swap_info
        .set_referrer(
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            user_referrer_data_pubkey,
            deltafi_referrer_account,
            &payer,
        )
        .await;

    swap_info
        .swap_v2(
            SwapType::Normal,
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            sol_user_account,
            srm_user_account,
            deltafi_user_account,
            2_000_000_000,
            35_000_000,
            &payer,
            Some(user_referrer_data_pubkey),
            Some(deltafi_referrer_account),
        )
        .await;

    assert_eq!(
        get_token_balance(&mut banks_client, sol_user_account).await,
        8_000_000_000,
    );
    assert!(get_token_balance(&mut banks_client, srm_user_account).await > 35_000_000);
    assert_eq!(
        get_token_balance(&mut banks_client, deltafi_user_account).await,
        1414
    );
    assert_eq!(
        get_token_balance(&mut banks_client, deltafi_referrer_account).await,
        70
    );
}
