//! Program state processor

#![allow(clippy::too_many_arguments)]

use std::{cmp::min, convert::TryInto, str::FromStr};

use solana_program::pubkey::PubkeyError;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    hash::hashv,
    instruction::Instruction,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    sysvar::{clock::Clock, rent::Rent, Sysvar},
};
use spl_token::{
    self,
    instruction::AuthorityType,
    state::{Account, Mint},
};

use crate::{
    admin::{is_admin, process_admin_instruction},
    curve::{InitPoolStateParams, PoolState, SwapDirection},
    error::SwapError,
    instruction::{
        DepositData, FarmDepositData, FarmInitializeData, FarmInstruction, FarmWithdrawData,
        InitializeData, InstructionType, StableInitializeData, StableSwapInstruction, SwapData,
        SwapInstruction, WithdrawData,
    },
    math::{Decimal, TryAdd, TryDiv, TryMul},
    pyth::{self, PriceStatus},
    state::{
        ConfigInfo, FarmInfo, FarmPosition, FarmUser, OraclePriorityFlag, SwapInfo, SwapType,
        UserReferrerData,
    },
    utils, DUMMY_REFERRER_ADDRESS, SERUM_DEX_V3_PROGRAM_ID,
};

use serum_dex::{critbit::SlabView, state::Market};

const SEED_REFRERRER: &str = "referrer";
const SEED_FARM_USER: &str = "farmUser";
const MAX_SEED_LEN: usize = 32;

/// Generate farm user address from owner, farm pool and program keys.
pub fn get_farm_user_pubkey(
    owner: &Pubkey,
    farm_pool_key: &Pubkey,
    program_id: &Pubkey,
) -> Result<Pubkey, PubkeyError> {
    let joint_key = format!("{}{}", SEED_FARM_USER, farm_pool_key);
    Pubkey::create_with_seed(owner, &joint_key.as_str()[0..MAX_SEED_LEN], program_id)
}

/// Generate referrer data address from owner, config key and program keys.
pub fn get_referrer_data_pubkey(
    owner: &Pubkey,
    config_key: &Pubkey,
    program_id: &Pubkey,
) -> Result<Pubkey, PubkeyError> {
    let joint_key = format!("{}{}", SEED_REFRERRER, config_key);
    Pubkey::create_with_seed(owner, &joint_key.as_str()[0..MAX_SEED_LEN], program_id)
}

fn validate_reward_token_accounts(
    config: &ConfigInfo,
    market_authority: &Pubkey,
    source_token: &Account,
    destination_token: &Account,
) -> ProgramResult {
    utils::validate(
        config.deltafi_mint == source_token.mint,
        SwapError::IncorrectMint,
    )?;
    utils::validate(
        source_token.owner == *market_authority,
        SwapError::InvalidOwner,
    )?;

    utils::validate(
        config.deltafi_mint == destination_token.mint,
        SwapError::IncorrectMint,
    )?;
    utils::validate(
        destination_token.owner != *market_authority,
        SwapError::InvalidOwner,
    )?;

    Ok(())
}

/// Processes an [Instruction](enum.Instruction.html).
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    match InstructionType::check(input) {
        Some(InstructionType::Admin) => process_admin_instruction(program_id, accounts, input),
        Some(InstructionType::Swap) => process_swap_instruction(program_id, accounts, input),
        Some(InstructionType::StableSwap) => {
            process_stable_swap_instruction(program_id, accounts, input)
        }
        Some(InstructionType::Farm) => process_farm_instruction(program_id, accounts, input),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn process_swap_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = SwapInstruction::unpack(input)?;
    match instruction {
        SwapInstruction::Initialize(InitializeData {
            nonce,
            slope,
            mid_price,
            token_a_decimals,
            token_b_decimals,
            token_a_amount,
            token_b_amount,
            oracle_priority_flags,
        }) => {
            msg!("Instruction: Initialize");
            process_initialize(
                program_id,
                nonce,
                slope,
                mid_price,
                token_a_decimals,
                token_b_decimals,
                token_a_amount,
                token_b_amount,
                oracle_priority_flags,
                accounts,
            )
        }
        SwapInstruction::Swap(SwapData {
            amount_in,
            minimum_amount_out,
        }) => {
            msg!("Instruction: Swap");
            process_swap(program_id, amount_in, minimum_amount_out, accounts)
        }
        SwapInstruction::SwapV2(SwapData {
            amount_in,
            minimum_amount_out,
        }) => {
            msg!("Instruction: SwapV2");
            process_swap_v2(program_id, amount_in, minimum_amount_out, accounts)
        }
        SwapInstruction::Deposit(DepositData {
            token_a_amount,
            token_b_amount,
            min_mint_amount,
        }) => {
            msg!("Instruction: Deposit");
            process_deposit(
                program_id,
                SwapType::Normal,
                token_a_amount,
                token_b_amount,
                min_mint_amount,
                accounts,
            )
        }
        SwapInstruction::Withdraw(WithdrawData {
            pool_token_amount,
            minimum_token_a_amount,
            minimum_token_b_amount,
        }) => {
            msg!("Instruction: Withdraw");
            process_withdraw(
                program_id,
                SwapType::Normal,
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
                accounts,
            )
        }
        SwapInstruction::SetReferrer => {
            msg!("Instruction: set referrer");
            process_set_referrer(program_id, accounts)
        }
    }
}

fn process_initialize(
    program_id: &Pubkey,
    nonce: u8,
    slope: u64,
    mid_price: u128,
    token_a_decimals: u8,
    token_b_decimals: u8,
    token_a_amount: u64,
    token_b_amount: u64,
    oracle_priority_flags: u8,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let admin_fee_a_info = next_account_info(account_info_iter)?;
    let admin_fee_b_info = next_account_info(account_info_iter)?;
    let token_a_info = next_account_info(account_info_iter)?;
    let token_b_info = next_account_info(account_info_iter)?;
    let pool_mint_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let pyth_a_product_info = next_account_info(account_info_iter)?;
    let pyth_a_price_info = next_account_info(account_info_iter)?;
    let pyth_b_product_info = next_account_info(account_info_iter)?;
    let pyth_b_price_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let serum_market_info = next_account_info(account_info_iter)?;
    let serum_bids_info = next_account_info(account_info_iter)?;
    let serum_asks_info = next_account_info(account_info_iter)?;
    let clock = &Clock::from_account_info(next_account_info(account_info_iter)?)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let token_program_info = next_account_info(account_info_iter)?;

    spl_token::check_program_account(token_program_info.key)?;

    if swap_info.owner != program_id || config_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Extract only fees and rewards to reduce stack usage
    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;
    let (fees, rewards) = (config.fees, config.rewards);

    let token_program_id = *token_program_info.key;
    let token_a = unpack_token_account(token_a_info, &token_program_id)?;
    let token_b = unpack_token_account(token_b_info, &token_program_id)?;

    // Pyth or Serum accounts verification
    let (pyth_a, pyth_b, serum_combined_address) =
        match OraclePriorityFlag::from_bits_truncate(oracle_priority_flags) {
            OraclePriorityFlag::PYTH_ONLY => {
                check_pyth_accounts(
                    pyth_a_product_info,
                    pyth_a_price_info,
                    &config.pyth_program_id,
                )?;
                check_pyth_accounts(
                    pyth_b_product_info,
                    pyth_b_price_info,
                    &config.pyth_program_id,
                )?;
                (
                    *pyth_a_price_info.key,
                    *pyth_b_price_info.key,
                    Pubkey::new(&[0u8; 32]),
                )
            }
            OraclePriorityFlag::SERUM_ONLY => {
                utils::check_serum_program_id(serum_market_info.owner)?;
                utils::check_serum_program_id(serum_bids_info.owner)?;
                utils::check_serum_program_id(serum_asks_info.owner)?;
                // validate serum market base/quote mint with token_a/token_b mint
                utils::validate_serum_market_mint_address(
                    serum_market_info,
                    &token_a.mint,
                    &token_b.mint,
                )?;
                (
                    Pubkey::new(&[0u8; 32]),
                    Pubkey::new(&[0u8; 32]),
                    Pubkey::new(
                        hashv(&[
                            serum_market_info.key.as_ref(),
                            serum_bids_info.key.as_ref(),
                            serum_asks_info.key.as_ref(),
                        ])
                        .as_ref(),
                    ),
                )
            }
            _ => {
                return Err(SwapError::UnsupportedOraclePriority.into());
            }
        };

    utils::validate(swap_info.is_signer, SwapError::InvalidSigner)?;

    assert_rent_exempt(rent, swap_info)?;
    assert_uninitialized::<SwapInfo>(swap_info)?;

    let swap_authority_signer_seeds = &[swap_info.key.as_ref(), &[nonce]];
    utils::validate(
        *authority_info.key
            == Pubkey::create_program_address(swap_authority_signer_seeds, program_id)?,
        SwapError::InvalidProgramAddress,
    )?;

    // This block does validation and release the memory on stack.
    {
        let destination = unpack_token_account(destination_info, &token_program_id)?;
        let pool_mint = unpack_mint(pool_mint_info, &token_program_id)?;
        let admin_fee_key_a = unpack_token_account(admin_fee_a_info, &token_program_id)?;
        let admin_fee_key_b = unpack_token_account(admin_fee_b_info, &token_program_id)?;

        utils::validate(
            *authority_info.key == token_a.owner,
            SwapError::InvalidOwner,
        )?;
        utils::validate(
            *authority_info.key == token_b.owner,
            SwapError::InvalidOwner,
        )?;

        utils::validate(
            *authority_info.key != destination.owner,
            SwapError::InvalidOutputOwner,
        )?;
        utils::validate(
            *authority_info.key != admin_fee_key_a.owner,
            SwapError::InvalidOutputOwner,
        )?;
        utils::validate(
            *authority_info.key != admin_fee_key_b.owner,
            SwapError::InvalidOutputOwner,
        )?;

        utils::validate(token_a.mint != token_b.mint, SwapError::RepeatedMint)?;
        utils::validate(
            token_a.mint == admin_fee_key_a.mint,
            SwapError::InvalidAdmin,
        )?;
        utils::validate(
            token_b.mint == admin_fee_key_b.mint,
            SwapError::InvalidAdmin,
        )?;

        utils::validate(
            token_a.amount == token_a_amount,
            SwapError::InconsistentInitialPoolTokenBalance,
        )?;
        utils::validate(
            token_b.amount == token_b_amount,
            SwapError::InconsistentInitialPoolTokenBalance,
        )?;

        utils::validate(!token_a.delegate.is_some(), SwapError::InvalidDelegate)?;
        utils::validate(!token_b.delegate.is_some(), SwapError::InvalidDelegate)?;

        utils::validate(
            !token_a.close_authority.is_some(),
            SwapError::InvalidCloseAuthority,
        )?;
        utils::validate(
            !token_b.close_authority.is_some(),
            SwapError::InvalidCloseAuthority,
        )?;

        if pool_mint.mint_authority.is_some()
            && *authority_info.key != pool_mint.mint_authority.unwrap()
        {
            return Err(SwapError::InvalidOwner.into());
        }
        if pool_mint.freeze_authority.is_some() {
            return Err(SwapError::InvalidFreezeAuthority.into());
        }
        if pool_mint.supply != 0 {
            return Err(SwapError::InvalidSupply.into());
        }
    }

    if Decimal::from_scaled_val(slope as u128) > Decimal::one() {
        return Err(SwapError::InvalidSlope.into());
    }

    let (market_price, valid_slot) = get_market_price(
        oracle_priority_flags,
        pyth_a_price_info,
        pyth_b_price_info,
        clock,
        serum_market_info,
        serum_bids_info,
        serum_asks_info,
        token_a_decimals,
        token_b_decimals,
    )
    .unwrap_or_else(|_| (Decimal::from_scaled_val(mid_price), clock.slot));

    let mut pool_state = PoolState::new(InitPoolStateParams {
        market_price,
        slope: Decimal::from_scaled_val(slope.into()),
        base_reserve: Decimal::zero(),
        quote_reserve: Decimal::zero(),
        total_supply: 0,
        last_market_price: market_price,
        last_valid_market_price_slot: min(clock.slot, valid_slot),
    });
    pool_state.set_market_price(token_a_decimals, token_b_decimals, market_price)?;

    let (mint_amount, token_a_output, token_b_output) =
        pool_state.buy_shares(token_a.amount, token_b.amount)?;
    utils::validate(
        token_a_output == token_a.amount && token_b_output == token_b.amount,
        SwapError::CalculationFailure,
    )?;

    pool_state.check_reserve_amount(token_a.amount, token_b.amount)?;

    SwapInfo::pack(
        SwapInfo {
            is_initialized: true,
            is_paused: false,
            nonce,
            swap_type: SwapType::Normal,
            config_key: *config_info.key,
            token_a: *token_a_info.key,
            token_b: *token_b_info.key,
            pool_mint: *pool_mint_info.key,
            token_a_mint: token_a.mint,
            token_b_mint: token_b.mint,
            admin_fee_key_a: *admin_fee_a_info.key,
            admin_fee_key_b: *admin_fee_b_info.key,
            pyth_a,
            pyth_b,
            fees,
            rewards,
            pool_state,
            token_a_decimals,
            token_b_decimals,
            oracle_priority_flags,
            serum_combined_address,
            ..SwapInfo::default()
        },
        &mut swap_info.data.borrow_mut(),
    )?;

    token_mint_to(
        pool_mint_info.clone(),
        destination_info.clone(),
        authority_info.clone(),
        token_program_info.clone(),
        mint_amount,
        swap_authority_signer_seeds,
    )?;

    Ok(())
}

