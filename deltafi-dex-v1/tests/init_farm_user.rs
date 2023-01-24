#![cfg(feature = "test-bpf")]

mod utils;

use deltafi_swap::{
    math::{Decimal, TryDiv},
    processor::process,
    state::SwapType,
};

use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use utils::*;

#[tokio::test]
async fn test_success() {
    let mut test = ProgramTest::new("deltafi_swap", deltafi_swap::id(), processor!(process));

    // limit to track compute unit increase
    test.set_bpf_compute_max_units(40_000);

    let swap_config = add_swap_config(&mut test);

    let sol_oracle = add_sol_oracle(&mut test);
    let srm_oracle = add_srm_oracle(&mut test);
    let srm_mint = add_srm_mint(&mut test);

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
            market_price: Decimal::one(),
            slope: Decimal::one().try_div(2).unwrap(),
            last_market_price: Decimal::one(),
            last_valid_market_price_slot: 0,
            swap_out_limit_percentage: 10u8,
            ..AddSwapInfoArgs::default()
        },
    );

    let mut context = test.start_with_context().await;

    let fee_numerator = 1u64;
    let fee_denominator = 1u64;
    let rewards_numerator = 1u64;
    let rewards_denominator = 1u64;
    let test_farm_pool_info = TestFarmPoolInfo::init(
        &mut context.banks_client,
        &swap_config,
        &swap_info,
        &context.payer,
        fee_numerator,
        fee_denominator,
        rewards_numerator,
        rewards_denominator,
    )
    .await;

    let test_farm_user = TestFarmUser::init(
        &mut context.banks_client,
        swap_config.pubkey,
        test_farm_pool_info.farm_pool_key,
        &user_account_owner,
        &context.payer,
    )
    .await;

    test_farm_user
        .validate_state(&mut context.banks_client)
        .await;

    // Check initial state
    assert_eq!(
        get_token_balance(&mut context.banks_client, swap_info.pool_token).await,
        4_200_000_000_000u64,
    );
    assert_eq!(
        get_token_balance(
            &mut context.banks_client,
            test_farm_pool_info.farm_pool_token
        )
        .await,
        0,
    );
    test_farm_user
        .do_farm_deposit(
            &mut context.banks_client,
            &user_account_owner,
            swap_info.pool_token,
            test_farm_pool_info.farm_pool_token,
            1000u64,
            &context.payer,
        )
        .await;
    // Check state after deposit
    assert_eq!(
        get_token_balance(&mut context.banks_client, swap_info.pool_token).await,
        4_200_000_000_000 - 1000u64,
    );
    assert_eq!(
        get_token_balance(
            &mut context.banks_client,
            test_farm_pool_info.farm_pool_token
        )
        .await,
        1000u64,
    );

    let farm_user_state = test_farm_user.get_state(&mut context.banks_client).await;
    assert_eq!(farm_user_state.position.deposited_amount, 1000u64);

    // Change the slot, otherwise it will report flash loan attack
    context.warp_to_slot(5).unwrap();

    test_farm_user
        .do_farm_withdraw(
            &mut context.banks_client,
            &user_account_owner,
            swap_info.pool_token,
            test_farm_pool_info.farm_pool_token,
            test_farm_pool_info.authority,
            500u64,
            &context.payer,
        )
        .await;
    assert_eq!(
        get_token_balance(&mut context.banks_client, swap_info.pool_token).await,
        4_200_000_000_000 - 500u64,
    );
    assert_eq!(
        get_token_balance(
            &mut context.banks_client,
            test_farm_pool_info.farm_pool_token
        )
        .await,
        500u64,
    );
}
