//! Module for processing admin-only instructions.

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
};

use spl_token::instruction::AuthorityType;

use crate::{
    error::SwapError,
    instruction::{AdminInitializeData, AdminInstruction, CommitNewAdmin, FarmRewards},
    processor::{assert_rent_exempt, assert_uninitialized, set_authority, unpack_token_account},
    state::{ConfigInfo, SwapInfo, PROGRAM_VERSION},
    state::{Decimal, FarmInfo, Fees, Rewards},
    utils,
};

/// Process admin instruction
pub fn process_admin_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = AdminInstruction::unpack(input)?;
    match instruction {
        AdminInstruction::Initialize(AdminInitializeData { fees, rewards }) => {
            msg!("AdminInstruction : Initialization");
            initialize(program_id, &fees, &rewards, accounts)
        }
        AdminInstruction::Pause => {
            msg!("Instruction: Pause");
            pause(program_id, accounts)
        }
        AdminInstruction::Unpause => {
            msg!("Instruction: Unpause");
            unpause(program_id, accounts)
        }
        AdminInstruction::SetFeeAccount => {
            msg!("Instruction: SetFeeAccount");
            set_fee_account(program_id, accounts)
        }
        AdminInstruction::CommitNewAdmin(CommitNewAdmin { new_admin_key }) => {
            msg!("Instruction: CommitNewAdmin");
            commit_new_admin(program_id, new_admin_key, accounts)
        }
        AdminInstruction::SetNewFees(new_fees) => {
            msg!("Instruction: SetNewFees");
            set_new_fees(program_id, &new_fees, accounts)
        }
        AdminInstruction::SetNewRewards(new_rewards) => {
            msg!("Instruction: SetRewardsInfo");
            set_new_rewards(program_id, &new_rewards, accounts)
        }
        AdminInstruction::SetFarmRewards(farm_rewards) => {
            msg!("Insturction: SetFarmRewards");
            set_farm_rewards(program_id, &farm_rewards, accounts)
        }
        AdminInstruction::SetSlope(slope) => {
            msg!("Insturction: SetSlope");
            set_slope(program_id, slope, accounts)
        }
        AdminInstruction::SetDecimals(token_a_decimals, token_b_decimals) => {
            msg!("Instruction: SetDecimals");
            set_decimals(program_id, token_a_decimals, token_b_decimals, accounts)
        }
        AdminInstruction::SetSwapLimit(swap_out_limit_percentage) => {
            msg!("Instruction: SetSwapLimit");
            set_swap_limit(program_id, swap_out_limit_percentage, accounts)
        }
    }
}

