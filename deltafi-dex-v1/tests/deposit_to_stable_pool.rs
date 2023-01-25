#![cfg(feature = "test-bpf")]

mod utils;

use deltafi_swap::{
    math::{Decimal, TryDiv},
    processor::process,
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
            token_a_amount: 42_000_000_000,
            token_b_amount: 800_000_000_000,
            market_price: Decimal::one(),
            slope: Decimal::one().try_div(2).unwrap(),
            swap_out_limit_percentage: 10u8,
            // ignored pyth, oracle for stable swap
            ..AddSwapInfoArgs::default()
        },
    );

    let pool_owner = Keypair::new();

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let usdc_deposit_account = create_and_mint_to_token_account(
        &mut banks_client,
        usdc_mint.pubkey,
        Some(&usdc_mint.authority),
        &payer,
        pool_owner.pubkey(),
        10_000_000_000,
    )
    .await;

    let usdt_deposit_account = create_and_mint_to_token_account(
        &mut banks_client,
        usdt_mint.pubkey,
        Some(&usdt_mint.authority),
        &payer,
        pool_owner.pubkey(),
        200_000_000_000,
    )
    .await;

    let pool_token_account = create_and_mint_to_token_account(
        &mut banks_client,
        swap_info.pool_mint,
        None,
        &payer,
        pool_owner.pubkey(),
        0,
    )
    .await;

    swap_info
        .deposit(
            SwapType::Stable,
            &mut banks_client,
            &pool_owner,
            usdc_deposit_account,
            usdt_deposit_account,
            pool_token_account,
            8_000_000_000,
            160_000_000_000,
            0,
            &payer,
        )
        .await;

    assert_eq!(
        get_token_balance(&mut banks_client, usdc_deposit_account).await,
        2_000_000_000,
    );
    assert_eq!(
        get_token_balance(&mut banks_client, usdt_deposit_account).await,
        // The deposit amount is 800/42*8_000_000_000 = 152_380_952_380
        200_000_000_000 - 152_380_952_380,
    );
    assert!(get_token_balance(&mut banks_client, pool_token_account).await > 0);
    assert_eq!(
        get_token_balance(&mut banks_client, swap_info.token_a).await,
        50_000_000_000,
    );
    assert_eq!(
        get_token_balance(&mut banks_client, swap_info.token_b).await,
        // The deposit amount is 800/42*8_000_000_000 = 152_380_952_380
        800_000_000_000 + 152_380_952_380,
    );
}
