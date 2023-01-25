#![cfg(feature = "test-bpf")]

mod utils;

use deltafi_swap::{
    math::{Decimal, TryDiv},
    processor::{get_referrer_data_pubkey, process},
    state::SwapType,
};

use solana_program_test::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use utils::*;

#[tokio::test]
async fn test_success() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(200_000);

    let swap_config = add_swap_config(&mut test);

    let usdc_mint = add_token_mint(&mut test, USDC_MINT, 6);
    let usdt_mint = add_token_mint(&mut test, USDT_MINT, 6);

    let user_account_owner = Keypair::new();
    let admin_account_owner = Keypair::new();

    let swap_info = add_swap_info(
        SwapType::Stable,
        &mut test,
        &swap_config,
        &user_account_owner,
        &admin_account_owner,
        AddSwapInfoArgs {
            token_a_mint: usdc_mint.pubkey,
            token_b_mint: usdt_mint.pubkey,
            token_a_amount: 4_200_000_000_000,
            token_b_amount: 80_000_000_000_000,
            oracle_a: Keypair::new().pubkey(),
            oracle_b: Keypair::new().pubkey(),
            market_price: Decimal::one(),
            slope: Decimal::one().try_div(2).unwrap(),
            last_market_price: Decimal::zero(), // ignored for stable swap
            last_valid_market_price_slot: 0,    // ignored for stable swap
            swap_out_limit_percentage: 10u8,
            ..AddSwapInfoArgs::default()
        },
    );

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let user_account_owner = Keypair::new();
    let usdc_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        usdc_mint.pubkey,
        Some(&usdc_mint.authority),
        &payer,
        user_account_owner.pubkey(),
        10_000_000_000,
    )
    .await;

    let usdt_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        usdt_mint.pubkey,
        Some(&usdt_mint.authority),
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
        .swap(
            SwapType::Stable,
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            usdc_user_account,
            usdc_mint.pubkey,
            usdt_user_account,
            usdt_mint.pubkey,
            deltafi_user_account,
            2_000_000_000,
            2_000_000_000,
            &payer,
            Some(user_referrer_data_pubkey),
            Some(deltafi_referrer_account),
        )
        .await;

    assert_eq!(
        get_token_balance(&mut banks_client, usdc_user_account).await,
        8_000_000_000,
    );
    assert!(get_token_balance(&mut banks_client, usdt_user_account).await > 2_000_000_000);
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
        .swap(
            SwapType::Stable,
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            usdc_user_account,
            usdc_mint.pubkey,
            usdt_user_account,
            usdt_mint.pubkey,
            deltafi_user_account,
            2_000_000,
            2_000_000,
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
    test.set_bpf_compute_max_units(200_000);

    let swap_config = add_swap_config(&mut test);

    let usdc_mint = add_new_mint(&mut test, 9);
    let usdt_mint = add_new_mint(&mut test, 6);

    let user_account_owner = Keypair::new();
    let admin_account_owner = Keypair::new();

    let swap_info = add_swap_info(
        SwapType::Stable,
        &mut test,
        &swap_config,
        &user_account_owner,
        &admin_account_owner,
        AddSwapInfoArgs {
            token_a_mint: usdc_mint.pubkey,
            token_b_mint: usdt_mint.pubkey,
            token_a_amount: 4_200_000_000_000,
            token_b_amount: 80_000_000_000,
            oracle_a: Keypair::new().pubkey(),
            oracle_b: Keypair::new().pubkey(),
            market_price: Decimal::one(),
            slope: Decimal::one().try_div(2).unwrap(),
            last_market_price: Decimal::zero(), // ignored for stable swap
            last_valid_market_price_slot: 0,    // ignored for stable swap
            swap_out_limit_percentage: 10u8,
            ..AddSwapInfoArgs::default()
        },
    );

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let user_account_owner = Keypair::new();
    let usdc_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        usdc_mint.pubkey,
        Some(&usdc_mint.authority),
        &payer,
        user_account_owner.pubkey(),
        10_000_000_000,
    )
    .await;

    let usdt_user_account = create_and_mint_to_token_account(
        &mut banks_client,
        usdt_mint.pubkey,
        Some(&usdt_mint.authority),
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
        .swap(
            SwapType::Stable,
            &mut banks_client,
            &swap_config,
            &user_account_owner,
            usdc_user_account,
            usdc_mint.pubkey,
            usdt_user_account,
            usdt_mint.pubkey,
            deltafi_user_account,
            2_000_000_000,
            2_000_000,
            &payer,
            Some(user_referrer_data_pubkey),
            Some(deltafi_referrer_account),
        )
        .await;

    assert_eq!(
        get_token_balance(&mut banks_client, usdc_user_account).await,
        8_000_000_000,
    );
    assert!(get_token_balance(&mut banks_client, usdt_user_account).await > 2_000_000);
    assert_eq!(
        get_token_balance(&mut banks_client, deltafi_user_account).await,
        1414
    );
    assert_eq!(
        get_token_balance(&mut banks_client, deltafi_referrer_account).await,
        70
    );
}