/// Access control for admin only instructions
#[inline(never)]
pub fn is_admin(expected_admin_key: &Pubkey, admin_account_info: &AccountInfo) -> ProgramResult {
    if expected_admin_key != admin_account_info.key {
        return Err(SwapError::Unauthorized.into());
    }

    if !admin_account_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

/// Initialize configuration
#[inline(never)]
fn initialize(
    program_id: &Pubkey,
    fees: &Fees,
    rewards: &Rewards,
    accounts: &[AccountInfo],
) -> ProgramResult {
    msg!("Start initilization");
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let market_autority_info = next_account_info(account_info_iter)?;
    let deltafi_mint_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let pyth_program_info = next_account_info(account_info_iter)?;
    let deltafi_token_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    if !admin_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    utils::check_pyth_program_account(pyth_program_info.key)?;

    utils::validate(config_info.is_signer, SwapError::InvalidSigner)?;
    assert_rent_exempt(rent, config_info)?;
    let mut config = assert_uninitialized::<ConfigInfo>(config_info)?;

    let (market_autority_key, bump_seed) =
        Pubkey::find_program_address(&[config_info.key.as_ref()], program_id);
    if &market_autority_key != market_autority_info.key {
        return Err(SwapError::InvalidProgramAddress.into());
    }

    let token_program_id = *token_program_info.key;
    spl_token::check_program_account(&token_program_id)?;

    let deltafi_token = unpack_token_account(deltafi_token_info, &token_program_id)?;
    utils::validate(
        *deltafi_mint_info.key == deltafi_token.mint,
        SwapError::IncorrectMint,
    )?;
    utils::validate(
        *market_autority_info.key == deltafi_token.owner,
        SwapError::InvalidOwner,
    )?;

    utils::validate(rewards.decimals <= 10, SwapError::InvalidTokenDecimals)?;

    config.version = PROGRAM_VERSION;
    config.bump_seed = bump_seed;
    config.admin_key = *admin_info.key;
    config.deltafi_mint = *deltafi_mint_info.key;
    config.pyth_program_id = *pyth_program_info.key;
    config.fees = Fees::new(fees);
    config.rewards = Rewards::new(rewards);
    config.deltafi_token = *deltafi_token_info.key;
    ConfigInfo::pack(config, &mut config_info.data.borrow_mut())?;
    Ok(())
}

/// Pause swap
#[inline(never)]
fn pause(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;

    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    utils::validate_swap_config_key(&token_swap, config_info.key)?;

    token_swap.is_paused = true;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Unpause swap
#[inline(never)]
fn unpause(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    utils::validate_swap_config_key(&token_swap, config_info.key)?;

    token_swap.is_paused = false;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Set fee account
#[inline(never)]
fn set_fee_account(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let new_fee_account_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;

    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    utils::validate_swap_config_key(&token_swap, config_info.key)?;

    if *authority_info.key
        != Pubkey::create_program_address(
            &[swap_info.key.as_ref(), &[token_swap.nonce]],
            program_id,
        )?
    {
        return Err(SwapError::InvalidProgramAddress.into());
    }

    let new_admin_fee_account = unpack_token_account(new_fee_account_info, token_program_info.key)?;

    if *authority_info.key != new_admin_fee_account.owner {
        return Err(SwapError::InvalidOwner.into());
    }
    if new_admin_fee_account.mint == token_swap.token_a_mint {
        token_swap.admin_fee_key_a = *new_fee_account_info.key;
    } else if new_admin_fee_account.mint == token_swap.token_b_mint {
        token_swap.admin_fee_key_b = *new_fee_account_info.key;
    } else {
        return Err(SwapError::IncorrectMint.into());
    }

    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Commit new admin (initiate admin transfer)
#[inline(never)]
fn commit_new_admin(
    program_id: &Pubkey,
    new_admin_key: Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;
    let deltafi_mint_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let mut config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    if config.deltafi_mint != *deltafi_mint_info.key {
        return Err(SwapError::IncorrectMint.into());
    }
    config.admin_key = new_admin_key;
    ConfigInfo::pack(config, &mut config_info.data.borrow_mut())?;

    set_authority(
        token_program_info,
        deltafi_mint_info,
        Some(new_admin_key),
        AuthorityType::FreezeAccount,
        admin_info,
    )?;

    Ok(())
}

/// Set new fees
#[inline(never)]
fn set_new_fees(program_id: &Pubkey, new_fees: &Fees, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    utils::validate_swap_config_key(&token_swap, config_info.key)?;

    token_swap.fees = Fees::new(new_fees);
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Set new rewards
#[inline(never)]
fn set_new_rewards(
    program_id: &Pubkey,
    new_rewards: &Rewards,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    utils::validate_swap_config_key(&token_swap, config_info.key)?;

    utils::validate(new_rewards.decimals <= 10, SwapError::InvalidTokenDecimals)?;

    token_swap.rewards = Rewards::new(new_rewards);
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Set new staking rewards
#[inline(never)]
fn set_farm_rewards(
    program_id: &Pubkey,
    farm_rewards: &FarmRewards,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let farm_pool_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || farm_pool_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;
    is_admin(&config.admin_key, admin_info)?;

    let mut farm_pool = FarmInfo::unpack(&farm_pool_info.data.borrow())?;
    utils::validate_farm_config_key(&farm_pool, config_info.key)?;

    farm_pool.apr_numerator = farm_rewards.apr_numerator;
    farm_pool.apr_denominator = farm_rewards.apr_denominator;
    FarmInfo::pack(farm_pool, &mut farm_pool_info.data.borrow_mut())?;

    Ok(())
}

/// Set new slope
#[inline(never)]
fn set_slope(program_id: &Pubkey, slope: u64, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;

    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    utils::validate_swap_config_key(&token_swap, config_info.key)?;

    token_swap.pool_state.slope = Decimal::from_scaled_val(slope as u128);
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

/// Set base and quote token decimals
#[inline(never)]
fn set_decimals(
    program_id: &Pubkey,
    token_a_decimals: u8,
    token_b_decimals: u8,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;

    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    utils::validate_swap_config_key(&token_swap, config_info.key)?;

    token_swap.token_a_decimals = token_a_decimals;
    token_swap.token_b_decimals = token_b_decimals;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

#[inline(never)]
fn set_swap_limit(
    program_id: &Pubkey,
    swap_out_limit_percentage: u8,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let swap_info = next_account_info(account_info_iter)?;
    let admin_info = next_account_info(account_info_iter)?;

    if config_info.owner != program_id || swap_info.owner != program_id {
        return Err(SwapError::InvalidAccountOwner.into());
    }

    let config = ConfigInfo::unpack(&config_info.data.borrow())?;

    is_admin(&config.admin_key, admin_info)?;

    let mut token_swap = SwapInfo::unpack(&swap_info.data.borrow())?;
    utils::validate_swap_config_key(&token_swap, config_info.key)?;

    token_swap.swap_out_limit_percentage = swap_out_limit_percentage;
    SwapInfo::pack(token_swap, &mut swap_info.data.borrow_mut())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pyth::PYTH_PROGRAM_ID;
    use solana_program::sysvar::Sysvar;
    use spl_token::{
        self,
        state::{Account, AccountState, Mint},
    };
    use std::{mem, str::FromStr};

    #[test]
    fn test_is_admin() {
        let admin_key = Pubkey::new_unique();
        let owner_key = Pubkey::new_unique();
        let wrong_key = Pubkey::new_unique();
        let mut lamports = 0u64;
        let mut data = [0u8];
        let mut test_admin_account = AccountInfo::new(
            &admin_key,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner_key,
            true,
            1u64,
        );

        assert_eq!(
            is_admin(&wrong_key, &test_admin_account),
            Err(ProgramError::from(SwapError::Unauthorized))
        );
        assert_eq!(
            is_admin(&admin_key, &test_admin_account),
            Err(ProgramError::MissingRequiredSignature)
        );

        let mut lamports_ = 0u64;
        let mut data_ = [0u8];
        test_admin_account = AccountInfo::new(
            &admin_key,
            true,
            false,
            &mut lamports_,
            &mut data_,
            &owner_key,
            true,
            1u64,
        );
        assert!(is_admin(&admin_key, &test_admin_account).is_ok());
    }

    fn get_initialize_result(option: u8) -> ProgramResult {
        let program_id = Pubkey::new_unique();
        let config_key = Pubkey::new_unique();
        let market_authority_key = if option == 3u8 {
            Pubkey::new_unique()
        } else {
            Pubkey::find_program_address(&[config_key.as_ref()], &program_id).0
        };

        let market_authority_owner = Pubkey::new_unique();
        let rent_key = solana_program::sysvar::rent::id();

        let deltafi_mint_key = Pubkey::new_unique();
        let deltafi_token_key = Pubkey::new_unique();
        let admin_key = Pubkey::new_unique();
        let token_program_key = if option == 7u8 {
            Pubkey::new_unique()
        } else {
            spl_token::id()
        };

        let pyth_program_key = if option == 6u8 {
            Pubkey::new_unique()
        } else {
            Pubkey::from_str(PYTH_PROGRAM_ID).unwrap()
        };

        let fees = Fees {
            ..Default::default()
        };

        let rewards = Rewards {
            ..Default::default()
        };

        let rent_val = Rent {
            lamports_per_byte_year: 200_000u64,
            exemption_threshold: 60f64,
            burn_percent: 10u8,
        };

        let deltafi_mint = Mint {
            mint_authority: solana_program::program_option::COption::Some(Pubkey::new_unique()),
            supply: 0u64,
            decimals: 0u8,
            is_initialized: true,
            freeze_authority: solana_program::program_option::COption::Some(Pubkey::new_unique()),
        };

        let mut config_lamport = 9_500_000_000u64;
        let config_initial = ConfigInfo {
            version: 0u8,
            bump_seed: 0u8,
            admin_key,
            deltafi_mint: deltafi_mint_key,
            pyth_program_id: pyth_program_key,
            fees: fees.clone(),
            rewards: rewards.clone(),
            deltafi_token: deltafi_token_key,
            ..ConfigInfo::default()
        };
        let mut config_data = [0u8; ConfigInfo::LEN];
        config_initial.pack_into_slice(&mut config_data);
        let config_info = AccountInfo::new(
            &config_key,
            true,
            false,
            &mut config_lamport,
            &mut config_data,
            if option == 1u8 {
                &admin_key
            } else {
                &program_id
            },
            false,
            0u64,
        );

        let mut rent_lamport = 2_000_000_000u64;
        let mut rent_data = [0u8; mem::size_of::<Rent>()];
        let mut rent_info = AccountInfo::new(
            &rent_key,
            false,
            false,
            &mut rent_lamport,
            &mut rent_data,
            &program_id,
            false,
            0u64,
        );
        assert!(rent_val.to_account_info(&mut rent_info).is_some());

        let mut admin_lamports = 0u64;
        let mut admin_data = [0u8];
        let admin_info = AccountInfo::new(
            &admin_key,
            option != 2u8,
            false,
            &mut admin_lamports,
            &mut admin_data,
            &market_authority_key,
            false,
            0u64,
        );

        let mut market_authority_lamports = 0u64;
        let mut market_authority_data = [0u8];
        let market_authority_info = AccountInfo::new(
            &market_authority_key,
            false,
            false,
            &mut market_authority_lamports,
            &mut market_authority_data,
            &market_authority_owner,
            false,
            0u64,
        );

        let mut packed_mint_data = [0u8; Mint::LEN];
        assert!(Mint::pack(deltafi_mint, &mut packed_mint_data).is_ok());

        let mut mint_lamports = 0u64;
        let mint_info = AccountInfo::new(
            &deltafi_mint_key,
            false,
            false,
            &mut mint_lamports,
            &mut packed_mint_data,
            &token_program_key,
            false,
            0u64,
        );

        let mut token_program_lamports = 0u64;
        let mut token_program_data = [0u8];
        let token_program_info = AccountInfo::new(
            &token_program_key,
            true,
            false,
            &mut token_program_lamports,
            &mut token_program_data,
            &admin_key,
            false,
            0u64,
        );

        let mut pyth_program_lamports = 0u64;
        let mut pyth_program_data = [0u8];
        let pyth_program_info = AccountInfo::new(
            &pyth_program_key,
            true,
            false,
            &mut pyth_program_lamports,
            &mut pyth_program_data,
            &admin_key,
            false,
            0u64,
        );

        let mut token_lamports = 0u64;
        let mut deltafi_token_account_data = [0u8; 165];
        let deltafi_token_account = spl_token::state::Account {
            mint: if option == 4u8 {
                Pubkey::new_unique()
            } else {
                deltafi_mint_key
            },
            owner: if option == 5u8 {
                Pubkey::new_unique()
            } else {
                market_authority_key
            },
            amount: 0,
            delegate: solana_program::program_option::COption::None,
            state: spl_token::state::AccountState::Initialized,
            is_native: solana_program::program_option::COption::None,
            delegated_amount: 0,
            close_authority: solana_program::program_option::COption::None,
        };
        deltafi_token_account.pack_into_slice(&mut deltafi_token_account_data);
        let deltafi_token_info = AccountInfo::new(
            &deltafi_token_key,
            false,
            false,
            &mut token_lamports,
            &mut deltafi_token_account_data,
            &token_program_key,
            false,
            0u64,
        );

        initialize(
            &program_id,
            &fees,
            &rewards,
            &[
                config_info,
                market_authority_info,
                mint_info,
                admin_info,
                rent_info,
                token_program_info,
                pyth_program_info,
                deltafi_token_info,
            ],
        )
    }

    #[test]
    fn test_initialize() {
        assert_eq!(get_initialize_result(0u8), Ok(()));
        assert_eq!(
            get_initialize_result(1u8),
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_initialize_result(2u8),
            Err(ProgramError::MissingRequiredSignature)
        );
        assert_eq!(
            get_initialize_result(3u8),
            Err(ProgramError::from(SwapError::InvalidProgramAddress))
        );
        assert_eq!(
            get_initialize_result(4u8),
            Err(ProgramError::from(SwapError::IncorrectMint))
        );
        assert_eq!(
            get_initialize_result(5u8),
            Err(ProgramError::from(SwapError::InvalidOwner))
        );
        assert_eq!(
            get_initialize_result(6u8),
            Err(ProgramError::from(SwapError::InvalidPythProgramId))
        );
        assert_eq!(
            get_initialize_result(7u8),
            Err(ProgramError::IncorrectProgramId)
        );
    }

    fn get_pause_result(option: u8) -> ProgramResult {
        let mut accounts = Vec::new();
        let program_id = Pubkey::new_unique();
        let config_key = Pubkey::new_unique();
        let swap_key = Pubkey::new_unique();
        let admin_key = Pubkey::new_unique();

        let config = ConfigInfo {
            version: 1u8,
            bump_seed: 0u8,
            admin_key: if option == 3u8 { config_key } else { admin_key },
            deltafi_mint: Pubkey::new_unique(),
            pyth_program_id: Pubkey::new_unique(),
            fees: Fees {
                ..Default::default()
            },
            rewards: Rewards {
                ..Default::default()
            },
            deltafi_token: Pubkey::new_unique(),
            ..ConfigInfo::default()
        };

        let mut swap = SwapInfo {
            ..Default::default()
        };
        if option == 6u8 {
            swap.config_key = swap_key;
        } else {
            swap.config_key = config_key;
        }
        if option == 5u8 {
            swap.is_initialized = false;
        } else {
            swap.is_initialized = true;
        }

        let mut lamports = 0u64;
        let mut config_data = [0u8; ConfigInfo::LEN];
        config.pack_into_slice(&mut config_data);
        accounts.push(AccountInfo::new(
            &config_key,
            false,
            false,
            &mut lamports,
            &mut config_data,
            if option == 1u8 {
                &config_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut swap_data = [0u8; SwapInfo::LEN];
        swap.pack_into_slice(&mut swap_data);
        accounts.push(AccountInfo::new(
            &swap_key,
            false,
            false,
            &mut lamports,
            &mut swap_data,
            if option == 2u8 {
                &swap_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut admin_data = [0u8];
        accounts.push(AccountInfo::new(
            &admin_key,
            option != 4u8,
            false,
            &mut lamports,
            &mut admin_data,
            &program_id,
            false,
            0u64,
        ));

        pause(&program_id, &accounts[..])
    }

    #[test]
    fn test_pause() {
        assert!(get_pause_result(0u8).is_ok());
        assert_eq!(
            get_pause_result(1u8),
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_pause_result(2u8),
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_pause_result(3u8),
            Err(ProgramError::from(SwapError::Unauthorized))
        );
        assert_eq!(
            get_pause_result(4u8),
            Err(ProgramError::MissingRequiredSignature)
        );
        assert_eq!(
            get_pause_result(5u8),
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_pause_result(6u8),
            Err(ProgramError::from(SwapError::InvalidMarketConfig))
        );
    }

    fn get_unpause_result(option: u8) -> ProgramResult {
        let mut accounts = Vec::new();
        let program_id = Pubkey::new_unique();
        let config_key = Pubkey::new_unique();
        let swap_key = Pubkey::new_unique();
        let admin_key = Pubkey::new_unique();

        let config = ConfigInfo {
            version: 1u8,
            bump_seed: 0u8,
            admin_key: if option == 3u8 { config_key } else { admin_key },
            deltafi_mint: Pubkey::new_unique(),
            pyth_program_id: Pubkey::new_unique(),
            fees: Fees {
                ..Default::default()
            },
            rewards: Rewards {
                ..Default::default()
            },
            deltafi_token: Pubkey::new_unique(),
            ..ConfigInfo::default()
        };

        let mut swap = SwapInfo {
            ..Default::default()
        };
        if option == 6u8 {
            swap.config_key = swap_key;
        } else {
            swap.config_key = config_key;
        }
        if option == 5u8 {
            swap.is_initialized = false;
        } else {
            swap.is_initialized = true;
        }

        let mut lamports = 0u64;
        let mut config_data = [0u8; ConfigInfo::LEN];
        config.pack_into_slice(&mut config_data);
        accounts.push(AccountInfo::new(
            &config_key,
            false,
            false,
            &mut lamports,
            &mut config_data,
            if option == 1u8 {
                &config_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut swap_data = [0u8; SwapInfo::LEN];
        swap.pack_into_slice(&mut swap_data);
        accounts.push(AccountInfo::new(
            &swap_key,
            false,
            false,
            &mut lamports,
            &mut swap_data,
            if option == 2u8 {
                &swap_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut admin_data = [0u8];
        accounts.push(AccountInfo::new(
            &admin_key,
            option != 4u8,
            false,
            &mut lamports,
            &mut admin_data,
            &program_id,
            false,
            0u64,
        ));

        unpause(&program_id, &accounts[..])
    }

    #[test]
    fn test_unpause() {
        assert!(get_unpause_result(0u8).is_ok());
        assert_eq!(
            get_unpause_result(1u8),
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_unpause_result(2u8),
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_unpause_result(3u8),
            Err(ProgramError::from(SwapError::Unauthorized))
        );
        assert_eq!(
            get_unpause_result(4u8),
            Err(ProgramError::MissingRequiredSignature)
        );
        assert_eq!(
            get_unpause_result(5u8),
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_unpause_result(6u8),
            Err(ProgramError::from(SwapError::InvalidMarketConfig))
        );
    }

    fn get_set_fee_account_result(option: u8) -> ProgramResult {
        let mut accounts = Vec::new();
        let nonce = 8u8;
        let program_id = Pubkey::from_str("5NjW2CAV6MBQYxpL4oK2CESrpdj6tkcvxP3iigAgrHyR").unwrap();
        let config_key = Pubkey::from_str("BuP3jEYfnTCfB4UqQk9L37k2vaXsNuVsbWxrYbGDmL6s").unwrap();
        let swap_key = Pubkey::from_str("CWWiYh5Rpyf5rHZbzHYM6TT6FfcojTR2rKjr5M4BFa3y").unwrap();
        let admin_key = Pubkey::from_str("CaR8CBoWip9oFygaVnonzPCKWgBaSQWwSC3jaYNAyNiK").unwrap();
        let _authority_key =
            Pubkey::create_program_address(&[swap_key.as_ref(), &[nonce]], &program_id);
        assert!(_authority_key.is_ok());

        let authority_key = if option == 7u8 {
            Pubkey::new_unique()
        } else {
            _authority_key.unwrap()
        };
        let new_admin_fee_key = Pubkey::new_unique();
        let token_program_key = Pubkey::new_unique();

        let config = ConfigInfo {
            version: 1u8,
            bump_seed: 0u8,
            admin_key: if option == 3u8 { config_key } else { admin_key },
            deltafi_mint: Pubkey::new_unique(),
            pyth_program_id: Pubkey::new_unique(),
            fees: Fees {
                ..Default::default()
            },
            rewards: Rewards {
                ..Default::default()
            },
            deltafi_token: Pubkey::new_unique(),
            ..ConfigInfo::default()
        };

        let mut swap = SwapInfo {
            ..Default::default()
        };
        if option == 6u8 {
            swap.config_key = swap_key;
        } else {
            swap.config_key = config_key;
        }
        if option == 5u8 {
            swap.is_initialized = false;
        } else {
            swap.is_initialized = true;
        }
        swap.token_a_mint = Pubkey::new_unique();
        swap.token_b_mint = Pubkey::new_unique();
        swap.nonce = nonce;

        let mut account = Account {
            ..Default::default()
        };
        account.owner = if option == 10u8 {
            program_id
        } else {
            authority_key
        };
        account.mint = if option == 11u8 {
            program_id
        } else if option == 12u8 {
            swap.token_b_mint
        } else {
            swap.token_a_mint
        };
        account.state = if option == 9u8 {
            AccountState::Uninitialized
        } else {
            AccountState::Initialized
        };

        let mut lamports = 0u64;
        let mut config_data = [0u8; ConfigInfo::LEN];
        config.pack_into_slice(&mut config_data);
        accounts.push(AccountInfo::new(
            &config_key,
            false,
            false,
            &mut lamports,
            &mut config_data,
            if option == 1u8 {
                &config_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut swap_data = [0u8; SwapInfo::LEN];
        swap.pack_into_slice(&mut swap_data);
        accounts.push(AccountInfo::new(
            &swap_key,
            false,
            false,
            &mut lamports,
            &mut swap_data,
            if option == 2u8 {
                &swap_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut authority_data = [0u8];
        accounts.push(AccountInfo::new(
            &authority_key,
            false,
            false,
            &mut lamports,
            &mut authority_data,
            &program_id,
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut admin_data = [0u8];
        accounts.push(AccountInfo::new(
            &admin_key,
            option != 4u8,
            false,
            &mut lamports,
            &mut admin_data,
            &program_id,
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut new_fee_account_data = [0u8; Account::LEN];
        account.pack_into_slice(&mut new_fee_account_data);
        accounts.push(AccountInfo::new(
            &new_admin_fee_key,
            false,
            false,
            &mut lamports,
            &mut new_fee_account_data,
            if option == 8u8 {
                &program_id
            } else {
                &token_program_key
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut token_swap_data = [0u8];
        accounts.push(AccountInfo::new(
            &token_program_key,
            false,
            false,
            &mut lamports,
            &mut token_swap_data,
            &program_id,
            false,
            0u64,
        ));

        set_fee_account(&program_id, &accounts[..])
    }

    #[test]
    fn test_set_fee_account() {
        assert!(get_set_fee_account_result(0u8).is_ok());
        assert_eq!(
            get_set_fee_account_result(1u8),
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_set_fee_account_result(2u8),
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_set_fee_account_result(3u8),
            Err(ProgramError::from(SwapError::Unauthorized))
        );
        assert_eq!(
            get_set_fee_account_result(4u8),
            Err(ProgramError::MissingRequiredSignature)
        );
        assert_eq!(
            get_set_fee_account_result(5u8),
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_set_fee_account_result(6u8),
            Err(ProgramError::from(SwapError::InvalidMarketConfig))
        );
        assert_eq!(
            get_set_fee_account_result(7u8),
            Err(ProgramError::from(SwapError::InvalidProgramAddress))
        );
        assert_eq!(
            get_set_fee_account_result(8u8),
            Err(ProgramError::from(SwapError::IncorrectTokenProgramId))
        );
        assert_eq!(
            get_set_fee_account_result(9u8),
            Err(ProgramError::from(SwapError::ExpectedAccount))
        );
        assert_eq!(
            get_set_fee_account_result(10u8),
            Err(ProgramError::from(SwapError::InvalidOwner))
        );
        assert_eq!(
            get_set_fee_account_result(11u8),
            Err(ProgramError::from(SwapError::IncorrectMint))
        );
        assert!(get_set_fee_account_result(12u8).is_ok());
    }

    fn get_set_new_fees_result(new_fees: &Fees, option: u8) -> (ProgramResult, Fees) {
        let mut accounts = Vec::new();
        let program_id = Pubkey::new_unique();
        let config_key = Pubkey::new_unique();
        let swap_key = Pubkey::new_unique();
        let admin_key = Pubkey::new_unique();

        let config = ConfigInfo {
            version: if option == 3u8 { 0u8 } else { 1u8 },
            bump_seed: 0u8,
            admin_key: if option == 4u8 { config_key } else { admin_key },
            deltafi_mint: Pubkey::new_unique(),
            pyth_program_id: Pubkey::new_unique(),
            fees: Fees {
                ..Default::default()
            },
            rewards: Rewards {
                ..Default::default()
            },
            deltafi_token: Pubkey::new_unique(),
            ..ConfigInfo::default()
        };

        let mut swap = SwapInfo {
            ..Default::default()
        };
        if option == 7u8 {
            swap.config_key = swap_key;
        } else {
            swap.config_key = config_key;
        }

        swap.is_initialized = option != 6u8;
        swap.fees = Fees {
            ..Default::default()
        };

        let mut lamports = 0u64;
        let mut config_data = [0u8; ConfigInfo::LEN];
        config.pack_into_slice(&mut config_data);
        accounts.push(AccountInfo::new(
            &config_key,
            false,
            false,
            &mut lamports,
            &mut config_data,
            if option == 1u8 {
                &config_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut swap_data = [0u8; SwapInfo::LEN];
        swap.pack_into_slice(&mut swap_data);
        accounts.push(AccountInfo::new(
            &swap_key,
            false,
            false,
            &mut lamports,
            &mut swap_data,
            if option == 2u8 {
                &swap_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut admin_data = [0u8];
        accounts.push(AccountInfo::new(
            &admin_key,
            option != 5u8,
            false,
            &mut lamports,
            &mut admin_data,
            &program_id,
            false,
            0u64,
        ));

        let result = set_new_fees(&program_id, new_fees, &accounts[..]);
        let result_swap = SwapInfo::unpack(&accounts[1].data.borrow());

        (
            result,
            if let Ok(result_swap_content) = result_swap {
                result_swap_content.fees
            } else {
                Fees {
                    ..Default::default()
                }
            },
        )
    }

    #[test]
    fn test_set_new_fees() {
        let test_fees = Fees {
            is_initialized: true,
            admin_trade_fee_numerator: 123_123_123u64,
            admin_trade_fee_denominator: 40_000_000u64,
            admin_withdraw_fee_numerator: 15_000_000u64,
            admin_withdraw_fee_denominator: 3_000_000u64,
            trade_fee_numerator: 24_000_000u64,
            trade_fee_denominator: 31_000_000u64,
            withdraw_fee_numerator: 5_000_000u64,
            withdraw_fee_denominator: 91_000_000u64,
        };

        let ok_result = get_set_new_fees_result(&test_fees, 0u8);
        assert!(ok_result.0.is_ok());
        assert_eq!(ok_result.1, test_fees);
        assert_eq!(
            get_set_new_fees_result(&test_fees, 1u8).0,
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_set_new_fees_result(&test_fees, 2u8).0,
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_set_new_fees_result(&test_fees, 3u8).0,
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_set_new_fees_result(&test_fees, 4u8).0,
            Err(ProgramError::from(SwapError::Unauthorized))
        );
        assert_eq!(
            get_set_new_fees_result(&test_fees, 5u8).0,
            Err(ProgramError::MissingRequiredSignature)
        );
        assert_eq!(
            get_set_new_fees_result(&test_fees, 6u8).0,
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_set_new_fees_result(&test_fees, 7u8).0,
            Err(ProgramError::from(SwapError::InvalidMarketConfig))
        );
    }

    fn get_set_new_rewards_result(new_rewards: &Rewards, option: u8) -> (ProgramResult, Rewards) {
        let mut accounts = Vec::new();
        let program_id = Pubkey::new_unique();
        let config_key = Pubkey::new_unique();
        let swap_key = Pubkey::new_unique();
        let admin_key = Pubkey::new_unique();

        let config = ConfigInfo {
            version: if option == 3u8 { 0u8 } else { 1u8 },
            bump_seed: 0u8,
            admin_key: if option == 4u8 { config_key } else { admin_key },
            deltafi_mint: Pubkey::new_unique(),
            pyth_program_id: Pubkey::new_unique(),
            fees: Fees {
                ..Default::default()
            },
            rewards: Rewards {
                ..Default::default()
            },
            deltafi_token: Pubkey::new_unique(),
            ..ConfigInfo::default()
        };

        let mut swap = SwapInfo {
            ..Default::default()
        };
        if option == 7u8 {
            swap.config_key = swap_key;
        } else {
            swap.config_key = config_key;
        }

        swap.is_initialized = option != 6u8;
        swap.fees = Fees {
            ..Default::default()
        };

        let mut lamports = 0u64;
        let mut config_data = [0u8; ConfigInfo::LEN];
        config.pack_into_slice(&mut config_data);
        accounts.push(AccountInfo::new(
            &config_key,
            false,
            false,
            &mut lamports,
            &mut config_data,
            if option == 1u8 {
                &config_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut swap_data = [0u8; SwapInfo::LEN];
        swap.pack_into_slice(&mut swap_data);
        accounts.push(AccountInfo::new(
            &swap_key,
            false,
            false,
            &mut lamports,
            &mut swap_data,
            if option == 2u8 {
                &swap_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut admin_data = [0u8];
        accounts.push(AccountInfo::new(
            &admin_key,
            option != 5u8,
            false,
            &mut lamports,
            &mut admin_data,
            &program_id,
            false,
            0u64,
        ));

        let result = set_new_rewards(&program_id, new_rewards, &accounts[..]);
        let result_swap = SwapInfo::unpack(&accounts[1].data.borrow());

        (
            result,
            if let Ok(result_swap_content) = result_swap {
                result_swap_content.rewards
            } else {
                Rewards {
                    ..Default::default()
                }
            },
        )
    }

    #[test]
    fn test_set_new_rewards() {
        let test_rewards = Rewards {
            is_initialized: true,
            decimals: 9,
            reserved: [0u8; 7],
            trade_reward_numerator: 123_123_123u64,
            trade_reward_denominator: 40_000_000u64,
            trade_reward_cap: 15_000_000u64,
        };

        let ok_result = get_set_new_rewards_result(&test_rewards, 0u8);
        assert!(ok_result.0.is_ok());
        assert_eq!(ok_result.1, test_rewards);
        assert_eq!(
            get_set_new_rewards_result(&test_rewards, 1u8).0,
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_set_new_rewards_result(&test_rewards, 2u8).0,
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_set_new_rewards_result(&test_rewards, 3u8).0,
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_set_new_rewards_result(&test_rewards, 4u8).0,
            Err(ProgramError::from(SwapError::Unauthorized))
        );
        assert_eq!(
            get_set_new_rewards_result(&test_rewards, 5u8).0,
            Err(ProgramError::MissingRequiredSignature)
        );
        assert_eq!(
            get_set_new_rewards_result(&test_rewards, 6u8).0,
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_set_new_rewards_result(&test_rewards, 7u8).0,
            Err(ProgramError::from(SwapError::InvalidMarketConfig))
        );
    }

    fn get_set_farm_rewards_result(
        farm_rewards: &FarmRewards,
        option: u8,
    ) -> (ProgramResult, FarmRewards) {
        let mut accounts = Vec::new();
        let program_id = Pubkey::new_unique();
        let config_key = Pubkey::new_unique();
        let farm_key = Pubkey::new_unique();
        let admin_key = Pubkey::new_unique();

        let config = ConfigInfo {
            version: if option == 3u8 { 0u8 } else { 1u8 },
            bump_seed: 0u8,
            admin_key: if option == 4u8 { config_key } else { admin_key },
            deltafi_mint: Pubkey::new_unique(),
            pyth_program_id: Pubkey::new_unique(),
            fees: Fees {
                ..Default::default()
            },
            rewards: Rewards {
                ..Default::default()
            },
            deltafi_token: Pubkey::new_unique(),
            ..ConfigInfo::default()
        };

        let mut farm = FarmInfo {
            ..Default::default()
        };
        if option == 7u8 {
            farm.config_key = farm_key;
        } else {
            farm.config_key = config_key;
        }

        farm.is_initialized = option != 6u8;
        farm.apr_numerator = farm_rewards.apr_numerator;
        farm.apr_denominator = farm_rewards.apr_denominator;

        let mut lamports = 0u64;
        let mut config_data = [0u8; ConfigInfo::LEN];
        config.pack_into_slice(&mut config_data);
        accounts.push(AccountInfo::new(
            &config_key,
            false,
            false,
            &mut lamports,
            &mut config_data,
            if option == 1u8 {
                &config_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut farm_data = [0u8; FarmInfo::LEN];
        farm.pack_into_slice(&mut farm_data);
        accounts.push(AccountInfo::new(
            &farm_key,
            false,
            false,
            &mut lamports,
            &mut farm_data,
            if option == 2u8 {
                &farm_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut admin_data = [0u8];
        accounts.push(AccountInfo::new(
            &admin_key,
            option != 5u8,
            false,
            &mut lamports,
            &mut admin_data,
            &program_id,
            false,
            0u64,
        ));

        let result = set_farm_rewards(&program_id, farm_rewards, &accounts[..]);
        let result_farm = FarmInfo::unpack(&accounts[1].data.borrow());

        (
            result,
            if let Ok(result_farm_content) = result_farm {
                FarmRewards {
                    apr_numerator: result_farm_content.apr_numerator,
                    apr_denominator: result_farm_content.apr_denominator,
                }
            } else {
                FarmRewards {
                    apr_numerator: 0u64,
                    apr_denominator: 0u64,
                }
            },
        )
    }

    #[test]
    fn test_set_farm_rewards() {
        let test_staking_reward = FarmRewards {
            apr_numerator: 222_222_222u64,
            apr_denominator: 333_333_333u64,
        };

        let ok_result = get_set_farm_rewards_result(&test_staking_reward, 0u8);
        assert!(ok_result.0.is_ok());
        assert_eq!(ok_result.1, test_staking_reward);
        assert_eq!(
            get_set_farm_rewards_result(&test_staking_reward, 1u8).0,
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_set_farm_rewards_result(&test_staking_reward, 2u8).0,
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_set_farm_rewards_result(&test_staking_reward, 3u8).0,
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_set_farm_rewards_result(&test_staking_reward, 4u8).0,
            Err(ProgramError::from(SwapError::Unauthorized))
        );
        assert_eq!(
            get_set_farm_rewards_result(&test_staking_reward, 5u8).0,
            Err(ProgramError::MissingRequiredSignature)
        );
        assert_eq!(
            get_set_farm_rewards_result(&test_staking_reward, 6u8).0,
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_set_farm_rewards_result(&test_staking_reward, 7u8).0,
            Err(ProgramError::from(SwapError::InvalidMarketConfig))
        );
    }

    fn get_set_slope_result(slope: u64, option: u8) -> (ProgramResult, u64) {
        let mut accounts = Vec::new();
        let program_id = Pubkey::new_unique();
        let config_key = Pubkey::new_unique();
        let pool_key = Pubkey::new_unique();
        let admin_key = Pubkey::new_unique();

        let config = ConfigInfo {
            version: if option == 3u8 { 0u8 } else { 1u8 },
            bump_seed: 0u8,
            admin_key: if option == 4u8 { config_key } else { admin_key },
            deltafi_mint: Pubkey::new_unique(),
            pyth_program_id: Pubkey::new_unique(),
            fees: Fees {
                ..Default::default()
            },
            rewards: Rewards {
                ..Default::default()
            },
            deltafi_token: Pubkey::new_unique(),
            ..ConfigInfo::default()
        };

        let mut swap = SwapInfo {
            ..Default::default()
        };
        if option == 7u8 {
            swap.config_key = pool_key;
        } else {
            swap.config_key = config_key;
        }

        swap.is_initialized = option != 6u8;
        swap.pool_state.slope = Decimal::from(slope);

        let mut lamports = 0u64;
        let mut config_data = [0u8; ConfigInfo::LEN];
        config.pack_into_slice(&mut config_data);
        accounts.push(AccountInfo::new(
            &config_key,
            false,
            false,
            &mut lamports,
            &mut config_data,
            if option == 1u8 {
                &config_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut swap_data = [0u8; SwapInfo::LEN];
        swap.pack_into_slice(&mut swap_data);
        accounts.push(AccountInfo::new(
            &pool_key,
            false,
            false,
            &mut lamports,
            &mut swap_data,
            if option == 2u8 {
                &pool_key
            } else {
                &program_id
            },
            false,
            0u64,
        ));

        let mut lamports = 0u64;
        let mut admin_data = [0u8];
        accounts.push(AccountInfo::new(
            &admin_key,
            option != 5u8,
            false,
            &mut lamports,
            &mut admin_data,
            &program_id,
            false,
            0u64,
        ));

        let result = set_slope(&program_id, slope, &accounts[..]);
        let result_pool = SwapInfo::unpack(&accounts[1].data.borrow());

        (result, if result_pool.is_ok() { slope } else { 0u64 })
    }

    #[test]
    fn test_set_slope() {
        let test_set_slope = 123_123_123u64;

        let ok_result = get_set_slope_result(test_set_slope, 0u8);
        assert!(ok_result.0.is_ok());
        assert_eq!(ok_result.1, test_set_slope);
        assert_eq!(
            get_set_slope_result(test_set_slope, 1u8).0,
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_set_slope_result(test_set_slope, 2u8).0,
            Err(ProgramError::from(SwapError::InvalidAccountOwner))
        );
        assert_eq!(
            get_set_slope_result(test_set_slope, 3u8).0,
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_set_slope_result(test_set_slope, 4u8).0,
            Err(ProgramError::from(SwapError::Unauthorized))
        );
        assert_eq!(
            get_set_slope_result(test_set_slope, 5u8).0,
            Err(ProgramError::MissingRequiredSignature)
        );
        assert_eq!(
            get_set_slope_result(test_set_slope, 6u8).0,
            Err(ProgramError::UninitializedAccount)
        );
        assert_eq!(
            get_set_slope_result(test_set_slope, 7u8).0,
            Err(ProgramError::from(SwapError::InvalidMarketConfig))
        );
    }
}
