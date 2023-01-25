//! Util functions

use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hashv, pubkey::Pubkey,
};

use crate::{
    curve::SwapDirection,
    error::SwapError,
    pyth::PYTH_PROGRAM_ID,
    state::{FarmInfo, SwapInfo},
    SERUM_DEX_V3_PROGRAM_ID,
};
use safe_transmute::to_bytes::transmute_to_bytes;
use serum_dex::state::Market;
use std::{convert::identity, str::FromStr};

/// Utils funciton to validate certain condition.
pub fn validate(condition: bool, error_code: SwapError) -> ProgramResult {
    if !condition {
        return Err(error_code.into());
    }
    Ok(())
}

/// Validate the relationship between swap info and the config.
pub fn validate_swap_config_key(token_swap: &SwapInfo, config_key: &Pubkey) -> ProgramResult {
    validate(
        token_swap.config_key == *config_key,
        SwapError::InvalidMarketConfig,
    )
}

/// Validate the relationship between farm info and the config.
pub fn validate_farm_config_key(farm: &FarmInfo, config_key: &Pubkey) -> ProgramResult {
    if farm.config_key != *config_key {
        return Err(SwapError::InvalidMarketConfig.into());
    }
    Ok(())
}

/// Checks that the supplied program ID is the correct one for pyth program
pub fn check_pyth_program_account(pyth_program_id: &Pubkey) -> ProgramResult {
    let expected_pyth_program_id = Pubkey::from_str(PYTH_PROGRAM_ID).unwrap();
    if *pyth_program_id != expected_pyth_program_id {
        return Err(SwapError::InvalidPythProgramId.into());
    }
    Ok(())
}

/// Checks that the supplied program ID is the correct one for serum program
pub fn check_serum_program_id(serum_program_id: &Pubkey) -> ProgramResult {
    if *serum_program_id != Pubkey::from_str(SERUM_DEX_V3_PROGRAM_ID).unwrap() {
        return Err(SwapError::InvalidSerumProgramId.into());
    }
    Ok(())
}

/// check serum accounts
pub fn check_serum_accounts(
    serum_market_info: &AccountInfo,
    serum_bids_info: &AccountInfo,
    serum_asks_info: &AccountInfo,
    expected_serum_address: &Pubkey,
) -> ProgramResult {
    check_serum_program_id(serum_market_info.owner)?;
    check_serum_program_id(serum_bids_info.owner)?;
    check_serum_program_id(serum_asks_info.owner)?;

    let serum_combined_address = Pubkey::new(
        hashv(&[
            serum_market_info.key.as_ref(),
            serum_bids_info.key.as_ref(),
            serum_asks_info.key.as_ref(),
        ])
        .as_ref(),
    );
    validate(
        *expected_serum_address == serum_combined_address,
        SwapError::InvalidSerumMarketAccounts,
    )?;

    Ok(())
}

///  validate serum market mint address
pub fn validate_serum_market_mint_address(
    serum_market_info: &AccountInfo,
    token_a_mint: &Pubkey,
    token_b_mint: &Pubkey,
) -> ProgramResult {
    let market = Market::load(serum_market_info, serum_market_info.owner, true)?;
    validate(
        (token_a_mint.as_ref() == transmute_to_bytes(&identity(market.coin_mint)))
            && (token_b_mint.as_ref() == transmute_to_bytes(&identity(market.pc_mint))),
        SwapError::InvalidSerumMarketMintAddress,
    )?;
    Ok(())
}