fn process_swap(
    program_id: &Pubkey,
    amount_in: u64,
    minimum_amount_out: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let market_authority_info = next_account_info(account_info_iter)?;
    let swap_authority_info = next_account_info(account_info_iter)?;
    let user_transfer_authority_info = next_account_info(account_info_iter)?;
    let source_info = next_account_info(account_info_iter)?;
    let swap_source_info = next_account_info(account_info_iter)?;
    let source_mint_info = next_account_info(account_info_iter)?;
    let swap_destination_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let destination_mint_info = next_account_info(account_info_iter)?;
    let reward_token_info = next_account_info(account_info_iter)?;
    let source_reward_token_info = next_account_info(account_info_iter)?;
    let admin_destination_info = next_account_info(account_info_iter)?;
    let pyth_a_price_info = next_account_info(account_info_iter)?;
    let pyth_b_price_info = next_account_info(account_info_iter)?;
    let clock = &Clock::get()?;
    let token_program_info = next_account_info(account_info_iter)?;

    spl_token::check_program_account(token_program_info.key)?;

    if swap_info.owner != program_id || config_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;

    utils::validate_swap_config_key(&token_swap, config_info.key)?;
    utils::validate(
        token_swap.swap_type == SwapType::Normal,
        SwapError::IncorrectSwapType,
    )?;

    if token_swap.is_paused {
        return Err(SwapError::IsPaused.into());
    }
    let swap_authority_signer_seeds = &[swap_info.key.as_ref(), &[token_swap.nonce]];
    if *swap_authority_info.key
        != Pubkey::create_program_address(swap_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    let market_authority_signer_seeds = &[config_info.key.as_ref(), &[config.bump_seed]];
    if *market_authority_info.key
        != Pubkey::create_program_address(market_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    if token_swap.pyth_a != *pyth_a_price_info.key || token_swap.pyth_b != *pyth_b_price_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if source_info.key == destination_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if source_info.key == swap_source_info.key || destination_info.key == swap_destination_info.key
    {
        return Err(SwapError::InvalidInput.into());
    }
    let swap_direction = if *swap_source_info.key == token_swap.token_a
        && *swap_destination_info.key == token_swap.token_b
    {
        SwapDirection::SellBase
    } else if *swap_source_info.key == token_swap.token_b
        && *swap_destination_info.key == token_swap.token_a
    {
        SwapDirection::SellQuote
    } else {
        return Err(SwapError::IncorrectSwapAccount.into());
    };

    match swap_direction {
        SwapDirection::SellQuote => utils::validate(
            token_swap.admin_fee_key_a == *admin_destination_info.key,
            SwapError::InvalidAdmin,
        )?,
        SwapDirection::SellBase => utils::validate(
            token_swap.admin_fee_key_b == *admin_destination_info.key,
            SwapError::InvalidAdmin,
        )?,
    }

    // verify source and dest mint address
    utils::validate_swap_token_mint(
        swap_direction,
        &token_swap.token_a_mint,
        &token_swap.token_b_mint,
        source_mint_info.key,
        destination_mint_info.key,
    )?;

    let source_mint = Mint::unpack(&source_mint_info.data.borrow())?;
    let destination_mint = Mint::unpack(&destination_mint_info.data.borrow())?;

    let source_decimals = source_mint.decimals;
    let destination_decimals = destination_mint.decimals;
    let (base_decimals, quote_decimals) = match swap_direction {
        SwapDirection::SellBase => (source_decimals, destination_decimals),
        SwapDirection::SellQuote => (destination_decimals, source_decimals),
    };

    let token_program_id = *token_program_info.key;
    let reward_token = unpack_token_account(reward_token_info, &token_program_id)?;
    let source_reward_token = unpack_token_account(source_reward_token_info, &token_program_id)?;
    validate_reward_token_accounts(
        &config,
        market_authority_info.key,
        &source_reward_token,
        &reward_token,
    )?;

    match get_market_price_from_pyth(pyth_a_price_info, pyth_b_price_info, clock) {
        Ok((market_price, valid_slot)) => {
            token_swap
                .pool_state
                .check_and_update_market_price_and_slot(market_price, valid_slot)?;

            token_swap
                .pool_state
                .set_market_price(base_decimals, quote_decimals, market_price)?;
        }
        Err(e) => {
            return Err(e);
        }
    }

    let receive_amount = token_swap
        .pool_state
        .get_out_amount(amount_in, swap_direction)?;
    let fees = &token_swap.fees;
    let trade_fee = fees.trade_fee(receive_amount)?;
    let admin_fee = fees.admin_trade_fee(trade_fee)?;
    let rewards = &token_swap.rewards;
    let amount_out = receive_amount
        .checked_sub(trade_fee)
        .ok_or(SwapError::CalculationFailure)?;

    token_swap.check_swap_out_amount(amount_out, swap_direction)?;
    if amount_out < minimum_amount_out {
        return Err(SwapError::ExceededSlippage.into());
    }

    // Token price is fluctuated and need verification.
    // To consider token price in reward calculation may not be a reliable solution.
    // The awarded amount can be adjusted by config setting if required.
    let amount_to_reward =
        rewards.trade_reward_u64(if swap_direction == SwapDirection::SellBase {
            amount_in
        } else {
            amount_out
        })?;

    // The actual token amount moving out of the pool is amount_out + admin_fee.
    token_swap
        .pool_state
        .swap(amount_in, amount_out + admin_fee, swap_direction)?;

    token_transfer(
        source_info.clone(),
        swap_source_info.clone(),
        user_transfer_authority_info.clone(),
        token_program_info.clone(),
        amount_in,
        &[],
    )?;
    token_transfer(
        swap_destination_info.clone(),
        destination_info.clone(),
        swap_authority_info.clone(),
        token_program_info.clone(),
        amount_out,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        swap_destination_info.clone(),
        admin_destination_info.clone(),
        swap_authority_info.clone(),
        token_program_info.clone(),
        admin_fee,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        source_reward_token_info.clone(),
        reward_token_info.clone(),
        market_authority_info.clone(),
        token_program_info.clone(),
        amount_to_reward,
        market_authority_signer_seeds,
    )?;

    let swap_source_token = unpack_token_account(swap_source_info, &token_program_id)?;
    let swap_dest_token = unpack_token_account(swap_destination_info, &token_program_id)?;
    if swap_direction == SwapDirection::SellBase {
        token_swap
            .pool_state
            .check_reserve_amount(swap_source_token.amount, swap_dest_token.amount)?;
    } else {
        token_swap
            .pool_state
            .check_reserve_amount(swap_dest_token.amount, swap_source_token.amount)?;
    }

    // Handle referral reward
    if let Some(user_referrer_data_info) = account_info_iter.next() {
        let referrer_token_info = next_account_info(account_info_iter)?;

        let source_token = unpack_token_account(source_info, &token_program_id)?;
        let expected_user_referrer_data_pubkey =
            get_referrer_data_pubkey(&source_token.owner, config_info.key, program_id)?;
        utils::validate(
            expected_user_referrer_data_pubkey == *user_referrer_data_info.key,
            SwapError::InvalidAccountOwner,
        )?;

        let user_referrer_data = UserReferrerData::unpack(&user_referrer_data_info.data.borrow())?;
        utils::validate(
            user_referrer_data.referrer == *referrer_token_info.key
                && user_referrer_data.config_key == *config_info.key,
            SwapError::InvalidAccountOwner,
        )?;

        // Dummy referrer is a placeholder to indicate that the user is not referred by anyone.
        // If it is given in the input, skip the referral reward distribution here.
        if user_referrer_data.referrer.to_string() != DUMMY_REFERRER_ADDRESS {
            let referral_reward = rewards.referral_reward(amount_to_reward)?;
            token_transfer(
                source_reward_token_info.clone(),
                referrer_token_info.clone(),
                market_authority_info.clone(),
                token_program_info.clone(),
                referral_reward,
                market_authority_signer_seeds,
            )?;
        }
    }

    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    Ok(())
}

fn process_swap_v2(
    program_id: &Pubkey,
    amount_in: u64,
    minimum_amount_out: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let market_authority_info = next_account_info(account_info_iter)?;
    let swap_authority_info = next_account_info(account_info_iter)?;
    let user_transfer_authority_info = next_account_info(account_info_iter)?;
    let source_info = next_account_info(account_info_iter)?;
    let swap_source_info = next_account_info(account_info_iter)?;
    let swap_destination_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let reward_token_info = next_account_info(account_info_iter)?;
    let source_reward_token_info = next_account_info(account_info_iter)?;
    let admin_destination_info = next_account_info(account_info_iter)?;
    let pyth_a_price_info = next_account_info(account_info_iter)?;
    let pyth_b_price_info = next_account_info(account_info_iter)?;
    let serum_market_info = next_account_info(account_info_iter)?;
    let serum_bids_info = next_account_info(account_info_iter)?;
    let serum_asks_info = next_account_info(account_info_iter)?;
    let clock = &Clock::get()?;
    let token_program_info = next_account_info(account_info_iter)?;

    spl_token::check_program_account(token_program_info.key)?;

    if swap_info.owner != program_id || config_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;

    utils::validate_swap_config_key(&token_swap, config_info.key)?;
    utils::validate(
        token_swap.swap_type == SwapType::Normal,
        SwapError::IncorrectSwapType,
    )?;

    if token_swap.is_paused {
        return Err(SwapError::IsPaused.into());
    }
    let swap_authority_signer_seeds = &[swap_info.key.as_ref(), &[token_swap.nonce]];
    if *swap_authority_info.key
        != Pubkey::create_program_address(swap_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    let market_authority_signer_seeds = &[config_info.key.as_ref(), &[config.bump_seed]];
    if *market_authority_info.key
        != Pubkey::create_program_address(market_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    if token_swap.pyth_a != *pyth_a_price_info.key || token_swap.pyth_b != *pyth_b_price_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if source_info.key == destination_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if source_info.key == swap_source_info.key || destination_info.key == swap_destination_info.key
    {
        return Err(SwapError::InvalidInput.into());
    }
    let swap_direction = if *swap_source_info.key == token_swap.token_a
        && *swap_destination_info.key == token_swap.token_b
    {
        SwapDirection::SellBase
    } else if *swap_source_info.key == token_swap.token_b
        && *swap_destination_info.key == token_swap.token_a
    {
        SwapDirection::SellQuote
    } else {
        return Err(SwapError::IncorrectSwapAccount.into());
    };

    match swap_direction {
        SwapDirection::SellQuote => utils::validate(
            token_swap.admin_fee_key_a == *admin_destination_info.key,
            SwapError::InvalidAdmin,
        )?,
        SwapDirection::SellBase => utils::validate(
            token_swap.admin_fee_key_b == *admin_destination_info.key,
            SwapError::InvalidAdmin,
        )?,
    }

    {
        // verify source and dest mint address
        let source_token = unpack_token_account(source_info, token_program_info.key)?;
        let destination_token = unpack_token_account(destination_info, token_program_info.key)?;

        utils::validate_swap_token_mint(
            swap_direction,
            &token_swap.token_a_mint,
            &token_swap.token_b_mint,
            &source_token.mint,
            &destination_token.mint,
        )?;
    }

    let token_program_id = *token_program_info.key;
    {
        let reward_token = unpack_token_account(reward_token_info, &token_program_id)?;
        let source_reward_token =
            unpack_token_account(source_reward_token_info, &token_program_id)?;
        validate_reward_token_accounts(
            &config,
            market_authority_info.key,
            &source_reward_token,
            &reward_token,
        )?;
        if !OraclePriorityFlag::from_bits_truncate(token_swap.oracle_priority_flags).is_pyth_only()
        {
            utils::check_serum_accounts(
                serum_market_info,
                serum_bids_info,
                serum_asks_info,
                &token_swap.serum_combined_address,
            )?;
            utils::validate_serum_market_mint_address(
                serum_market_info,
                &token_swap.token_a_mint,
                &token_swap.token_b_mint,
            )?;
        }
    }

    match get_market_price(
        token_swap.oracle_priority_flags,
        pyth_a_price_info,
        pyth_b_price_info,
        clock,
        serum_market_info,
        serum_bids_info,
        serum_asks_info,
        token_swap.token_a_decimals,
        token_swap.token_b_decimals,
    ) {
        Ok((market_price, valid_slot)) => {
            token_swap
                .pool_state
                .check_and_update_market_price_and_slot(market_price, valid_slot)?;

            token_swap.pool_state.set_market_price(
                token_swap.token_a_decimals,
                token_swap.token_b_decimals,
                market_price,
            )?;
        }
        Err(e) => {
            return Err(e);
        }
    }

    let receive_amount = token_swap
        .pool_state
        .get_out_amount(amount_in, swap_direction)?;
    let fees = &token_swap.fees;
    let trade_fee = fees.trade_fee(receive_amount)?;
    let admin_fee = fees.admin_trade_fee(trade_fee)?;
    let rewards = &token_swap.rewards;
    let amount_out = receive_amount
        .checked_sub(trade_fee)
        .ok_or(SwapError::CalculationFailure)?;
    token_swap.check_swap_out_amount(amount_out, swap_direction)?;
    if amount_out < minimum_amount_out {
        return Err(SwapError::ExceededSlippage.into());
    }

    // Token price is fluctuated and need verification.
    // To consider token price in reward calculation may not be a reliable solution.
    // The awarded amount can be adjusted by config setting if required.
    let amount_to_reward =
        rewards.trade_reward_u64(if swap_direction == SwapDirection::SellBase {
            amount_in
        } else {
            amount_out
        })?;

    // The actual token amount moving out of the pool is amount_out + admin_fee.
    token_swap
        .pool_state
        .swap(amount_in, amount_out + admin_fee, swap_direction)?;

    token_transfer(
        source_info.clone(),
        swap_source_info.clone(),
        user_transfer_authority_info.clone(),
        token_program_info.clone(),
        amount_in,
        &[],
    )?;
    token_transfer(
        swap_destination_info.clone(),
        destination_info.clone(),
        swap_authority_info.clone(),
        token_program_info.clone(),
        amount_out,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        swap_destination_info.clone(),
        admin_destination_info.clone(),
        swap_authority_info.clone(),
        token_program_info.clone(),
        admin_fee,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        source_reward_token_info.clone(),
        reward_token_info.clone(),
        market_authority_info.clone(),
        token_program_info.clone(),
        amount_to_reward,
        market_authority_signer_seeds,
    )?;

    let swap_source_token = unpack_token_account(swap_source_info, &token_program_id)?;
    let swap_dest_token = unpack_token_account(swap_destination_info, &token_program_id)?;
    if swap_direction == SwapDirection::SellBase {
        token_swap
            .pool_state
            .check_reserve_amount(swap_source_token.amount, swap_dest_token.amount)?;
    } else {
        token_swap
            .pool_state
            .check_reserve_amount(swap_dest_token.amount, swap_source_token.amount)?;
    }

    // Handle referral reward
    if let Some(user_referrer_data_info) = account_info_iter.next() {
        let referrer_token_info = next_account_info(account_info_iter)?;

        let source_token = unpack_token_account(source_info, &token_program_id)?;
        let expected_user_referrer_data_pubkey =
            get_referrer_data_pubkey(&source_token.owner, config_info.key, program_id)?;
        utils::validate(
            expected_user_referrer_data_pubkey == *user_referrer_data_info.key,
            SwapError::InvalidAccountOwner,
        )?;

        let user_referrer_data = UserReferrerData::unpack(&user_referrer_data_info.data.borrow())?;
        utils::validate(
            user_referrer_data.referrer == *referrer_token_info.key
                && user_referrer_data.config_key == *config_info.key,
            SwapError::InvalidAccountOwner,
        )?;

        // Dummy referrer is a placeholder to indicate that the user is not referred by anyone.
        // If it is given in the input, skip the referral reward distribution here.
        if user_referrer_data.referrer.to_string() != DUMMY_REFERRER_ADDRESS {
            let referral_reward = rewards.referral_reward(amount_to_reward)?;
            token_transfer(
                source_reward_token_info.clone(),
                referrer_token_info.clone(),
                market_authority_info.clone(),
                token_program_info.clone(),
                referral_reward,
                market_authority_signer_seeds,
            )?;
        }
    }

    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    Ok(())
}

fn process_deposit(
    program_id: &Pubkey,
    swap_type: SwapType,
    token_a_amount: u64,
    token_b_amount: u64,
    min_mint_amount: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let swap_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let user_transfer_authority_info = next_account_info(account_info_iter)?;
    let source_a_info = next_account_info(account_info_iter)?;
    let source_b_info = next_account_info(account_info_iter)?;
    let token_a_info = next_account_info(account_info_iter)?;
    let token_b_info = next_account_info(account_info_iter)?;
    let pool_mint_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;

    spl_token::check_program_account(token_program_info.key)?;

    if swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    utils::validate(
        token_swap.swap_type == swap_type,
        SwapError::IncorrectSwapType,
    )?;

    if token_swap.is_paused {
        return Err(SwapError::IsPaused.into());
    }
    let swap_authority_signer_seeds = &[swap_info.key.as_ref(), &[token_swap.nonce]];
    if *authority_info.key
        != Pubkey::create_program_address(swap_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    if *token_a_info.key != token_swap.token_a {
        return Err(SwapError::IncorrectSwapAccount.into());
    }
    if *token_b_info.key != token_swap.token_b {
        return Err(SwapError::IncorrectSwapAccount.into());
    }
    if *pool_mint_info.key != token_swap.pool_mint {
        return Err(SwapError::IncorrectMint.into());
    }
    if token_a_info.key == source_a_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if token_b_info.key == source_b_info.key {
        return Err(SwapError::InvalidInput.into());
    }

    let (pool_mint_amount, token_a_output, token_b_output) = token_swap
        .pool_state
        .buy_shares(token_a_amount, token_b_amount)?;

    if pool_mint_amount < min_mint_amount {
        return Err(SwapError::ExceededSlippage.into());
    }

    token_transfer(
        source_a_info.clone(),
        token_a_info.clone(),
        user_transfer_authority_info.clone(),
        token_program_info.clone(),
        token_a_output,
        &[],
    )?;
    token_transfer(
        source_b_info.clone(),
        token_b_info.clone(),
        user_transfer_authority_info.clone(),
        token_program_info.clone(),
        token_b_output,
        &[],
    )?;
    token_mint_to(
        pool_mint_info.clone(),
        destination_info.clone(),
        authority_info.clone(),
        token_program_info.clone(),
        pool_mint_amount,
        swap_authority_signer_seeds,
    )?;

    let token_a = unpack_token_account(token_a_info, token_program_info.key)?;
    let token_b = unpack_token_account(token_b_info, token_program_info.key)?;
    token_swap
        .pool_state
        .check_reserve_amount(token_a.amount, token_b.amount)?;

    let pool_mint = unpack_mint(pool_mint_info, token_program_info.key)?;
    token_swap.pool_state.check_mint_supply(pool_mint.supply)?;

    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    Ok(())
}

fn process_withdraw(
    program_id: &Pubkey,
    swap_type: SwapType,
    pool_token_amount: u64,
    minimum_token_a_amount: u64,
    minimum_token_b_amount: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let swap_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let user_transfer_authority_info = next_account_info(account_info_iter)?;
    let pool_mint_info = next_account_info(account_info_iter)?;
    let source_info = next_account_info(account_info_iter)?;
    let token_a_info = next_account_info(account_info_iter)?;
    let token_b_info = next_account_info(account_info_iter)?;
    let dest_token_a_info = next_account_info(account_info_iter)?;
    let dest_token_b_info = next_account_info(account_info_iter)?;
    let admin_fee_dest_a_info = next_account_info(account_info_iter)?;
    let admin_fee_dest_b_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    spl_token::check_program_account(token_program_info.key)?;

    if swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    utils::validate(token_swap.swap_type == swap_type, SwapError::InvalidAccount)?;

    let swap_authority_signer_seeds = &[swap_info.key.as_ref(), &[token_swap.nonce]];
    if *authority_info.key
        != Pubkey::create_program_address(swap_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    if *token_a_info.key != token_swap.token_a {
        return Err(SwapError::IncorrectSwapAccount.into());
    }
    if *token_b_info.key != token_swap.token_b {
        return Err(SwapError::IncorrectSwapAccount.into());
    }
    if token_a_info.key == dest_token_a_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if token_b_info.key == dest_token_b_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if *pool_mint_info.key != token_swap.pool_mint {
        return Err(SwapError::IncorrectMint.into());
    }
    if *admin_fee_dest_a_info.key != token_swap.admin_fee_key_a {
        return Err(SwapError::InvalidAdmin.into());
    }
    if *admin_fee_dest_b_info.key != token_swap.admin_fee_key_b {
        return Err(SwapError::InvalidAdmin.into());
    }

    let pool_mint = unpack_mint(pool_mint_info, token_program_info.key)?;
    if pool_mint.supply == 0 {
        return Err(SwapError::EmptySupply.into());
    }

    let (base_out_amount, quote_out_amount) = token_swap.pool_state.sell_shares(
        pool_token_amount,
        minimum_token_a_amount,
        minimum_token_b_amount,
    )?;

    let fees = &token_swap.fees;
    let withdraw_fee_base = fees.withdraw_fee(base_out_amount)?;
    let admin_fee_base = fees.admin_withdraw_fee(withdraw_fee_base)?;
    let base_out_amount = base_out_amount
        .checked_sub(withdraw_fee_base)
        .ok_or(SwapError::CalculationFailure)?;

    let withdraw_fee_quote = fees.withdraw_fee(quote_out_amount)?;
    let admin_fee_quote = fees.admin_withdraw_fee(withdraw_fee_quote)?;
    let quote_out_amount = quote_out_amount
        .checked_sub(withdraw_fee_quote)
        .ok_or(SwapError::CalculationFailure)?;

    token_swap.pool_state.collect_trade_fee(
        withdraw_fee_base - admin_fee_base,
        withdraw_fee_quote - admin_fee_quote,
    )?;

    token_transfer(
        token_a_info.clone(),
        dest_token_a_info.clone(),
        authority_info.clone(),
        token_program_info.clone(),
        base_out_amount,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        token_a_info.clone(),
        admin_fee_dest_a_info.clone(),
        authority_info.clone(),
        token_program_info.clone(),
        admin_fee_base,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        token_b_info.clone(),
        dest_token_b_info.clone(),
        authority_info.clone(),
        token_program_info.clone(),
        quote_out_amount,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        token_b_info.clone(),
        admin_fee_dest_b_info.clone(),
        authority_info.clone(),
        token_program_info.clone(),
        admin_fee_quote,
        swap_authority_signer_seeds,
    )?;
    token_burn(
        pool_mint_info.clone(),
        source_info.clone(),
        user_transfer_authority_info.clone(),
        token_program_info.clone(),
        pool_token_amount,
        &[],
    )?;

    let token_a = unpack_token_account(token_a_info, token_program_info.key)?;
    let token_b = unpack_token_account(token_b_info, token_program_info.key)?;
    token_swap
        .pool_state
        .check_reserve_amount(token_a.amount, token_b.amount)?;

    let pool_mint = unpack_mint(pool_mint_info, token_program_info.key)?;
    token_swap.pool_state.check_mint_supply(pool_mint.supply)?;

    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    Ok(())
}

fn process_set_referrer(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let owner_info = next_account_info(account_info_iter)?;
    let user_referrer_info = next_account_info(account_info_iter)?;
    let referrer_token_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let token_program_info = next_account_info(account_info_iter)?;

    spl_token::check_program_account(token_program_info.key)?;

    utils::validate(
        user_referrer_info.owner == program_id && config_info.owner == program_id,
        SwapError::InvalidAccountOwner,
    )?;

    let expected_user_referrer_data_pubkey =
        get_referrer_data_pubkey(owner_info.key, config_info.key, program_id)?;
    utils::validate(
        expected_user_referrer_data_pubkey == *user_referrer_info.key,
        SwapError::InvalidAccountOwner,
    )?;

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;

    // the valid address is either a valid DELFI token accout or the dummy referrer address
    if referrer_token_info.key.to_string() != DUMMY_REFERRER_ADDRESS {
        let referrer_token = unpack_token_account(referrer_token_info, token_program_info.key)?;
        utils::validate(
            referrer_token.mint == config.deltafi_mint,
            SwapError::ExpectedMint,
        )?;
        // The owner should not set the referrer to self.
        utils::validate(
            referrer_token.owner != *owner_info.key,
            SwapError::InvalidAccountOwner,
        )?;
    }

    assert_rent_exempt(rent, user_referrer_info)?;
    let mut user_referrer_data = assert_uninitialized::<UserReferrerData>(user_referrer_info)?;
    if !owner_info.is_signer {
        return Err(SwapError::InvalidSigner.into());
    }

    user_referrer_data.is_initialized = true;
    user_referrer_data.config_key = *config_info.key;
    user_referrer_data.owner = *owner_info.key;
    user_referrer_data.referrer = *referrer_token_info.key;

    UserReferrerData::pack(
        user_referrer_data,
        &mut user_referrer_info.data.borrow_mut(),
    )?;

    Ok(())
}

fn process_stable_swap_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = StableSwapInstruction::unpack(input)?;
    match instruction {
        StableSwapInstruction::Initialize(StableInitializeData {
            nonce,
            slope,
            token_a_decimals,
            token_b_decimals,
            token_a_amount,
            token_b_amount,
        }) => {
            msg!("Instruction: Stable Initialize");
            process_stable_initialize(
                program_id,
                nonce,
                slope,
                token_a_decimals,
                token_b_decimals,
                token_a_amount,
                token_b_amount,
                accounts,
            )
        }
        StableSwapInstruction::Swap(SwapData {
            amount_in,
            minimum_amount_out,
        }) => {
            msg!("Instruction: Stable Swap");
            process_stable_swap(program_id, amount_in, minimum_amount_out, accounts)
        }
        StableSwapInstruction::SwapV2(SwapData {
            amount_in,
            minimum_amount_out,
        }) => {
            msg!("Instruction: Stable SwapV2");
            process_stable_swap_v2(program_id, amount_in, minimum_amount_out, accounts)
        }
        StableSwapInstruction::Deposit(DepositData {
            token_a_amount,
            token_b_amount,
            min_mint_amount,
        }) => {
            msg!("Instruction: Stable Deposit");
            process_deposit(
                program_id,
                SwapType::Stable,
                token_a_amount,
                token_b_amount,
                min_mint_amount,
                accounts,
            )
        }
        StableSwapInstruction::Withdraw(WithdrawData {
            pool_token_amount,
            minimum_token_a_amount,
            minimum_token_b_amount,
        }) => {
            msg!("Instruction: Stable Withdraw");
            process_withdraw(
                program_id,
                SwapType::Stable,
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
                accounts,
            )
        }
    }
}

fn process_stable_initialize(
    program_id: &Pubkey,
    nonce: u8,
    slope: u64,
    token_a_decimals: u8,
    token_b_decimals: u8,
    token_a_amount: u64,
    token_b_amount: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let swap_authority_info = next_account_info(account_info_iter)?;
    let admin_fee_a_info = next_account_info(account_info_iter)?;
    let admin_fee_b_info = next_account_info(account_info_iter)?;
    let token_a_info = next_account_info(account_info_iter)?;
    let token_b_info = next_account_info(account_info_iter)?;
    let pool_mint_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let token_program_info = next_account_info(account_info_iter)?;

    spl_token::check_program_account(token_program_info.key)?;

    utils::validate(swap_info.is_signer, SwapError::InvalidSigner)?;

    assert_rent_exempt(rent, swap_info)?;
    assert_uninitialized::<SwapInfo>(swap_info)?;
    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    // Extract only fees and rewards to reduce stack usage
    let (fees, rewards) = {
        let config = ConfigInfo::unpack(&config_info.data.borrow())?;
        is_admin(&config.admin_key, admin_info)?;
        (config.fees, config.rewards)
    };

    let swap_authority_signer_seeds = &[swap_info.key.as_ref(), &[nonce]];
    utils::validate(
        *swap_authority_info.key
            == Pubkey::create_program_address(swap_authority_signer_seeds, program_id)?,
        SwapError::InvalidProgramAddress,
    )?;

    let token_program_id = *token_program_info.key;
    let token_a = unpack_token_account(token_a_info, &token_program_id)?;
    let token_b = unpack_token_account(token_b_info, &token_program_id)?;

    // This block does validation and release the memory on stack.
    {
        let destination = unpack_token_account(destination_info, &token_program_id)?;
        let pool_mint = unpack_mint(pool_mint_info, &token_program_id)?;
        let admin_fee_key_a = unpack_token_account(admin_fee_a_info, &token_program_id)?;
        let admin_fee_key_b = unpack_token_account(admin_fee_b_info, &token_program_id)?;

        utils::validate(
            *swap_authority_info.key == token_a.owner,
            SwapError::InvalidOwner,
        )?;
        utils::validate(
            *swap_authority_info.key == token_b.owner,
            SwapError::InvalidOwner,
        )?;

        utils::validate(
            *swap_authority_info.key != destination.owner,
            SwapError::InvalidOutputOwner,
        )?;
        utils::validate(
            *swap_authority_info.key != admin_fee_key_a.owner,
            SwapError::InvalidOutputOwner,
        )?;
        utils::validate(
            *swap_authority_info.key != admin_fee_key_b.owner,
            SwapError::InvalidOutputOwner,
        )?;
        utils::validate(token_a.mint != token_b.mint, SwapError::RepeatedMint)?;
        utils::validate(
            token_a.mint == admin_fee_key_a.mint,
            SwapError::InvalidAdmin,
        )?;
        utils::validate(
            token_b.mint == admin_fee_key_b.mint,
            SwapError::InvalidAdmin,
        )?;

        utils::validate(
            token_a.amount == token_a_amount,
            SwapError::InconsistentInitialPoolTokenBalance,
        )?;
        utils::validate(
            token_b.amount == token_b_amount,
            SwapError::InconsistentInitialPoolTokenBalance,
        )?;

        utils::validate(!token_a.delegate.is_some(), SwapError::InvalidDelegate)?;
        utils::validate(!token_b.delegate.is_some(), SwapError::InvalidDelegate)?;

        utils::validate(
            !token_a.close_authority.is_some(),
            SwapError::InvalidCloseAuthority,
        )?;
        utils::validate(
            !token_b.close_authority.is_some(),
            SwapError::InvalidCloseAuthority,
        )?;

        if pool_mint.mint_authority.is_some()
            && *swap_authority_info.key != pool_mint.mint_authority.unwrap()
        {
            return Err(SwapError::InvalidOwner.into());
        }
        if pool_mint.freeze_authority.is_some() {
            return Err(SwapError::InvalidFreezeAuthority.into());
        }
        if pool_mint.supply != 0 {
            return Err(SwapError::InvalidSupply.into());
        }
    }

    let mut pool_state = PoolState::new(InitPoolStateParams {
        market_price: Decimal::one(),
        slope: Decimal::from_scaled_val(slope.into()),
        base_reserve: Decimal::zero(),
        quote_reserve: Decimal::zero(),
        total_supply: 0,
        last_market_price: Decimal::one(),
        last_valid_market_price_slot: 0,
    });
    pool_state.set_market_price(token_a_decimals, token_b_decimals, Decimal::one())?;

    let (mint_amount, token_a_output, token_b_output) =
        pool_state.buy_shares(token_a.amount, token_b.amount)?;
    utils::validate(
        token_a_output == token_a.amount && token_b_output == token_b.amount,
        SwapError::CalculationFailure,
    )?;

    pool_state.check_reserve_amount(token_a.amount, token_b.amount)?;

    SwapInfo::pack(
        SwapInfo {
            is_initialized: true,
            is_paused: false,
            nonce,
            swap_type: SwapType::Stable,
            config_key: *config_info.key,
            token_a: *token_a_info.key,
            token_b: *token_b_info.key,
            pool_mint: *pool_mint_info.key,
            token_a_mint: token_a.mint,
            token_b_mint: token_b.mint,
            admin_fee_key_a: *admin_fee_a_info.key,
            admin_fee_key_b: *admin_fee_b_info.key,
            fees,
            rewards,
            pool_state,
            token_a_decimals,
            token_b_decimals,
            // stable swap use same data structure as swap
            // we set pyth price accounts to null by using default value
            ..SwapInfo::default()
        },
        &mut swap_info.data.borrow_mut(),
    )?;

    token_mint_to(
        pool_mint_info.clone(),
        destination_info.clone(),
        swap_authority_info.clone(),
        token_program_info.clone(),
        mint_amount,
        swap_authority_signer_seeds,
    )?;

    Ok(())
}

fn process_stable_swap(
    program_id: &Pubkey,
    amount_in: u64,
    minimum_amount_out: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let market_authority_info = next_account_info(account_info_iter)?;
    let swap_authority_info = next_account_info(account_info_iter)?;
    let user_transfer_authority_info = next_account_info(account_info_iter)?;
    let source_info = next_account_info(account_info_iter)?;
    let swap_source_info = next_account_info(account_info_iter)?;
    let source_mint_info = next_account_info(account_info_iter)?;
    let swap_destination_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let destination_mint_info = next_account_info(account_info_iter)?;
    let reward_token_info = next_account_info(account_info_iter)?;
    let source_reward_token_info = next_account_info(account_info_iter)?;
    let admin_destination_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;

    spl_token::check_program_account(token_program_info.key)?;

    if swap_info.owner != program_id || config_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;

    utils::validate_swap_config_key(&token_swap, config_info.key)?;
    utils::validate(
        token_swap.swap_type == SwapType::Stable,
        SwapError::IncorrectSwapType,
    )?;

    if token_swap.is_paused {
        return Err(SwapError::IsPaused.into());
    }
    let swap_authority_signer_seeds = &[swap_info.key.as_ref(), &[token_swap.nonce]];
    if *swap_authority_info.key
        != Pubkey::create_program_address(swap_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    let market_authority_signer_seeds = &[config_info.key.as_ref(), &[config.bump_seed]];
    if *market_authority_info.key
        != Pubkey::create_program_address(market_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    if source_info.key == destination_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if source_info.key == swap_source_info.key || destination_info.key == swap_destination_info.key
    {
        return Err(SwapError::InvalidInput.into());
    }
    let swap_direction = if *swap_source_info.key == token_swap.token_a
        && *swap_destination_info.key == token_swap.token_b
    {
        SwapDirection::SellBase
    } else if *swap_source_info.key == token_swap.token_b
        && *swap_destination_info.key == token_swap.token_a
    {
        SwapDirection::SellQuote
    } else {
        return Err(SwapError::IncorrectSwapAccount.into());
    };

    match swap_direction {
        SwapDirection::SellQuote => utils::validate(
            token_swap.admin_fee_key_a == *admin_destination_info.key,
            SwapError::InvalidAdmin,
        )?,
        SwapDirection::SellBase => utils::validate(
            token_swap.admin_fee_key_b == *admin_destination_info.key,
            SwapError::InvalidAdmin,
        )?,
    }

    // verify source and dest mint address
    utils::validate_swap_token_mint(
        swap_direction,
        &token_swap.token_a_mint,
        &token_swap.token_b_mint,
        source_mint_info.key,
        destination_mint_info.key,
    )?;

    let source_mint = Mint::unpack(&source_mint_info.data.borrow())?;
    let destination_mint = Mint::unpack(&destination_mint_info.data.borrow())?;

    let source_decimals = source_mint.decimals;
    let destination_decimals = destination_mint.decimals;
    let (base_decimals, quote_decimals) = match swap_direction {
        SwapDirection::SellBase => (source_decimals, destination_decimals),
        SwapDirection::SellQuote => (destination_decimals, source_decimals),
    };

    let token_program_id = *token_program_info.key;
    let reward_token = unpack_token_account(reward_token_info, &token_program_id)?;
    let source_reward_token = unpack_token_account(source_reward_token_info, &token_program_id)?;
    validate_reward_token_accounts(
        &config,
        market_authority_info.key,
        &source_reward_token,
        &reward_token,
    )?;

    // Set the price to 1 for stable swap.
    token_swap
        .pool_state
        .set_market_price(base_decimals, quote_decimals, Decimal::one())?;

    let receive_amount = token_swap
        .pool_state
        .get_out_amount(amount_in, swap_direction)?;
    let fees = &token_swap.fees;
    let trade_fee = fees.trade_fee(receive_amount)?;
    let admin_fee = fees.admin_trade_fee(trade_fee)?;
    let rewards = &token_swap.rewards;
    let amount_out = receive_amount
        .checked_sub(trade_fee)
        .ok_or(SwapError::CalculationFailure)?;

    token_swap.check_swap_out_amount(amount_out, swap_direction)?;
    if amount_out < minimum_amount_out {
        return Err(SwapError::ExceededSlippage.into());
    }

    let amount_to_reward =
        rewards.trade_reward_u64(if swap_direction == SwapDirection::SellBase {
            amount_in
        } else {
            amount_out
        })?;

    // The actual token amount moving out of the pool is amount_out + admin_fee.
    token_swap
        .pool_state
        .swap(amount_in, amount_out + admin_fee, swap_direction)?;

    token_transfer(
        source_info.clone(),
        swap_source_info.clone(),
        user_transfer_authority_info.clone(),
        token_program_info.clone(),
        amount_in,
        &[],
    )?;
    token_transfer(
        swap_destination_info.clone(),
        destination_info.clone(),
        swap_authority_info.clone(),
        token_program_info.clone(),
        amount_out,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        swap_destination_info.clone(),
        admin_destination_info.clone(),
        swap_authority_info.clone(),
        token_program_info.clone(),
        admin_fee,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        source_reward_token_info.clone(),
        reward_token_info.clone(),
        market_authority_info.clone(),
        token_program_info.clone(),
        amount_to_reward,
        market_authority_signer_seeds,
    )?;

    let swap_source_token = unpack_token_account(swap_source_info, &token_program_id)?;
    let swap_dest_token = unpack_token_account(swap_destination_info, &token_program_id)?;
    if swap_direction == SwapDirection::SellBase {
        token_swap
            .pool_state
            .check_reserve_amount(swap_source_token.amount, swap_dest_token.amount)?;
    } else {
        token_swap
            .pool_state
            .check_reserve_amount(swap_dest_token.amount, swap_source_token.amount)?;
    }

    // Handle referral reward
    if let Some(user_referrer_data_info) = account_info_iter.next() {
        let referrer_token_info = next_account_info(account_info_iter)?;

        let source_token = unpack_token_account(source_info, &token_program_id)?;
        let expected_user_referrer_data_pubkey =
            get_referrer_data_pubkey(&source_token.owner, config_info.key, program_id)?;
        utils::validate(
            expected_user_referrer_data_pubkey == *user_referrer_data_info.key,
            SwapError::InvalidAccountOwner,
        )?;

        let user_referrer_data = UserReferrerData::unpack(&user_referrer_data_info.data.borrow())?;
        utils::validate(
            user_referrer_data.referrer == *referrer_token_info.key
                && user_referrer_data.config_key == *config_info.key,
            SwapError::InvalidAccountOwner,
        )?;

        // Dummy referrer is a placeholder to indicate that the user is not referred by anyone.
        // If it is given in the input, skip the referral reward distribution here.
        if user_referrer_data.referrer.to_string() != DUMMY_REFERRER_ADDRESS {
            let referral_reward = rewards.referral_reward(amount_to_reward)?;
            token_transfer(
                source_reward_token_info.clone(),
                referrer_token_info.clone(),
                market_authority_info.clone(),
                token_program_info.clone(),
                referral_reward,
                market_authority_signer_seeds,
            )?;
        }
    }

    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    Ok(())
}

fn process_stable_swap_v2(
    program_id: &Pubkey,
    amount_in: u64,
    minimum_amount_out: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let market_authority_info = next_account_info(account_info_iter)?;
    let swap_authority_info = next_account_info(account_info_iter)?;
    let user_transfer_authority_info = next_account_info(account_info_iter)?;
    let source_info = next_account_info(account_info_iter)?;
    let swap_source_info = next_account_info(account_info_iter)?;
    let swap_destination_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let reward_token_info = next_account_info(account_info_iter)?;
    let source_reward_token_info = next_account_info(account_info_iter)?;
    let admin_destination_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;

    spl_token::check_program_account(token_program_info.key)?;

    if swap_info.owner != program_id || config_info.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;

    utils::validate_swap_config_key(&token_swap, config_info.key)?;
    utils::validate(
        token_swap.swap_type == SwapType::Stable,
        SwapError::IncorrectSwapType,
    )?;

    if token_swap.is_paused {
        return Err(SwapError::IsPaused.into());
    }
    let swap_authority_signer_seeds = &[swap_info.key.as_ref(), &[token_swap.nonce]];
    if *swap_authority_info.key
        != Pubkey::create_program_address(swap_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    let market_authority_signer_seeds = &[config_info.key.as_ref(), &[config.bump_seed]];
    if *market_authority_info.key
        != Pubkey::create_program_address(market_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    if source_info.key == destination_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    if source_info.key == swap_source_info.key || destination_info.key == swap_destination_info.key
    {
        return Err(SwapError::InvalidInput.into());
    }
    let swap_direction = if *swap_source_info.key == token_swap.token_a
        && *swap_destination_info.key == token_swap.token_b
    {
        SwapDirection::SellBase
    } else if *swap_source_info.key == token_swap.token_b
        && *swap_destination_info.key == token_swap.token_a
    {
        SwapDirection::SellQuote
    } else {
        return Err(SwapError::IncorrectSwapAccount.into());
    };

    match swap_direction {
        SwapDirection::SellQuote => utils::validate(
            token_swap.admin_fee_key_a == *admin_destination_info.key,
            SwapError::InvalidAdmin,
        )?,
        SwapDirection::SellBase => utils::validate(
            token_swap.admin_fee_key_b == *admin_destination_info.key,
            SwapError::InvalidAdmin,
        )?,
    }

    {
        // verify source and dest mint address
        let source_token = unpack_token_account(source_info, token_program_info.key)?;
        let destination_token = unpack_token_account(destination_info, token_program_info.key)?;

        utils::validate_swap_token_mint(
            swap_direction,
            &token_swap.token_a_mint,
            &token_swap.token_b_mint,
            &source_token.mint,
            &destination_token.mint,
        )?;
    }

    let token_program_id = *token_program_info.key;
    let reward_token = unpack_token_account(reward_token_info, &token_program_id)?;
    let source_reward_token = unpack_token_account(source_reward_token_info, &token_program_id)?;
    validate_reward_token_accounts(
        &config,
        market_authority_info.key,
        &source_reward_token,
        &reward_token,
    )?;

    // Set the price to 1 for stable swap.
    token_swap.pool_state.set_market_price(
        token_swap.token_a_decimals,
        token_swap.token_b_decimals,
        Decimal::one(),
    )?;

    let receive_amount = token_swap
        .pool_state
        .get_out_amount(amount_in, swap_direction)?;
    let fees = &token_swap.fees;
    let trade_fee = fees.trade_fee(receive_amount)?;
    let admin_fee = fees.admin_trade_fee(trade_fee)?;
    let rewards = &token_swap.rewards;
    let amount_out = receive_amount
        .checked_sub(trade_fee)
        .ok_or(SwapError::CalculationFailure)?;

    token_swap.check_swap_out_amount(amount_out, swap_direction)?;
    if amount_out < minimum_amount_out {
        return Err(SwapError::ExceededSlippage.into());
    }

    let amount_to_reward =
        rewards.trade_reward_u64(if swap_direction == SwapDirection::SellBase {
            amount_in
        } else {
            amount_out
        })?;

    // The actual token amount moving out of the pool is amount_out + admin_fee.
    token_swap
        .pool_state
        .swap(amount_in, amount_out + admin_fee, swap_direction)?;

    token_transfer(
        source_info.clone(),
        swap_source_info.clone(),
        user_transfer_authority_info.clone(),
        token_program_info.clone(),
        amount_in,
        &[],
    )?;
    token_transfer(
        swap_destination_info.clone(),
        destination_info.clone(),
        swap_authority_info.clone(),
        token_program_info.clone(),
        amount_out,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        swap_destination_info.clone(),
        admin_destination_info.clone(),
        swap_authority_info.clone(),
        token_program_info.clone(),
        admin_fee,
        swap_authority_signer_seeds,
    )?;
    token_transfer(
        source_reward_token_info.clone(),
        reward_token_info.clone(),
        market_authority_info.clone(),
        token_program_info.clone(),
        amount_to_reward,
        market_authority_signer_seeds,
    )?;

    let swap_source_token = unpack_token_account(swap_source_info, &token_program_id)?;
    let swap_dest_token = unpack_token_account(swap_destination_info, &token_program_id)?;
    if swap_direction == SwapDirection::SellBase {
        token_swap
            .pool_state
            .check_reserve_amount(swap_source_token.amount, swap_dest_token.amount)?;
    } else {
        token_swap
            .pool_state
            .check_reserve_amount(swap_dest_token.amount, swap_source_token.amount)?;
    }

    // Handle referral reward
    if let Some(user_referrer_data_info) = account_info_iter.next() {
        let referrer_token_info = next_account_info(account_info_iter)?;

        let source_token = unpack_token_account(source_info, &token_program_id)?;
        let expected_user_referrer_data_pubkey =
            get_referrer_data_pubkey(&source_token.owner, config_info.key, program_id)?;
        utils::validate(
            expected_user_referrer_data_pubkey == *user_referrer_data_info.key,
            SwapError::InvalidAccountOwner,
        )?;

        let user_referrer_data = UserReferrerData::unpack(&user_referrer_data_info.data.borrow())?;
        utils::validate(
            user_referrer_data.referrer == *referrer_token_info.key
                && user_referrer_data.config_key == *config_info.key,
            SwapError::InvalidAccountOwner,
        )?;

        // Dummy referrer is a placeholder to indicate that the user is not referred by anyone.
        // If it is given in the input, skip the referral reward distribution here.
        if user_referrer_data.referrer.to_string() != DUMMY_REFERRER_ADDRESS {
            let referral_reward = rewards.referral_reward(amount_to_reward)?;
            token_transfer(
                source_reward_token_info.clone(),
                referrer_token_info.clone(),
                market_authority_info.clone(),
                token_program_info.clone(),
                referral_reward,
                market_authority_signer_seeds,
            )?;
        }
    }

    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;

    Ok(())
}

fn process_farm_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = FarmInstruction::unpack(input)?;
    match instruction {
        FarmInstruction::Initialize(FarmInitializeData {
            fee_numerator,
            fee_denominator,
            rewards_numerator,
            rewards_denominator,
            bump_seed,
        }) => {
            msg!("Instruction: Farm initialize");
            process_farm_initialize(
                program_id,
                fee_numerator,
                fee_denominator,
                rewards_numerator,
                rewards_denominator,
                bump_seed,
                accounts,
            )
        }
        FarmInstruction::InitializeFarmUser => {
            msg!("Instruction: Farm user initialize");
            process_farm_user_initialize(program_id, accounts)
        }
        FarmInstruction::Claim => {
            msg!("Instruction: Farm claim");
            process_farm_claim(program_id, accounts)
        }
        FarmInstruction::Refresh => {
            msg!("Instruction: Farm refresh");
            // refresh instruction is removed
            Err(SwapError::InvalidInstruction.into())
        }
        FarmInstruction::Deposit(FarmDepositData { amount }) => {
            msg!("Instruction: Farm deposit");
            process_farm_deposit(program_id, amount, accounts)
        }
        FarmInstruction::Withdraw(FarmWithdrawData { amount }) => {
            msg!("Instruction: Farm withdraw");
            process_farm_withdraw(program_id, amount, accounts)
        }
    }
}

fn process_farm_initialize(
    program_id: &Pubkey,
    fee_numerator: u64,
    fee_denominator: u64,
    apr_numerator: u64,
    apr_denominator: u64,
    bump_seed: u8,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let farm_pool_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let pool_token_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;

    utils::validate(
        farm_pool_info.owner == program_id
            && config_info.owner == program_id
            && swap_info.owner == program_id,
        SwapError::InvalidAccountOwner,
    )?;

    utils::validate(farm_pool_info.is_signer, SwapError::InvalidSigner)?;
    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    if pool_token_info.owner != &spl_token::ID {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    assert_rent_exempt(rent, farm_pool_info)?;
    assert_uninitialized::<FarmInfo>(farm_pool_info)?;

    if *authority_info.key
        != Pubkey::create_program_address(&[farm_pool_info.key.as_ref(), &[bump_seed]], program_id)?
    {
        return Err(SwapError::InvalidAccountOwner.into());
    }
    let token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    let pool_token = unpack_token_account(pool_token_info, &spl_token::id())?;
    if pool_token.mint != token_swap.pool_mint {
        return Err(SwapError::IncorrectMint.into());
    }
    if pool_token.owner != *authority_info.key {
        return Err(SwapError::InvalidOwner.into());
    }
    if pool_token.delegate.is_some() {
        return Err(SwapError::InvalidDelegate.into());
    }
    if pool_token.close_authority.is_some() {
        return Err(SwapError::InvalidCloseAuthority.into());
    }

    FarmInfo::pack(
        FarmInfo {
            is_initialized: true,
            bump_seed,
            config_key: *config_info.key,
            pool_mint: token_swap.pool_mint,
            pool_token: *pool_token_info.key,
            reserved_amount: 0,
            fee_numerator,
            fee_denominator,
            apr_numerator,
            apr_denominator,
            ..FarmInfo::default()
        },
        &mut farm_pool_info.data.borrow_mut(),
    )?;

    Ok(())
}

fn process_farm_user_initialize(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let farm_pool_info = next_account_info(account_info_iter)?;
    let farm_user_info = next_account_info(account_info_iter)?;
    let owner_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let clock = &Clock::get()?;

    utils::validate(
        farm_user_info.owner == program_id
            && farm_pool_info.owner == program_id
            && config_info.owner == program_id,
        SwapError::InvalidAccountOwner,
    )?;

    let farm_info = FarmInfo::unpack(&farm_pool_info.data.borrow_mut())?;
    utils::validate_farm_config_key(&farm_info, config_info.key)?;

    let farm_user_pubkey = get_farm_user_pubkey(owner_info.key, farm_pool_info.key, program_id)?;
    utils::validate(
        *farm_user_info.key == farm_user_pubkey,
        SwapError::InvalidAccountOwner,
    )?;

    assert_rent_exempt(rent, farm_user_info)?;
    let mut farm_user = assert_uninitialized::<FarmUser>(farm_user_info)?;

    if !owner_info.is_signer {
        return Err(SwapError::InvalidSigner.into());
    }

    farm_user.init(
        *config_info.key,
        *farm_pool_info.key,
        *owner_info.key,
        FarmPosition::new(*farm_pool_info.key, clock.unix_timestamp)?,
    );
    FarmUser::pack(farm_user, &mut farm_user_info.data.borrow_mut())?;

    Ok(())
}

fn process_farm_deposit(
    program_id: &Pubkey,
    amount: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let farm_pool_info = next_account_info(account_info_iter)?;
    let user_transfer_authority_info = next_account_info(account_info_iter)?;
    let source_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let farm_user_info = next_account_info(account_info_iter)?;
    let farm_owner_info = next_account_info(account_info_iter)?;
    let clock = &Clock::get()?;
    let token_program_info = next_account_info(account_info_iter)?;

    utils::validate(
        farm_user_info.owner == program_id
            && farm_pool_info.owner == program_id
            && config_info.owner == program_id,
        SwapError::InvalidAccountOwner,
    )?;
    spl_token::check_program_account(token_program_info.key)?;

    let mut farm_user = FarmUser::unpack(&farm_user_info.data.borrow_mut()).unwrap();
    if farm_user.owner != *farm_owner_info.key {
        return Err(SwapError::InvalidOwner.into());
    }
    if !farm_owner_info.is_signer {
        return Err(SwapError::InvalidSigner.into());
    }

    let mut farm_info = FarmInfo::unpack(&farm_pool_info.data.borrow_mut())?;
    utils::validate_farm_config_key(&farm_info, config_info.key)?;
    let farm_user_pubkey =
        get_farm_user_pubkey(farm_owner_info.key, farm_pool_info.key, program_id)?;
    utils::validate(
        *farm_user_info.key == farm_user_pubkey,
        SwapError::InvalidAccountOwner,
    )?;

    if farm_info.pool_token != *destination_info.key {
        return Err(SwapError::InvalidInput.into());
    }
    let token_program_id = *token_program_info.key;
    let source_token = unpack_token_account(source_info, &token_program_id)?;
    let destination_token = unpack_token_account(destination_info, &token_program_id)?;
    if source_token.mint != destination_token.mint {
        return Err(SwapError::IncorrectMint.into());
    }
    if source_token.mint != farm_info.pool_mint {
        return Err(SwapError::IncorrectMint.into());
    }

    // calculate and refresh reward before deposit
    let apr = Decimal::from(farm_info.apr_numerator).try_div(farm_info.apr_denominator)?;
    farm_user
        .position
        .calc_and_update_rewards(apr, clock.unix_timestamp, true)?;
    farm_user.position.deposit(amount, clock.slot)?;
    FarmUser::pack(farm_user, &mut farm_user_info.data.borrow_mut())?;

    farm_info.deposit(amount)?;

    token_transfer(
        source_info.clone(),
        destination_info.clone(),
        user_transfer_authority_info.clone(),
        token_program_info.clone(),
        amount,
        &[],
    )?;

    let destination_token = unpack_token_account(destination_info, &token_program_id)?;
    farm_info.check_reserve_amount(destination_token.amount)?;
    FarmInfo::pack(farm_info, &mut farm_pool_info.data.borrow_mut())?;

    Ok(())
}

fn process_farm_withdraw(
    program_id: &Pubkey,
    amount: u64,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let farm_pool_info = next_account_info(account_info_iter)?;
    let farm_user_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let source_info = next_account_info(account_info_iter)?;
    let destination_info = next_account_info(account_info_iter)?;
    let farm_owner_info = next_account_info(account_info_iter)?;
    let clock = &Clock::get()?;
    let token_program_info = next_account_info(account_info_iter)?;

    utils::validate(
        farm_user_info.owner == program_id
            && farm_pool_info.owner == program_id
            && config_info.owner == program_id,
        SwapError::InvalidAccountOwner,
    )?;
    spl_token::check_program_account(token_program_info.key)?;

    let mut farm_user = FarmUser::unpack(&farm_user_info.data.borrow_mut())?;
    if farm_user.owner != *farm_owner_info.key {
        return Err(SwapError::InvalidOwner.into());
    }
    if !farm_owner_info.is_signer {
        return Err(SwapError::InvalidSigner.into());
    }

    let mut farm_info = FarmInfo::unpack(&farm_pool_info.data.borrow_mut())?;
    utils::validate_farm_config_key(&farm_info, config_info.key)?;

    let farm_user_pubkey =
        get_farm_user_pubkey(farm_owner_info.key, farm_pool_info.key, program_id)?;
    utils::validate(
        *farm_user_info.key == farm_user_pubkey,
        SwapError::InvalidAccountOwner,
    )?;

    let farm_authority_signer_seeds = &[farm_pool_info.key.as_ref(), &[farm_info.bump_seed]];
    if *authority_info.key
        != Pubkey::create_program_address(farm_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }
    let token_program_id = *token_program_info.key;
    let source_token = unpack_token_account(source_info, &token_program_id)?;
    let destination_token = unpack_token_account(destination_info, &token_program_id)?;
    if source_token.mint != destination_token.mint {
        return Err(SwapError::IncorrectMint.into());
    }
    if source_token.mint != farm_info.pool_mint {
        return Err(SwapError::IncorrectMint.into());
    }

    /* make sure source_info token account is farm_pool's token account */
    if farm_info.pool_token != *source_info.key {
        return Err(SwapError::InvalidInput.into());
    }

    // calculate and refresh reward before withdraw
    let apr = Decimal::from(farm_info.apr_numerator).try_div(farm_info.apr_denominator)?;
    farm_user
        .position
        .calc_and_update_rewards(apr, clock.unix_timestamp, true)?;

    farm_user.withdraw(amount, clock.slot)?;
    FarmUser::pack(farm_user, &mut farm_user_info.data.borrow_mut())?;

    farm_info.withdraw(amount)?;

    token_transfer(
        source_info.clone(),
        destination_info.clone(),
        authority_info.clone(),
        token_program_info.clone(),
        amount,
        farm_authority_signer_seeds,
    )?;

    let source_token = unpack_token_account(source_info, &token_program_id)?;
    farm_info.check_reserve_amount(source_token.amount)?;
    FarmInfo::pack(farm_info, &mut farm_pool_info.data.borrow_mut())?;
    Ok(())
}

fn process_farm_claim(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let farm_pool_info = next_account_info(account_info_iter)?;
    let farm_user_info = next_account_info(account_info_iter)?;
    let farm_owner_info = next_account_info(account_info_iter)?;
    let market_authority_info = next_account_info(account_info_iter)?;
    let claim_destination_info = next_account_info(account_info_iter)?;
    let claim_source_info = next_account_info(account_info_iter)?;
    let clock = &Clock::get()?;
    let token_program_info = next_account_info(account_info_iter)?;

    utils::validate(
        farm_user_info.owner == program_id
            && farm_pool_info.owner == program_id
            && config_info.owner == program_id,
        SwapError::InvalidAccountOwner,
    )?;
    spl_token::check_program_account(token_program_info.key)?;

    let farm_info = FarmInfo::unpack(&farm_pool_info.data.borrow_mut())?;
    utils::validate_farm_config_key(&farm_info, config_info.key)?;

    let farm_user_pubkey =
        get_farm_user_pubkey(farm_owner_info.key, farm_pool_info.key, program_id)?;
    utils::validate(
        *farm_user_info.key == farm_user_pubkey,
        SwapError::InvalidAccountOwner,
    )?;

    let mut farm_user = FarmUser::unpack(&farm_user_info.data.borrow_mut())?;
    if farm_user.config_key != *config_info.key {
        return Err(SwapError::InvalidOwner.into());
    }
    if farm_user.owner != *farm_owner_info.key {
        return Err(SwapError::InvalidOwner.into());
    }
    if !farm_owner_info.is_signer {
        return Err(SwapError::InvalidSigner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    let market_authority_signer_seeds = &[config_info.key.as_ref(), &[config.bump_seed]];
    if *market_authority_info.key
        != Pubkey::create_program_address(market_authority_signer_seeds, program_id)?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }

    if claim_destination_info.owner == market_authority_info.key {
        return Err(SwapError::InvalidOwner.into());
    }

    // calculate and refresh reward before deposit
    let apr = Decimal::from(farm_info.apr_numerator).try_div(farm_info.apr_denominator)?;
    farm_user
        .position
        .calc_and_update_rewards(apr, clock.unix_timestamp, false)?;

    let token_program_id = *token_program_info.key;
    let claim_destination = unpack_token_account(claim_destination_info, &token_program_id)?;
    let claim_source = unpack_token_account(claim_source_info, &token_program_id)?;
    validate_reward_token_accounts(
        &config,
        market_authority_info.key,
        &claim_source,
        &claim_destination,
    )?;

    let reward_amount = farm_user.claim()?;
    token_transfer(
        claim_source_info.clone(),
        claim_destination_info.clone(),
        market_authority_info.clone(),
        token_program_info.clone(),
        reward_amount,
        market_authority_signer_seeds,
    )?;

    // Handle referral reward
    if let Some(user_referrer_data_info) = account_info_iter.next() {
        let referrer_token_info = next_account_info(account_info_iter)?;

        let expected_user_referrer_data_pubkey =
            get_referrer_data_pubkey(&farm_user.owner, config_info.key, program_id)?;
        utils::validate(
            expected_user_referrer_data_pubkey == *user_referrer_data_info.key,
            SwapError::InvalidAccountOwner,
        )?;

        let user_referrer_data = UserReferrerData::unpack(&user_referrer_data_info.data.borrow())?;
        utils::validate(
            user_referrer_data.referrer == *referrer_token_info.key
                && user_referrer_data.config_key == *config_info.key,
            SwapError::InvalidAccountOwner,
        )?;

        // Dummy referrer is a placeholder to indicate that the user is not referred by anyone.
        // If it is given in the input, skip the referral reward distribution here.
        if user_referrer_data.referrer.to_string() != DUMMY_REFERRER_ADDRESS {
            let referral_reward = config.rewards.referral_reward(reward_amount)?;
            token_transfer(
                claim_source_info.clone(),
                referrer_token_info.clone(),
                market_authority_info.clone(),
                token_program_info.clone(),
                referral_reward,
                market_authority_signer_seeds,
            )?;
        }
    }

    FarmUser::pack(farm_user, &mut farm_user_info.data.borrow_mut())?;

    Ok(())
}

fn get_market_price_from_pyth(
    pyth_a_price_info: &AccountInfo,
    pyth_b_price_info: &AccountInfo,
    clock: &Clock,
) -> Result<(Decimal, u64), ProgramError> {
    let (price_a, slot_a) = get_pyth_price(pyth_a_price_info, clock)?;
    let (price_b, slot_b) = get_pyth_price(pyth_b_price_info, clock)?;
    let market_price = price_a.try_div(price_b)?;
    let valid_slot = min(slot_a, slot_b);

    Ok((market_price, valid_slot))
}

fn get_pyth_price(
    pyth_price_info: &AccountInfo,
    clock: &Clock,
) -> Result<(Decimal, u64), ProgramError> {
    // Each slot has minimum 400ms. Set the stale timeout to 4s.
    const STALE_AFTER_SLOTS_ELAPSED: u64 = 10;

    let pyth_price_data = pyth_price_info.try_borrow_data()?;
    let pyth_price = pyth::load::<pyth::Price>(&pyth_price_data)
        .map_err(|_| ProgramError::InvalidAccountData)?;

    if pyth_price.ptype != pyth::PriceType::Price {
        msg!("Pyth price type is invalid");
        return Err(SwapError::InvalidPythConfig.into());
    }

    if pyth_price.agg.status != PriceStatus::Trading {
        msg!("Pyth price is currently unavailable");
        return Err(SwapError::InvalidPythConfig.into());
    }

    // Too few Pyth data providers
    if pyth_price
        .comp
        .iter()
        .filter(|comp| comp.is_active())
        .count()
        < 3
    {
        msg!("Pyth price is not guaranteed");
        return Err(SwapError::InvalidPythConfig.into());
    }

    // Stale Pyth price data
    let slots_elapsed = clock
        .slot
        .checked_sub(pyth_price.valid_slot)
        .ok_or(SwapError::CalculationFailure)?;
    if slots_elapsed >= STALE_AFTER_SLOTS_ELAPSED {
        msg!("Pyth price is stale");
        return Err(SwapError::StalePythPrice.into());
    }

    let price: u64 = pyth_price.agg.price.try_into().map_err(|_| {
        msg!("Pyth price cannot be negative");
        SwapError::InvalidPythConfig
    })?;

    // Pyth confidence interval is larger than 2% of the price.
    // pyth_price.agg.conf is the confidence interval length of the pyth price.
    // The confident price range will be [price - pyth_price.agg.conf, price + pyth_price.agg.conf]
    // Example: price=100, conf=2, it means the interval is in [98, 102]
    if pyth_price.agg.conf > 0
        && price
            < pyth_price
                .agg
                .conf
                .checked_mul(50u64)
                .ok_or(SwapError::CalculationFailure)?
    {
        msg!("Pyth suggests market is volatile");
        return Err(SwapError::InconfidentPythPrice.into());
    }

    // Too volatile Pyth price
    if pyth_price.agg.price
        < pyth_price
            .prev_price
            .checked_sub(pyth_price.agg.price)
            .ok_or(SwapError::CalculationFailure)?
            .abs()
            .checked_mul(100i64)
            .ok_or(SwapError::CalculationFailure)?
    {
        msg!("Difference between two pyth price is larger than 1%");
        return Err(SwapError::UnstableMarketPrice.into());
    }

    let market_price = if pyth_price.expo >= 0 {
        let exponent = pyth_price
            .expo
            .try_into()
            .map_err(|_| SwapError::CalculationFailure)?;
        let zeros = 10u64
            .checked_pow(exponent)
            .ok_or(SwapError::CalculationFailure)?;
        Decimal::from(price).try_mul(zeros)?
    } else {
        let exponent = pyth_price
            .expo
            .checked_abs()
            .ok_or(SwapError::CalculationFailure)?
            .try_into()
            .map_err(|_| SwapError::CalculationFailure)?;
        let decimals = 10u64
            .checked_pow(exponent)
            .ok_or(SwapError::CalculationFailure)?;
        Decimal::from(price).try_div(decimals)?
    };

    Ok((market_price, min(clock.slot, pyth_price.valid_slot)))
}

fn calculate_serum_market_price(
    base_lot_size: u64,
    quote_lot_size: u64,
    bids_price_lots: u128,
    asks_price_lots: u128,
    base_decimals: u8,
    quote_decimals: u8,
) -> Result<Decimal, ProgramError> {
    // From https://github.com/project-serum/serum-ts/blob/master/packages/serum/src/market.ts
    // price_lots (u128) can be convert to a number in this way
    // price = (priceLots * quoteLotSize * baseSplTokenMultiplier) / (baseLotSize * quoteSplTokenMultiplier)

    let base_multiplier = 10u64
        .checked_pow(base_decimals as u32)
        .ok_or(SwapError::CalculationFailure)?;
    let quote_multiplier = 10u64
        .checked_pow(quote_decimals as u32)
        .ok_or(SwapError::CalculationFailure)?;

    let market_price_lots: Decimal = Decimal::from(bids_price_lots)
        .try_add(Decimal::from(asks_price_lots))?
        .try_div(2)?;

    let market_price_numerator =
        Decimal::from(quote_lot_size).try_mul(Decimal::from(base_multiplier))?;

    let market_price_denominator =
        Decimal::from(base_lot_size).try_mul(Decimal::from(quote_multiplier))?;

    let market_price: Decimal = market_price_lots
        .try_mul(market_price_numerator)?
        .try_div(market_price_denominator)?;
    Ok(market_price)
}

fn get_market_price_from_serum(
    serum_market_info: &AccountInfo,
    serum_bids_info: &AccountInfo,
    serum_asks_info: &AccountInfo,
    token_a_decimals: u8,
    token_b_decimals: u8,
    serum_program_id: &Pubkey,
) -> Result<Decimal, ProgramError> {
    utils::check_serum_program_id(serum_program_id)?;

    // The logic to get price from serum market orerbook
    // Market -> Slab -> Nodehandle -> AnyNode -> LeafNode -> OrderId -> priceLot -> price
    let market = Market::load(serum_market_info, serum_program_id, true)?;
    // serum_dex only have this mutable api to load bids/asks, which is a verified usage.
    let bids_slab_refmut = market.load_bids_mut(serum_bids_info)?;
    let asks_slab_refmut = market.load_asks_mut(serum_asks_info)?;

    let bids_node_handle = bids_slab_refmut
        .find_max()
        .ok_or(SwapError::InvalidSerumData)?;
    let asks_node_handle = asks_slab_refmut
        .find_min()
        .ok_or(SwapError::InvalidSerumData)?;

    let bids_anynode = bids_slab_refmut
        .get(bids_node_handle)
        .ok_or(SwapError::InvalidSerumData)?;
    let asks_anynode = asks_slab_refmut
        .get(asks_node_handle)
        .ok_or(SwapError::InvalidSerumData)?;

    let bids_leaf = bids_anynode.as_leaf().ok_or(SwapError::InvalidSerumData)?;
    let asks_leaf = asks_anynode.as_leaf().ok_or(SwapError::InvalidSerumData)?;

    let market_price: Decimal = calculate_serum_market_price(
        market.coin_lot_size,
        market.pc_lot_size,
        bids_leaf.order_id() >> 64, // priceLot = orderId >> 64
        asks_leaf.order_id() >> 64,
        token_a_decimals,
        token_b_decimals,
    )?;
    Ok(market_price)
}

fn get_market_price(
    oracle_priority_flags: u8,
    pyth_a_price_info: &AccountInfo,
    pyth_b_price_info: &AccountInfo,
    clock: &Clock,
    serum_market_info: &AccountInfo,
    serum_bids_info: &AccountInfo,
    serum_asks_info: &AccountInfo,
    token_a_decimals: u8,
    token_b_decimals: u8,
) -> Result<(Decimal, u64), ProgramError> {
    match OraclePriorityFlag::from_bits_truncate(oracle_priority_flags) {
        OraclePriorityFlag::PYTH_ONLY => {
            match get_market_price_from_pyth(pyth_a_price_info, pyth_b_price_info, clock) {
                Ok((market_price, valid_slot)) => Ok((market_price, valid_slot)),
                Err(e) => Err(e),
            }
        }
        OraclePriorityFlag::SERUM_ONLY => {
            match get_market_price_from_serum(
                serum_market_info,
                serum_bids_info,
                serum_asks_info,
                token_a_decimals,
                token_b_decimals,
                &Pubkey::from_str(SERUM_DEX_V3_PROGRAM_ID).unwrap(),
            ) {
                Ok(market_price) => Ok((market_price, clock.slot)),
                Err(e) => Err(e),
            }
        }
        _ => Err(SwapError::UnsupportedOraclePriority.into()),
    }
}

/// Assert and unpack account data
pub fn assert_uninitialized<T: Pack + IsInitialized>(
    account_info: &AccountInfo,
) -> Result<T, ProgramError> {
    let account: T = T::unpack_unchecked(&account_info.data.borrow())?;
    if account.is_initialized() {
        Err(SwapError::AlreadyInUse.into())
    } else {
        Ok(account)
    }
}

/// Check if the account has enough lamports to be rent to store state
pub fn assert_rent_exempt(rent: &Rent, account_info: &AccountInfo) -> ProgramResult {
    if !rent.is_exempt(account_info.lamports(), account_info.data_len()) {
        Err(SwapError::NotRentExempt.into())
    } else {
        Ok(())
    }
}

/// Unpacks a spl_token `Mint`.
pub fn unpack_mint(
    account_info: &AccountInfo,
    token_program_id: &Pubkey,
) -> Result<Mint, SwapError> {
    if account_info.owner != token_program_id {
        Err(SwapError::IncorrectTokenProgramId)
    } else {
        Mint::unpack(&account_info.data.borrow()).map_err(|_| SwapError::ExpectedMint)
    }
}

/// Issue a spl_token `Transfer` instruction.
#[inline(always)]
fn token_transfer<'a>(
    source: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    amount: u64,
    authority_signature_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    if &spl_token::ID != token_program.key {
        return Err(ProgramError::IncorrectProgramId);
    }

    let result = invoke_optionally_signed(
        &spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?,
        &[source, destination, authority, token_program],
        authority_signature_seeds,
    );
    result.map_err(|_| SwapError::TokenTransferFailed.into())
}

/// Issue a spl_token `MintTo` instruction.
fn token_mint_to<'a>(
    mint: AccountInfo<'a>,
    destination: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    amount: u64,
    authority_signer_seeds: &[&[u8]],
) -> Result<(), ProgramError> {
    if &spl_token::ID != token_program.key {
        return Err(ProgramError::IncorrectProgramId);
    }

    let result = invoke_optionally_signed(
        &spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?,
        &[mint, destination, authority, token_program],
        authority_signer_seeds,
    );
    result.map_err(|_| SwapError::TokenMintToFailed.into())
}

/// Issue a spl_token `Burn` instruction.
#[inline(always)]
fn token_burn<'a>(
    mint: AccountInfo<'a>,
    burn_account: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    amount: u64,
    authority_signer_seeds: &[&[u8]],
) -> ProgramResult {
    if &spl_token::ID != token_program.key {
        return Err(ProgramError::IncorrectProgramId);
    }

    let result = invoke_optionally_signed(
        &spl_token::instruction::burn(
            token_program.key,
            burn_account.key,
            mint.key,
            authority.key,
            &[],
            amount,
        )?,
        &[mint, burn_account, authority, token_program],
        authority_signer_seeds,
    );
    result.map_err(|_| SwapError::TokenBurnFailed.into())
}

/// Set account authority
pub fn set_authority<'a>(
    token_program: &AccountInfo<'a>,
    account_to_transfer_ownership: &AccountInfo<'a>,
    new_authority: Option<Pubkey>,
    authority_type: AuthorityType,
    owner: &AccountInfo<'a>,
) -> ProgramResult {
    if &spl_token::ID != token_program.key {
        return Err(ProgramError::IncorrectProgramId);
    }

    let ix = spl_token::instruction::set_authority(
        token_program.key,
        account_to_transfer_ownership.key,
        new_authority.as_ref(),
        authority_type,
        owner.key,
        &[],
    )?;
    invoke(
        &ix,
        &[
            account_to_transfer_ownership.clone(),
            owner.clone(),
            token_program.clone(),
        ],
    )?;
    Ok(())
}

/// Unpacks a spl_token `Account`.
pub fn unpack_token_account(
    account_info: &AccountInfo,
    token_program_id: &Pubkey,
) -> Result<Account, ProgramError> {
    if account_info.owner != token_program_id {
        Err(SwapError::IncorrectTokenProgramId.into())
    } else {
        spl_token::state::Account::unpack(&account_info.data.borrow())
            .map_err(|_| SwapError::ExpectedAccount.into())
    }
}

fn check_pyth_accounts(
    pyth_product_info: &AccountInfo,
    pyth_price_info: &AccountInfo,
    pyth_program_id: &Pubkey,
) -> ProgramResult {
    utils::check_pyth_program_account(pyth_program_id)?;
    if pyth_program_id != pyth_product_info.owner {
        return Err(SwapError::InvalidPythConfig.into());
    }
    if pyth_program_id != pyth_price_info.owner {
        return Err(SwapError::InvalidPythConfig.into());
    }

    let pyth_product_data = pyth_product_info.try_borrow_data()?;
    let pyth_product = pyth::load::<pyth::Product>(&pyth_product_data)
        .map_err(|_| ProgramError::InvalidAccountData)?;
    if pyth_product.magic != pyth::MAGIC {
        return Err(SwapError::InvalidPythConfig.into());
    }
    if pyth_product.ver != pyth::VERSION_2 {
        return Err(SwapError::InvalidPythConfig.into());
    }
    if pyth_product.atype != pyth::AccountType::Product as u32 {
        return Err(SwapError::InvalidPythConfig.into());
    }

    let pyth_price_pubkey_bytes: &[u8; 32] = pyth_price_info
        .key
        .as_ref()
        .try_into()
        .map_err(|_| SwapError::InvalidAccount)?;
    if &pyth_product.px_acc.val != pyth_price_pubkey_bytes {
        return Err(SwapError::InvalidPythConfig.into());
    }

    Ok(())
}

/// Invoke signed unless signers seeds are empty
#[inline(always)]
fn invoke_optionally_signed(
    instruction: &Instruction,
    account_infos: &[AccountInfo],
    authority_signer_seeds: &[&[u8]],
) -> ProgramResult {
    if authority_signer_seeds.is_empty() {
        invoke(instruction, account_infos)
    } else {
        invoke_signed(instruction, account_infos, &[authority_signer_seeds])
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pyth::PYTH_PROGRAM_ID;
    use bytemuck::{bytes_of_mut, from_bytes_mut};
    use std::str::FromStr;

    #[test]
    fn test_assert_rent_exempt() {
        let rent = Rent {
            lamports_per_byte_year: 100_000u64,
            exemption_threshold: 200f64,
            burn_percent: 18u8,
        };

        let account_key = Pubkey::new_unique();
        let owner_key = Pubkey::new_unique();
        let mut lamports_1 = 3_000_000_000u64;
        let mut lamports_2 = 3_000_000_000u64;
        let mut short_data = [0u8; 4];
        let mut long_data = [0u8; 40];

        let mut test_account_info = AccountInfo::new(
            &account_key,
            false,
            false,
            &mut lamports_1,
            &mut long_data,
            &owner_key,
            false,
            0u64,
        );

        assert_eq!(
            assert_rent_exempt(&rent, &test_account_info),
            Err(ProgramError::from(SwapError::NotRentExempt))
        );

        test_account_info = AccountInfo::new(
            &account_key,
            false,
            false,
            &mut lamports_2,
            &mut short_data,
            &owner_key,
            false,
            0u64,
        );

        assert!(assert_rent_exempt(&rent, &test_account_info).is_ok());
    }

    fn get_check_pyth_accounts_result(option: u8) -> ProgramResult {
        let pyth_program_id = if option == 8u8 {
            Pubkey::new_unique()
        } else {
            Pubkey::from_str(PYTH_PROGRAM_ID).unwrap()
        };
        let pyth_price_key = Pubkey::new_unique();
        let pyth_product_key = Pubkey::new_unique();
        let pyth_price_key_slice: &[u8; 32] = pyth_price_key.as_ref().try_into().unwrap();

        let mut lamport = 0u64;
        let mut pyth_product = pyth::Product {
            magic: if option == 4u8 { 0u32 } else { pyth::MAGIC },
            ver: if option == 5u8 { 0u32 } else { pyth::VERSION_2 },
            atype: if option == 6u8 {
                0u32
            } else {
                pyth::AccountType::Product as u32
            },
            size: 32u32,
            px_acc: pyth::AccKey {
                val: if option == 7u8 {
                    [0u8; 32]
                } else {
                    *pyth_price_key_slice
                },
            },
            attr: [0u8; pyth::PROD_ATTR_SIZE],
        };

        let mut idle_data = [0u8];
        let pyth_product_data = if option == 3u8 {
            &mut idle_data
        } else {
            bytes_of_mut(&mut pyth_product)
        };
        let pyth_product_info = AccountInfo::new(
            &pyth_product_key,
            false,
            false,
            &mut lamport,
            pyth_product_data,
            if option == 1u8 {
                &pyth_price_key
            } else {
                &pyth_program_id
            },
            false,
            0u64,
        );

        let mut lamport = 0u64;
        let mut pyth_price_data = [0u8];
        let pyth_price_info = AccountInfo::new(
            &pyth_price_key,
            false,
            false,
            &mut lamport,
            &mut pyth_price_data,
            if option == 2u8 {
                &pyth_product_key
            } else {
                &pyth_program_id
            },
            false,
            0u64,
        );

        check_pyth_accounts(&pyth_product_info, &pyth_price_info, &pyth_program_id)
    }

    #[test]
    fn test_check_pyth_accounts() {
        assert!(get_check_pyth_accounts_result(0u8).is_ok());
        assert_eq!(
            get_check_pyth_accounts_result(1u8),
            Err(ProgramError::from(SwapError::InvalidPythConfig))
        );
        assert_eq!(
            get_check_pyth_accounts_result(2u8),
            Err(ProgramError::from(SwapError::InvalidPythConfig))
        );

        assert_eq!(
            get_check_pyth_accounts_result(3u8),
            Err(ProgramError::InvalidAccountData)
        );
        assert_eq!(
            get_check_pyth_accounts_result(4u8),
            Err(ProgramError::from(SwapError::InvalidPythConfig))
        );
        assert_eq!(
            get_check_pyth_accounts_result(5u8),
            Err(ProgramError::from(SwapError::InvalidPythConfig))
        );
        assert_eq!(
            get_check_pyth_accounts_result(6u8),
            Err(ProgramError::from(SwapError::InvalidPythConfig))
        );
        assert_eq!(
            get_check_pyth_accounts_result(7u8),
            Err(ProgramError::from(SwapError::InvalidPythConfig))
        );
        assert_eq!(
            get_check_pyth_accounts_result(8u8),
            Err(ProgramError::from(SwapError::InvalidPythProgramId))
        );
    }

    fn get_get_pyth_price_result(option: u8) -> Result<(Decimal, u64), ProgramError> {
        let program_id = Pubkey::new_unique();
        let pyth_price_key = Pubkey::new_unique();
        let mut pyth_price_data_vec = vec![0u8];
        let pyth_price_data = [0u8; std::mem::size_of::<pyth::Price>() * 2];
        pyth_price_data_vec.extend_from_slice(&pyth_price_data);
        let mut idle_data = [0u8];

        let price_align = std::mem::align_of::<pyth::Price>();
        let shift = price_align - ((pyth_price_data_vec.as_ptr() as usize) % price_align);
        assert_eq!(
            (pyth_price_data_vec[shift..shift + price_align].as_ptr() as usize) % price_align,
            0
        );

        let pyth_price: &mut pyth::Price = from_bytes_mut(
            &mut pyth_price_data_vec[shift..shift + std::mem::size_of::<pyth::Price>()],
        );

        pyth_price.ptype = if option == 3u8 {
            pyth::PriceType::Unknown
        } else {
            pyth::PriceType::Price
        };
        pyth_price.agg = pyth::PriceInfo {
            price: if option == 6u8 { -2i64 } else { 120_000_000i64 },
            conf: if option == 7u8 {
                20_000_000u64
            } else {
                200_000u64
            },
            status: if option == 4u8 {
                pyth::PriceStatus::Halted
            } else {
                pyth::PriceStatus::Trading
            },
            corp_act: pyth::CorpAction::NoCorpAct,
            pub_slot: 10_000u64,
        };

        pyth_price.prev_price = if option == 8u8 {
            100_000_000i64
        } else {
            119_000_000i64
        };

        let pyth_active_price_agg = pyth::PriceInfo {
            price: 120_000_000i64,
            conf: 200_000u64,
            status: pyth::PriceStatus::Trading,
            corp_act: pyth::CorpAction::NoCorpAct,
            pub_slot: 10_000u64,
        };
        let mut pyth_inactive_price_agg = pyth_active_price_agg;
        pyth_inactive_price_agg.status = pyth::PriceStatus::Halted;
        let mut pyth_price_comp_vec = vec![];

        if option == 5u8 {
            let active_slice = [pyth::PriceComp::new(
                pyth::AccKey { val: [0u8; 32] },
                pyth_active_price_agg,
                pyth_active_price_agg,
            ); 2];
            let inactive_slice = [pyth::PriceComp::new(
                pyth::AccKey { val: [0u8; 32] },
                pyth_inactive_price_agg,
                pyth_inactive_price_agg,
            ); 30];

            pyth_price_comp_vec.extend_from_slice(&active_slice);
            pyth_price_comp_vec.extend_from_slice(&inactive_slice);
        } else {
            let active_slice = [pyth::PriceComp::new(
                pyth::AccKey { val: [0u8; 32] },
                pyth_active_price_agg,
                pyth_active_price_agg,
            ); 4];
            let inactive_slice = [pyth::PriceComp::new(
                pyth::AccKey { val: [0u8; 32] },
                pyth_inactive_price_agg,
                pyth_inactive_price_agg,
            ); 28];

            pyth_price_comp_vec.extend_from_slice(&active_slice);
            pyth_price_comp_vec.extend_from_slice(&inactive_slice);
        }

        pyth_price.comp.copy_from_slice(&pyth_price_comp_vec[0..32]);
        pyth_price.expo = if option == 1u8 { -2i32 } else { 2i32 };

        pyth_price.valid_slot = 150_000u64;

        let mut lamport = 0u64;
        let pyth_price_info = AccountInfo::new(
            &pyth_price_key,
            false,
            false,
            &mut lamport,
            if option == 2u8 {
                &mut idle_data
            } else {
                &mut pyth_price_data_vec[shift..shift + std::mem::size_of::<pyth::Price>()]
            },
            &program_id,
            false,
            0u64,
        );

        let mut clock = Clock {
            ..Default::default()
        };

        if option == 9u8 {
            clock.slot = 150_001u64 + 11;
        } else {
            clock.slot = 150_001u64;
        }

        get_pyth_price(&pyth_price_info, &clock)
    }

    #[test]
    fn test_get_pyth_price() {
        let ok_result = get_get_pyth_price_result(0u8);
        assert!(ok_result.is_ok());
        assert_eq!(
            ok_result.unwrap(),
            (Decimal::from(12_000_000_000u64), 150_000u64)
        );

        let ok_result_neg = get_get_pyth_price_result(1u8);
        assert!(ok_result_neg.is_ok());
        assert_eq!(
            ok_result_neg.unwrap(),
            (Decimal::from(1_200_000u64), 150_000u64)
        );

        assert_eq!(
            get_get_pyth_price_result(2u8),
            Err(ProgramError::InvalidAccountData)
        );
        assert_eq!(
            get_get_pyth_price_result(3u8),
            Err(ProgramError::from(SwapError::InvalidPythConfig))
        );
        assert_eq!(
            get_get_pyth_price_result(4u8),
            Err(ProgramError::from(SwapError::InvalidPythConfig))
        );
        assert_eq!(
            get_get_pyth_price_result(5u8),
            Err(ProgramError::from(SwapError::InvalidPythConfig))
        );
        assert_eq!(
            get_get_pyth_price_result(6u8),
            Err(ProgramError::from(SwapError::InvalidPythConfig))
        );
        assert_eq!(
            get_get_pyth_price_result(7u8),
            Err(ProgramError::from(SwapError::InconfidentPythPrice))
        );
        assert_eq!(
            get_get_pyth_price_result(8u8),
            Err(ProgramError::from(SwapError::UnstableMarketPrice))
        );
        assert_eq!(
            get_get_pyth_price_result(9u8),
            Err(ProgramError::from(SwapError::StalePythPrice))
        );
    }

    #[test]
    fn test_calculate_serum_market_price() {
        // (3600+3587)/2 * (100000*10^6) / (10*10^9) = 35935
        assert_eq!(
            calculate_serum_market_price(10, 100000, 3600, 3587, 6, 9).unwrap(),
            Decimal::from(35935u64)
        );
        // ((2+8)/2)*(1*10^2/10*10^2) = 0.5
        assert_eq!(
            calculate_serum_market_price(10, 1, 2, 8, 2, 2).unwrap(),
            Decimal::from(1u64).try_div(2u64).unwrap()
        );
        // ((24+26)/2)*(10000*10^6/10000*10^9) = 0.025
        assert_eq!(
            calculate_serum_market_price(10000, 10000, 24, 26, 6, 9).unwrap(),
            Decimal::from(25u64).try_div(1000u64).unwrap()
        );
    }
}
