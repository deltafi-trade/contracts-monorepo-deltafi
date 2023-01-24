#![cfg(feature = "test-bpf")]

mod utils;

use deltafi_swap::{
    math::{Decimal, TryDiv},
    processor::process,
    state::SwapType,
};

use solana_program_test::*;
use solana_sdk::{
    program_pack::Pack,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use spl_token::state::{Account as Token, Mint};
use utils::*;

#[tokio::test]
async fn test_success() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(100_000);

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
            // ignored pyth, oracle for stable swap
            ..AddSwapInfoArgs::default()
        },
    );

    let (mut banks_client, payer, _recent_blockhash) = test.start().await;

    let usdc_withdraw_account = create_and_mint_to_token_account(
        &mut banks_client,
        usdc_mint.pubkey,
        Some(&usdc_mint.authority),
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    let usdt_withdraw_account = create_and_mint_to_token_account(
        &mut banks_client,
        usdt_mint.pubkey,
        Some(&usdt_mint.authority),
        &payer,
        user_account_owner.pubkey(),
        0,
    )
    .await;

    swap_info
        .withdraw(
            SwapType::Stable,
            &mut banks_client,
            &user_account_owner,
            usdc_withdraw_account,
            usdt_withdraw_account,
            swap_info.pool_token,
            2_000_000_000, // withdraw share amount
            2_000_000_000, // base_min_amount
            2_000_000_000, // quote_min_amount
            &payer,
        )
        .await;

    assert!(get_token_balance(&mut banks_client, usdc_withdraw_account).await < 2_000_000_000);
    assert!(get_token_balance(&mut banks_client, usdt_withdraw_account).await < 40_000_000_000);

    let pool_token_account = banks_client
        .get_account(swap_info.pool_token)
        .await
        .unwrap()
        .unwrap();
    let pool_token = Token::unpack(&pool_token_account.data[..]).unwrap();

    let pool_mint_account = banks_client
        .get_account(swap_info.pool_mint)
        .await
        .unwrap()
        .unwrap();
    let pool_mint = Mint::unpack(&pool_mint_account.data[..]).unwrap();

    assert_eq!(pool_token.amount, pool_mint.supply);
}
