//! Helpers for working with swaps in a fuzzing environment
#![allow(clippy::too_many_arguments)]
use crate::native_account_data::NativeAccountData;
use crate::native_processor::do_process_instruction;
use crate::native_token;

use solana_program::{
    entrypoint::ProgramResult,
    bpf_loader,
    system_program,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    sysvar::{clock, clock::Clock},
};

use deltafi_swap::{
    instruction::{
        self, FarmDepositData, FarmWithdrawData,
    },
    state::{FarmInfo, FarmUser, SwapInfo, FarmPosition},
    error::SwapError,
};
use spl_token::instruction::approve;
use std::borrow::Borrow;

pub struct NativeFarm {
    pub swap_pool_account: NativeAccountData,
    pub farm_pool_account: NativeAccountData,
    pub farm_authority_account: NativeAccountData,
    pub pool_mint_account: NativeAccountData,
    pub pool_token_account: NativeAccountData,
    pub clock_info_account: NativeAccountData,
    pub farm_user_account: NativeAccountData,
    pub farm_owner_account: NativeAccountData,
    pub user_account: NativeAccountData,
    pub user_token_account: NativeAccountData,
    pub token_program_account: NativeAccountData,
    pub bump_seed: u8,
    pub program_id: Pubkey,
}

pub fn create_program_account(program_id: Pubkey) -> NativeAccountData {
    let mut account_data = NativeAccountData::new(0, bpf_loader::id());
    account_data.key = program_id;
    account_data
}

impl NativeFarm {
    pub fn new(
        pool_reserved_amount: u64,
    ) -> Self {
        let program_id = deltafi_swap::id();
		let token_program_account = create_program_account(spl_token::id());
        let mut farm_pool_account = NativeAccountData::new(FarmInfo::LEN, program_id);
        let mut swap_pool_account = NativeAccountData::new(SwapInfo::LEN, program_id);
        let (farm_authority_key, bump_seed) = Pubkey::find_program_address(
            &[&farm_pool_account.key.to_bytes()[..]],
            &program_id,
        );
        let farm_authority_account = create_program_account(farm_authority_key);
    
        let mut pool_mint_account =
            native_token::create_mint(&farm_authority_account.key);

        let swap_pool_info = SwapInfo {
            is_initialized: true,
            pool_mint: pool_mint_account.key,
            ..Default::default()
        };
        SwapInfo::pack_into_slice(&swap_pool_info, &mut swap_pool_account.data);

        let mut farm_owner_account = NativeAccountData::new(0, system_program::id());
        farm_owner_account.is_signer = true;        
    
        let pool_token_account =
        native_token::create_token_account(
            &mut pool_mint_account, &farm_owner_account.key, pool_reserved_amount);

        let farm_pool_info = FarmInfo {
            is_initialized: true,
            pool_mint: pool_mint_account.key,
            pool_token: pool_token_account.key,
            bump_seed: bump_seed,
            reserved_amount: pool_reserved_amount,
            ..Default::default()
        };
        FarmInfo::pack_into_slice(&farm_pool_info, &mut farm_pool_account.data);

        let mut clock_info_account = NativeAccountData::new(std::mem::size_of::<Clock>(), program_id);
        clock_info_account.key = clock::id();

        let mut farm_user_account = NativeAccountData::new(FarmUser::LEN, program_id);

        let mut user_account = NativeAccountData::new(0, system_program::id());
        user_account.is_signer = true;

        let user_token_account =
            native_token::create_token_account(&mut pool_mint_account, &user_account.key, 100_000_000u64);
        
        let farm_user_list = [
            FarmUser {
                is_initialized: true,
                config_key: farm_owner_account.key,
                owner: farm_owner_account.key,
                positions: vec![
                    FarmPosition {
                        pool: farm_pool_account.key,
                        deposited_amount: 10_000_000u64,
                        rewards_owed: 100_000u64,
                        rewards_estimated: 200_000u64,
                        cumulative_interest: 50_000u64,
                        last_update_ts: 10i64,
                        next_claim_ts: 10i64,
                        latest_deposit_slot: 100,
                    },
                    FarmPosition {
                        pool: farm_pool_account.key,
                        deposited_amount: 10_000_000u64,
                        rewards_owed: 100_000u64,
                        rewards_estimated: 200_000u64,
                        cumulative_interest: 50_000u64,
                        last_update_ts: 10i64,
                        next_claim_ts: 10i64,
                        latest_deposit_slot: 100,
                    },
                ],
            },
        ];
        farm_user_list[0].pack_into_slice(&mut farm_user_account.data);

        Self {
            swap_pool_account,
            farm_pool_account,
            farm_authority_account,
            pool_mint_account,
            pool_token_account,
            clock_info_account,
            farm_user_account,
            farm_owner_account,
            user_account,
            user_token_account,
            token_program_account,
            bump_seed,
            program_id,
        }

    }
    