/// Validate swap's source and destination token mint address
pub fn validate_swap_token_mint(
    direction: SwapDirection,
    token_a_mint: &Pubkey,
    token_b_mint: &Pubkey,
    source_mint: &Pubkey,
    destination_mint: &Pubkey,
) -> ProgramResult {
    match direction {
        SwapDirection::SellBase => {
            if (*source_mint != *token_a_mint) || (*destination_mint != *token_b_mint) {
                return Err(SwapError::IncorrectMint.into());
            }
        }
        SwapDirection::SellQuote => {
            if (*source_mint != *token_b_mint) || (*destination_mint != *token_a_mint) {
                return Err(SwapError::IncorrectMint.into());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    #![allow(clippy::ptr_offset_with_cast)]
    use super::*;
    use arrayref::mut_array_refs;
    use bytemuck::try_cast_slice_mut;
    use enumflags2::BitFlags;
    use serum_dex::state::{AccountFlag, MarketState, ACCOUNT_HEAD_PADDING, ACCOUNT_TAIL_PADDING};
    use std::mem::size_of;

    #[test]
    fn test_validate_swap_config_key() {
        let config_key = Pubkey::new_unique();
        let token_swap = SwapInfo {
            config_key,
            ..Default::default()
        };
        assert_eq!(validate_swap_config_key(&token_swap, &config_key), Ok(()));

        let bad_config_key = Pubkey::new_unique();
        assert_eq!(
            validate_swap_config_key(&token_swap, &bad_config_key),
            Err(SwapError::InvalidMarketConfig.into())
        );
    }

    #[test]
    fn test_validate_farm_config_key() {
        let config_key = Pubkey::new_unique();
        let farm = FarmInfo {
            config_key,
            ..Default::default()
        };
        assert_eq!(validate_farm_config_key(&farm, &config_key), Ok(()));

        let bad_config_key = Pubkey::new_unique();
        assert_eq!(
            validate_farm_config_key(&farm, &bad_config_key),
            Err(SwapError::InvalidMarketConfig.into())
        );
    }

    #[test]
    fn test_valid_pyth_program_id() {
        let pyth_program_id = Pubkey::from_str(PYTH_PROGRAM_ID).unwrap();
        assert_eq!(check_pyth_program_account(&pyth_program_id), Ok(()));

        let pyth_program_id = Pubkey::new_unique();
        assert_eq!(
            check_pyth_program_account(&pyth_program_id),
            Err(SwapError::InvalidPythProgramId.into())
        );
    }

    #[test]
    fn test_validate_swap_token_mint() {
        let token_a_mint = Pubkey::new_unique();
        let token_b_mint = Pubkey::new_unique();

        let source_mint = token_a_mint;
        let destination_mint = token_b_mint;
        assert_eq!(
            validate_swap_token_mint(
                SwapDirection::SellBase,
                &token_a_mint,
                &token_b_mint,
                &source_mint,
                &destination_mint
            ),
            Ok(())
        );

        let source_mint = token_b_mint;
        let destination_mint = token_a_mint;
        assert_eq!(
            validate_swap_token_mint(
                SwapDirection::SellBase,
                &token_a_mint,
                &token_b_mint,
                &source_mint,
                &destination_mint
            ),
            Err(SwapError::IncorrectMint.into())
        );

        let source_mint = token_b_mint;
        let destination_mint = token_a_mint;
        assert_eq!(
            validate_swap_token_mint(
                SwapDirection::SellQuote,
                &token_a_mint,
                &token_b_mint,
                &source_mint,
                &destination_mint
            ),
            Ok(())
        );

        let source_mint = token_a_mint;
        let destination_mint = token_b_mint;
        assert_eq!(
            validate_swap_token_mint(
                SwapDirection::SellQuote,
                &token_a_mint,
                &token_b_mint,
                &source_mint,
                &destination_mint
            ),
            Err(SwapError::IncorrectMint.into())
        );
    }

    #[test]
    fn test_validate_serum_market_mint_address() {
        let serum_market_key = Pubkey::new_unique();
        let serum_program_id = Pubkey::from_str(SERUM_DEX_V3_PROGRAM_ID).unwrap();
        let mut lamports = 10u64;
        // market account data: head_padding (5 Bytes) + MarketState + tail padding (7 bytes)
        const MARKET_DATA_LEN: usize = 5 + size_of::<MarketState>() + 7;
        let mut buffer = [0u8; MARKET_DATA_LEN];
        let (head, data, tail) = mut_array_refs![&mut buffer, 5; ..; 7];
        *head = *ACCOUNT_HEAD_PADDING;
        *tail = *ACCOUNT_TAIL_PADDING;

        let market_state: &mut [MarketState] = try_cast_slice_mut(data).unwrap();
        market_state[0].account_flags =
            BitFlags::bits(AccountFlag::Initialized | AccountFlag::Market);
        market_state[0].coin_mint = [6u64; 4]; // set coin_mint address
        market_state[0].pc_mint = [7u64; 4]; // set pc_mint address

        let serum_market_account = AccountInfo::new(
            &serum_market_key,
            false,
            false,
            &mut lamports,
            &mut buffer,
            &serum_program_id,
            true,
            1u64,
        );

        let token_a_mint = Pubkey::new_unique();
        let token_b_mint = Pubkey::new_unique();
        assert_eq!(
            validate_serum_market_mint_address(&serum_market_account, &token_a_mint, &token_b_mint,),
            Err(SwapError::InvalidSerumMarketMintAddress.into())
        );

        let token_a_mint = Pubkey::new(transmute_to_bytes(&identity([6u64; 4])));
        let token_b_mint = Pubkey::new(transmute_to_bytes(&identity([7u64; 4])));
        assert_eq!(
            validate_serum_market_mint_address(&serum_market_account, &token_a_mint, &token_b_mint,),
            Ok(())
        );

        assert_eq!(
            validate_serum_market_mint_address(&serum_market_account, &token_b_mint, &token_a_mint,),
            Err(SwapError::InvalidSerumMarketMintAddress.into())
        );
    }
}