    pub fn get_deposited_amount(
        & self,
    ) -> Result<u64, ProgramError> {
        let mut farm_user = FarmUser::unpack_from_slice(&(self.farm_user_account).data.borrow())?;
        let (position, _position_index) = farm_user
            .find_position(self.farm_pool_account.key)
            .ok_or(SwapError::LiquidityPositionEmpty)?;
        Ok(position.deposited_amount)
    }

    pub fn run_farm_refresh(
        &mut self,
    ) -> ProgramResult {
        // create a FarmRefresh instruction
        let mut farm_user_pubkeys = Vec::new();
        farm_user_pubkeys.push(self.farm_user_account.key);

        let refresh_instruction = instruction::farm_refresh(
                self.program_id,
                self.farm_pool_account.key,
                farm_user_pubkeys,
        )
        .unwrap();

        do_process_instruction(
            refresh_instruction,
            &[
                self.farm_pool_account.as_account_info(),
                self.clock_info_account.as_account_info(),
                self.farm_user_account.as_account_info(),
            ],                                       
        )

    }

    pub fn run_farm_deposit(
        &mut self,
        instruction_data: FarmDepositData,
    ) -> ProgramResult {

        // create and send an Approve instruction
        let mut user_transfer_account = NativeAccountData::new(0, system_program::id());
        user_transfer_account.is_signer = true;

        do_process_instruction(
            approve(
                &self.token_program_account.key,
                &self.user_token_account.key,
                &user_transfer_account.key,
                &self.user_account.key,
                &[],
                instruction_data.amount,
            )
            .unwrap(),
            &[
                self.user_token_account.as_account_info(),
                user_transfer_account.as_account_info(),
                self.user_account.as_account_info(),
            ],
        ).unwrap();

        // create and send a FarmDeposit instruction
        let farm_deposit_instruction = instruction::farm_deposit(
            self.program_id,
            self.farm_pool_account.key,
            user_transfer_account.key,
            self.user_token_account.key,
            self.pool_token_account.key,
            self.farm_user_account.key,
            self.farm_owner_account.key,
            instruction_data,)
        .unwrap();

        do_process_instruction(
            farm_deposit_instruction,
            &[
                self.farm_pool_account.as_account_info(),
                user_transfer_account.as_account_info(),
                self.user_token_account.as_account_info(),
                self.pool_token_account.as_account_info(),
                self.farm_user_account.as_account_info(),
                self.farm_owner_account.as_account_info(),
                self.clock_info_account.as_account_info(),
                self.token_program_account.as_account_info(),
            ],                                       
        )

    }

    pub fn run_farm_withdraw(
        &mut self,
        instruction_data: FarmWithdrawData,
    ) -> ProgramResult {

        // create and send an Approve instruction
        let mut user_transfer_account = NativeAccountData::new(0, system_program::id());
        user_transfer_account.is_signer = true;

        do_process_instruction(
            approve(
                &self.token_program_account.key,
                &self.pool_token_account.key,
                &self.farm_authority_account.key,
                &self.farm_owner_account.key,
                &[],
                instruction_data.amount,
            )
            .unwrap(),
            &[
                self.pool_token_account.as_account_info(),
                self.farm_authority_account.as_account_info(),
                self.farm_owner_account.as_account_info(),
            ],
        ).unwrap();

        // create and send a FarmWithdraw instruction
        let farm_withdraw_instruction = instruction::farm_withdraw(
            self.program_id,
            self.farm_pool_account.key,
            self.farm_user_account.key,            
            self.farm_authority_account.key,
            self.pool_token_account.key,
            self.user_token_account.key,
            self.farm_owner_account.key,
            instruction_data,)
        .unwrap();

        do_process_instruction(
            farm_withdraw_instruction,
            &[
                self.farm_pool_account.as_account_info(),
                self.farm_user_account.as_account_info(),                
                self.farm_authority_account.as_account_info(),
                self.pool_token_account.as_account_info(),                
                self.user_token_account.as_account_info(),
                self.farm_owner_account.as_account_info(),
                self.token_program_account.as_account_info(),
            ],                                       
        )
    }
}

