//! Instruction types

#![allow(clippy::too_many_arguments)]

use std::{convert::TryInto, mem::size_of};

use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::{Pubkey, PUBKEY_BYTES},
    sysvar::{clock, rent},
};

use crate::{
    error::SwapError,
    state::{Fees, Rewards},
};

#[cfg(feature = "fuzz")]
use arbitrary::Arbitrary;

/// Instruction Type
#[repr(C)]
pub enum InstructionType {
    /// Admin
    Admin,
    /// Swap
    Swap,
    /// Stablecoins swap
    StableSwap,
    /// Farm
    Farm,
}

impl InstructionType {
    #[doc(hidden)]
    pub fn check(input: &[u8]) -> Option<Self> {
        let (&tag, _rest) = input.split_first()?;
        match tag {
            100..=110 => Some(Self::Admin),
            0..=5 => Some(Self::Swap),
            10..=14 => Some(Self::StableSwap),
            20..=25 => Some(Self::Farm),
            _ => None,
        }
    }
}

/// SWAP INSTRUNCTION DATA
/// Initialize instruction data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct InitializeData {
    /// Nonce used to create valid program address
    pub nonce: u8,
    /// Slope variable - real value * 10**18, 0 <= slope <= 1
    pub slope: u64,
    /// mid price
    pub mid_price: u128,
    /// token a decimals
    pub token_a_decimals: u8,
    /// token a decimals
    pub token_b_decimals: u8,
    /// token a amount
    pub token_a_amount: u64,
    /// token a decimals
    pub token_b_amount: u64,
    /// oracle priority flags
    pub oracle_priority_flags: u8,
}

/// Stable swap initialize data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct StableInitializeData {
    /// Nonce used to create valid program address
    pub nonce: u8,
    /// Slope variable - default = 0.001
    pub slope: u64,
    /// token a decimals
    pub token_a_decimals: u8,
    /// token a decimals
    pub token_b_decimals: u8,
    /// token a amount
    pub token_a_amount: u64,
    /// token a decimals
    pub token_b_amount: u64,
}

/// Swap instruction data
#[cfg_attr(feature = "fuzz", derive(Arbitrary))]
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct SwapData {
    /// SOURCE amount to transfer, output to DESTINATION is based on the exchange rate
    pub amount_in: u64,
    /// Minimum amount of DESTINATION token to output, prevents excessive slippage
    pub minimum_amount_out: u64,
}

/// Deposit instruction data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct DepositData {
    /// Token A amount to deposit
    pub token_a_amount: u64,
    /// Token B amount to deposit
    pub token_b_amount: u64,
    /// Minimum LP tokens to mint, prevents excessive slippage
    pub min_mint_amount: u64,
}

/// Withdraw instruction data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct WithdrawData {
    /// Amount of pool tokens to burn. User receives an output of token a
    /// and b based on the percentage of the pool tokens that are returned.
    pub pool_token_amount: u64,
    /// Minimum amount of token A to receive, prevents excessive slippage
    pub minimum_token_a_amount: u64,
    /// Minimum amount of token B to receive, prevents excessive slippage
    pub minimum_token_b_amount: u64,
}

/// ADMIN INSTRUCTION PARAMS
/// Admin initialize config data
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct AdminInitializeData {
    /// Default fees
    pub fees: Fees,
    /// Default rewards
    /// TODO: Add farm rates also
    pub rewards: Rewards,
}

/// Set new admin key
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct CommitNewAdmin {
    /// The new admin
    pub new_admin_key: Pubkey,
}

/// Set new staking rewards ratio to stake pool
#[derive(Clone, Debug, PartialEq)]
pub struct FarmRewards {
    /// LP staking APR numerator
    pub apr_numerator: u64,
    /// LP staking APR denominator
    pub apr_denominator: u64,
}

/// Admin only instructions.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum AdminInstruction {
    /// Admin initialization instruction
    ///
    ///   0. `[writable]` New Market config to create.
    ///   1. `[]` $market_athority derived from `create_program_address(&[market_config account])`
    ///   2. `[]` deltafi mint.
    ///   3. `[signer]` admin account.
    ///   4. `[]` Rent sysvar.
    ///   5. `[]` token_program_id.
    ///   6. `[]` pyth_program_id.
    ///   7. `[]` deltafi token.
    Initialize(AdminInitializeData),
    /// Pause pool
    ///
    ///   0. `[]` Market config
    ///   1. `[writable]` token_swap account of the pool
    ///   2. `[signer]` admin account
    Pause,
    /// Resume pool
    ///
    ///   0. `[]` Market config
    ///   1. `[writable]` token_swap account of the pool
    ///   2. `[signer]` admin account
    Unpause,
    /// Set fee account to the pool
    ///
    ///   0. `[]` Market config
    ///   1. `[writable]` token_swap account of the pool
    ///   2. `[]` $authority derived from `create_program_address(&[token_swap acc])`
    ///   3. `[signer]` admin account
    ///   4. `[]` new fee account that matches one of both
    ///   5. `[]` token_program_id
    SetFeeAccount,
    /// Commit new admin account
    ///
    ///   0. `[writable]` market config
    ///   1. `[signer]` admin account
    ///   2. `[writable]` deltafi mint account that'd change freeze_authority
    ///   3. `[]` token_program_id
    CommitNewAdmin(CommitNewAdmin),
    /// Set new fees to the pool
    ///
    ///   1. `[]` market config
    ///   2. `[writable]` token_swap account of the pool
    ///   3. `[signer]` admin account
    SetNewFees(Fees),
    /// Set new rewards to the pool
    ///
    ///   1. `[]` market config
    ///   2. `[writable]` token_swap account of the pool
    ///   3. `[signer]` admin account
    /// TODO: rename to trade rewards
    SetNewRewards(Rewards),
    /// Set new staking rewards ratio to the stake pool
    ///
    ///   0. `[]` Market config
    ///   1. `[writable]` farm pool account
    ///   2. `[signer]` admin account
    SetFarmRewards(FarmRewards),
    /// Set new staking rewards ratio to the stake pool
    ///
    ///   0. `[]` Market config
    ///   1. `[writable]` token_swap account of the pool
    ///   2. `[signer]` admin account
    SetSlope(u64),
    /// Set base token and quote token decimals
    ///
    ///   0. `[]` Market config
    ///   1. `[writable]` token_swap account of the pool
    ///   2. `[signer]` admin account
    SetDecimals(u8, u8),
    /// Set the limitation on the swap out amount
    ///
    ///   0. `[]` Market config
    ///   1. `[writable]` token_swap account of the pool
    ///   2. `[signer]` admin account
    SetSwapLimit(u8),
}

impl AdminInstruction {
    /// Unpacks a byte buffer into a [AdminInstruction](enum.AdminInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(SwapError::InstructionUnpackError)?;
        Ok(match tag {
            100 => {
                let (fees, rest) = rest.split_at(Fees::LEN);
                let fees = Fees::unpack_unchecked(fees)?;
                let (rewards, _rest) = rest.split_at(Rewards::LEN);
                let rewards = Rewards::unpack_unchecked(rewards)?;
                Self::Initialize(AdminInitializeData { fees, rewards })
            }
            101 => Self::Pause,
            102 => Self::Unpause,
            103 => Self::SetFeeAccount,
            104 => {
                let (new_admin_key, _) = unpack_pubkey(rest)?;
                Self::CommitNewAdmin(CommitNewAdmin { new_admin_key })
            }
            105 => {
                let fees = Fees::unpack_unchecked(rest)?;
                Self::SetNewFees(fees)
            }
            106 => {
                let rewards = Rewards::unpack_unchecked(rest)?;
                Self::SetNewRewards(rewards)
            }
            107 => {
                let (apr_numerator, rest) = unpack_u64(rest)?;
                let (apr_denominator, _) = unpack_u64(rest)?;
                Self::SetFarmRewards(FarmRewards {
                    apr_numerator,
                    apr_denominator,
                })
            }
            108 => {
                let (slope, _) = unpack_u64(rest)?;
                Self::SetSlope(slope)
            }
            109 => {
                let (base_decimals, rest) = unpack_u8(rest)?;
                let (quote_decimals, _) = unpack_u8(rest)?;
                Self::SetDecimals(base_decimals, quote_decimals)
            }
            110 => {
                let (swap_out_limit_percentage, _) = unpack_u8(rest)?;
                Self::SetSwapLimit(swap_out_limit_percentage)
            }
            _ => return Err(SwapError::InvalidInstruction.into()),
        })
    }

    /// Packs a [AdminInstruction](enum.AdminInstruciton.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match &*self {
            Self::Initialize(AdminInitializeData { fees, rewards }) => {
                buf.push(100);
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
                let mut rewards_slice = [0u8; Rewards::LEN];
                Pack::pack_into_slice(rewards, &mut rewards_slice[..]);
                buf.extend_from_slice(&rewards_slice);
            }
            Self::Pause => buf.push(101),
            Self::Unpause => buf.push(102),
            Self::SetFeeAccount => buf.push(103),
            Self::CommitNewAdmin(CommitNewAdmin { new_admin_key }) => {
                buf.push(104);
                buf.extend_from_slice(new_admin_key.as_ref());
            }
            Self::SetNewFees(fees) => {
                buf.push(105);
                let mut fees_slice = [0u8; Fees::LEN];
                Pack::pack_into_slice(fees, &mut fees_slice[..]);
                buf.extend_from_slice(&fees_slice);
            }
            Self::SetNewRewards(rewards) => {
                buf.push(106);
                let mut rewards_slice = [0u8; Rewards::LEN];
                Pack::pack_into_slice(rewards, &mut rewards_slice[..]);
                buf.extend_from_slice(&rewards_slice);
            }
            Self::SetFarmRewards(FarmRewards {
                apr_numerator,
                apr_denominator,
            }) => {
                buf.push(107);
                buf.extend_from_slice(&apr_numerator.to_le_bytes());
                buf.extend_from_slice(&apr_denominator.to_le_bytes());
            }
            Self::SetSlope(slope) => {
                buf.push(108);
                buf.extend_from_slice(&slope.to_le_bytes());
            }
            Self::SetDecimals(base_decimals, quote_decimals) => {
                buf.push(109);
                buf.extend_from_slice(&base_decimals.to_le_bytes());
                buf.extend_from_slice(&quote_decimals.to_le_bytes());
            }
            Self::SetSwapLimit(swap_out_limit_percentage) => {
                buf.push(110);
                buf.extend_from_slice(&swap_out_limit_percentage.to_le_bytes());
            }
        }
        buf
    }
}

/// Creates an 'initialize' instruction
pub fn initialize_config(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    market_authority_pubkey: Pubkey,
    deltafi_mint_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    pyth_program_id: Pubkey,
    fees: Fees,
    rewards: Rewards,
    deltafi_token_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Initialize(AdminInitializeData { fees, rewards }).pack();

    let accounts = vec![
        AccountMeta::new(config_pubkey, true),
        AccountMeta::new_readonly(market_authority_pubkey, false),
        AccountMeta::new_readonly(deltafi_mint_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new_readonly(rent::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(pyth_program_id, false),
        AccountMeta::new_readonly(deltafi_token_pubkey, false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'pause' instruction
pub fn pause(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Pause.pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'unpause' instruction
pub fn unpause(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::Unpause.pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'set_fee_account' instruction
pub fn set_fee_account(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    new_fee_account_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetFeeAccount.pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new_readonly(new_fee_account_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'commit_new_admin' instruction
pub fn commit_new_admin(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    deltafi_mint_pubkey: Pubkey,
    new_admin_key: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::CommitNewAdmin(CommitNewAdmin { new_admin_key }).pack();

    let accounts = vec![
        AccountMeta::new(config_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new(deltafi_mint_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'set_new_fees' instruction
pub fn set_new_fees(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    new_fees: Fees,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetNewFees(new_fees).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'set_rewards' instruction.
pub fn set_new_rewards(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    new_rewards: Rewards,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetNewRewards(new_rewards).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates `set_farm_rewards` instruction
pub fn set_farm_rewards(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    farm_pool_info: Pubkey,
    admin_pubkey: Pubkey,
    new_rewards: FarmRewards,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetFarmRewards(new_rewards).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(farm_pool_info, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates `set_slope` instruction
pub fn set_slope(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    slope: u64,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetSlope(slope).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates `set_decimals` instruction
pub fn set_decimals(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    base_decimals: u8,
    quote_decimals: u8,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetDecimals(base_decimals, quote_decimals).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates `set_swap_limit` instruction
pub fn set_swap_limit(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    swap_out_limit_percentage: u8,
) -> Result<Instruction, ProgramError> {
    let data = AdminInstruction::SetSwapLimit(swap_out_limit_percentage).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Instructions supported by the pool SwapInfo program.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum SwapInstruction {
    ///   Initializes a new SwapInfo.
    ///
    ///   0. `[]` market config.
    ///   1. `[writable]` new token-swap to create.
    ///   2. `[]` $swap_authority.
    ///   3. `[]` admin fee account for base token.
    ///   4. `[]` admin fee account for quote token.
    ///   5. `[]` base token account. Must be non zero, owned by $swap_authority.
    ///   6. `[]` quote token account. Must be non zero, owned by $swap_authority.
    ///   7. `[writable]` pool mint account mint by $swap_authority.
    ///   8. `[writable]` pool token account owned by user.
    ///   9. `[]` base token product from pyth network.
    ///   10. `[]` base token price from pyth network.
    ///   11. `[]` quote token product from pyth network.
    ///   12. `[]` quote token price from pyth network.
    ///   13. `[]` admin account.
    ///   14. '[]' serum market account
    ///   15. '[]' serum bids orderbook account
    ///   16. '[]' serum asks orderbook account
    Initialize(InitializeData),

    ///   Swap the tokens in the pool.
    ///
    ///   0. `[]` market config.
    ///   1. `[writable]` token-swap.
    ///   2. `[]` $market_authority to mint deltafi token.
    ///   3. `[]` $swap_authority.
    ///   4. `[signer]` $user_transfer_authority
    ///   5. `[writable]` SOURCE(base|quote) account, transferable by $user_transfer_authority.
    ///   6. `[writable]` (base|quote) token account to swap INTO. Must be the SOURCE token.
    ///   7. `[]` Mint account that provides mint info including decimals of the SOURCE token,
    ///   8. `[writable]` (base|quote) token account to swap FROM. Must be the DESTINATION token.
    ///   9. `[writable]` DESTINATION(base|quote) account owned by user.
    ///   10. `[]` mint account that provides mint info including decimals of the DESTINATION token.
    ///   11. `[writable]` rewards(DELTAFI) token account owned by user.
    ///   12. `[writable]` rewards(DELTAFI) source deltafi token account to issue reward.
    ///   13. `[writable]` (base|quote) admin fee account. Must have same mint as DESTINATION token.
    ///   14. `[]` base token price from pyth network.
    ///   15. `[]` quote token price from pyth network.
    ///   16. `[]` token program id.
    ///   17. `[]` optional: user referrer data account.
    ///   18. `[writable]` optional: referrer token account.
    Swap(SwapData),

    ///   Deposit some tokens into the pool.  The output is a "pool" token representing ownership
    ///   into the pool. Inputs are converted to the current ratio.
    ///
    ///   0. `[]` token-swap.
    ///   1. `[]` $swap_authority.
    ///   2. `[signer]` $user_transfer_authority.
    ///   3. `[writable]` base token account to deposit FROM, transferable by $user_transfer_authority.
    ///   4. `[writable]` quote token account to deposit FROM, transferable by $user_transfer_authority.
    ///   5. `[writable]` base token account to deposit INTO.
    ///   6. `[writable]` quote token account to deposit INTO.
    ///   7. `[writable]` pool mint account, mint by $swap_authority.
    ///   8. `[writable]` pool token account owned by user.
    ///   9. `[]` base token price from pyth network.
    ///   10. `[]` quote token price from pyth network.
    ///   11. `[]` clock sysvar.
    ///   12. `[]` token program id.
    Deposit(DepositData),

    ///   Withdraw tokens from the pool at the current ratio.
    ///
    ///   0. `[]` token-swap.
    ///   1. `[]` $swap_authority.
    ///   2. `[signer]` $user_transfer_authority.
    ///   3. `[writable]` pool mint account, mint by $swap_autority.
    ///   4. `[writable]` SOURCE pool token account, transferable by $user_transfer_authority.
    ///   5. `[writable]` base token account to withdraw FROM.
    ///   6. `[writable]` quote token account to withdraw FROM.
    ///   7. `[writable]` base token account to withdraw INTO.
    ///   8. `[writable]` quote token account to withdraw INTO.
    ///   9. `[writable]` admin fee account for base token.
    ///   10. `[writable]` admin fee account for quote token.
    ///   11. `[]` base token price from pyth network.
    ///   12. `[]` quote token price from pyth network.
    ///   13. `[]` clock sysvar.
    ///   14. `[]` token program id.
    Withdraw(WithdrawData),

    ///   Withdraw tokens from the pool at the current ratio.
    ///
    ///   0. `[]` market config.
    ///   1. `[signer]` user.
    ///   2. `[writable]` user referrer data account
    ///   3. `[]` referrer token address.
    ///   4. `[]` rent sysvar.
    ///   5. `[]` token program id.
    SetReferrer,

    ///   Swap the tokens in the pool (removed src and dest mint accounts).
    ///
    ///   0. `[]` market config.
    ///   1. `[writable]` token-swap.
    ///   2. `[]` $market_authority to mint deltafi token.
    ///   3. `[]` $swap_authority.
    ///   4. `[signer]` $user_transfer_authority
    ///   5. `[writable]` SOURCE(base|quote) account, transferable by $user_transfer_authority.
    ///   6. `[writable]` (base|quote) token account to swap INTO. Must be the SOURCE token.
    ///   7. `[writable]` (base|quote) token account to swap FROM. Must be the DESTINATION token.
    ///   8. `[writable]` DESTINATION(base|quote) account owned by user.
    ///   9. `[writable]` rewards(DELTAFI) token account owned by user.
    ///   10. `[writable]` rewards(DELTAFI) source deltafi token account to issue reward.
    ///   11. `[writable]` (base|quote) admin fee account. Must have same mint as DESTINATION token.
    ///   12. `[]` base token price from pyth network.
    ///   13. `[]` quote token price from pyth network.
    ///   14. '[]' serum market account
    ///   15. '[]' serum bids orderbook account
    ///   16. '[]' serum asks orderbook account
    ///   17. `[]` token program id.
    ///   18. `[]` optional: user referrer data account.
    ///   19. `[writable]` optional: referrer token account.
    SwapV2(SwapData),
}

impl SwapInstruction {
    /// Unpacks a byte buffer into a [SwapInstruction](enum.SwapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(SwapError::InstructionUnpackError)?;
        Ok(match tag {
            0 => {
                let (&nonce, rest) = rest
                    .split_first()
                    .ok_or(SwapError::InstructionUnpackError)?;
                let (slope, rest) = unpack_u64(rest)?;
                let (mid_price, rest) = unpack_u128(rest)?;
                let (token_a_decimals, rest) = unpack_u8(rest)?;
                let (token_b_decimals, rest) = unpack_u8(rest)?;
                let (token_a_amount, rest) = unpack_u64(rest)?;
                let (token_b_amount, rest) = unpack_u64(rest)?;
                let (oracle_priority_flags, _) = unpack_u8(rest)?;
                Self::Initialize(InitializeData {
                    nonce,
                    slope,
                    mid_price,
                    token_a_decimals,
                    token_b_decimals,
                    token_a_amount,
                    token_b_amount,
                    oracle_priority_flags,
                })
            }
            1 => {
                let (amount_in, rest) = unpack_u64(rest)?;
                let (minimum_amount_out, _) = unpack_u64(rest)?;
                Self::Swap(SwapData {
                    amount_in,
                    minimum_amount_out,
                })
            }
            2 => {
                let (token_a_amount, rest) = unpack_u64(rest)?;
                let (token_b_amount, rest) = unpack_u64(rest)?;
                let (min_mint_amount, _) = unpack_u64(rest)?;
                Self::Deposit(DepositData {
                    token_a_amount,
                    token_b_amount,
                    min_mint_amount,
                })
            }
            3 => {
                let (pool_token_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_a_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_b_amount, _) = unpack_u64(rest)?;
                Self::Withdraw(WithdrawData {
                    pool_token_amount,
                    minimum_token_a_amount,
                    minimum_token_b_amount,
                })
            }
            4 => Self::SetReferrer,
            5 => {
                let (amount_in, rest) = unpack_u64(rest)?;
                let (minimum_amount_out, _) = unpack_u64(rest)?;
                Self::SwapV2(SwapData {
                    amount_in,
                    minimum_amount_out,
                })
            }
            _ => return Err(SwapError::InvalidInstruction.into()),
        })
    }

    /// Packs a [SwapInstruction](enum.SwapInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::Initialize(InitializeData {
                nonce,
                slope,
                mid_price,
                token_a_decimals,
                token_b_decimals,
                token_a_amount,
                token_b_amount,
                oracle_priority_flags,
            }) => {
                buf.push(0);
                buf.push(nonce);
                buf.extend_from_slice(&slope.to_le_bytes());
                buf.extend_from_slice(&mid_price.to_le_bytes());
                buf.push(token_a_decimals);
                buf.push(token_b_decimals);
                buf.extend_from_slice(&token_a_amount.to_le_bytes());
                buf.extend_from_slice(&token_b_amount.to_le_bytes());
                buf.extend_from_slice(&oracle_priority_flags.to_le_bytes());
            }
            Self::Swap(SwapData {
                amount_in,
                minimum_amount_out,
            }) => {
                buf.push(1);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
            Self::Deposit(DepositData {
                token_a_amount,
                token_b_amount,
                min_mint_amount,
            }) => {
                buf.push(2);
                buf.extend_from_slice(&token_a_amount.to_le_bytes());
                buf.extend_from_slice(&token_b_amount.to_le_bytes());
                buf.extend_from_slice(&min_mint_amount.to_le_bytes());
            }
            Self::Withdraw(WithdrawData {
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
            }) => {
                buf.push(3);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_a_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_b_amount.to_le_bytes());
            }
            Self::SetReferrer => {
                buf.push(4);
            }
            Self::SwapV2(SwapData {
                amount_in,
                minimum_amount_out,
            }) => {
                buf.push(5);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
        }
        buf
    }
}

/// Creates an 'initialize' instruction.
pub fn initialize(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    admin_fee_a_pubkey: Pubkey,
    admin_fee_b_pubkey: Pubkey,
    token_a_pubkey: Pubkey,
    token_b_pubkey: Pubkey,
    pool_mint_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    pyth_a_product_pubkey: Pubkey,
    pyth_a_pubkey: Pubkey,
    pyth_b_product_pubkey: Pubkey,
    pyth_b_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    serum_market_pubkey: Pubkey,
    serum_bids_pubkey: Pubkey,
    serum_asks_pubkey: Pubkey,
    init_data: InitializeData,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Initialize(init_data).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, true),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(admin_fee_a_pubkey, false),
        AccountMeta::new_readonly(admin_fee_b_pubkey, false),
        AccountMeta::new_readonly(token_a_pubkey, false),
        AccountMeta::new_readonly(token_b_pubkey, false),
        AccountMeta::new(pool_mint_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new_readonly(pyth_a_product_pubkey, false),
        AccountMeta::new_readonly(pyth_a_pubkey, false),
        AccountMeta::new_readonly(pyth_b_product_pubkey, false),
        AccountMeta::new_readonly(pyth_b_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new_readonly(serum_market_pubkey, false),
        AccountMeta::new_readonly(serum_bids_pubkey, false),
        AccountMeta::new_readonly(serum_asks_pubkey, false),
        AccountMeta::new_readonly(clock::id(), false),
        AccountMeta::new_readonly(rent::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'SetReferrer' instruction.
pub fn set_referrer(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    user_pubkey: Pubkey,
    user_referrer_data_pubkey: Pubkey,
    referrer_token_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::SetReferrer.pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new_readonly(user_pubkey, true),
        AccountMeta::new(user_referrer_data_pubkey, false),
        AccountMeta::new_readonly(referrer_token_pubkey, false),
        AccountMeta::new_readonly(rent::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'swap' instruction.
pub fn swap(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    market_authority_pubkey: Pubkey,
    swap_authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    source_pubkey: Pubkey,
    swap_source_pubkey: Pubkey,
    source_mint_pubkey: Pubkey,
    swap_destination_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    destination_mint_pubkey: Pubkey,
    reward_token_pubkey: Pubkey,
    source_reward_token_pubkey: Pubkey,
    admin_fee_destination_pubkey: Pubkey,
    pyth_a_pubkey: Pubkey,
    pyth_b_pubkey: Pubkey,
    user_referrer_data_pubkey: Option<Pubkey>,
    referrer_token_pubkey: Option<Pubkey>,
    swap_data: SwapData,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Swap(swap_data).pack();

    let mut accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(market_authority_pubkey, false),
        AccountMeta::new_readonly(swap_authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new(swap_source_pubkey, false),
        AccountMeta::new_readonly(source_mint_pubkey, false),
        AccountMeta::new(swap_destination_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new_readonly(destination_mint_pubkey, false),
        AccountMeta::new(reward_token_pubkey, false),
        AccountMeta::new(source_reward_token_pubkey, false),
        AccountMeta::new(admin_fee_destination_pubkey, false),
        AccountMeta::new_readonly(pyth_a_pubkey, false),
        AccountMeta::new_readonly(pyth_b_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(user_referrer_data_pubkey) = user_referrer_data_pubkey {
        accounts.extend_from_slice(&[
            AccountMeta::new_readonly(user_referrer_data_pubkey, false),
            AccountMeta::new(referrer_token_pubkey.unwrap(), false),
        ]);
    }

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'swapV2' instruction.
pub fn swap_v2(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    market_authority_pubkey: Pubkey,
    swap_authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    source_pubkey: Pubkey,
    swap_source_pubkey: Pubkey,
    swap_destination_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    reward_token_pubkey: Pubkey,
    source_reward_token_pubkey: Pubkey,
    admin_fee_destination_pubkey: Pubkey,
    pyth_a_pubkey: Pubkey,
    pyth_b_pubkey: Pubkey,
    serum_market_pubkey: Pubkey,
    serum_bids_pubkey: Pubkey,
    serum_asks_pubkey: Pubkey,
    user_referrer_data_pubkey: Option<Pubkey>,
    referrer_token_pubkey: Option<Pubkey>,
    swap_data: SwapData,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::SwapV2(swap_data).pack();

    let mut accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(market_authority_pubkey, false),
        AccountMeta::new_readonly(swap_authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new(swap_source_pubkey, false),
        AccountMeta::new(swap_destination_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new(reward_token_pubkey, false),
        AccountMeta::new(source_reward_token_pubkey, false),
        AccountMeta::new(admin_fee_destination_pubkey, false),
        AccountMeta::new_readonly(pyth_a_pubkey, false),
        AccountMeta::new_readonly(pyth_b_pubkey, false),
        AccountMeta::new_readonly(serum_market_pubkey, false),
        AccountMeta::new_readonly(serum_bids_pubkey, false),
        AccountMeta::new_readonly(serum_asks_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(user_referrer_data_pubkey) = user_referrer_data_pubkey {
        accounts.extend_from_slice(&[
            AccountMeta::new_readonly(user_referrer_data_pubkey, false),
            AccountMeta::new(referrer_token_pubkey.unwrap(), false),
        ]);
    }

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'deposit' instruction.
pub fn deposit(
    program_id: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    deposit_token_a_pubkey: Pubkey,
    deposit_token_b_pubkey: Pubkey,
    swap_token_a_pubkey: Pubkey,
    swap_token_b_pubkey: Pubkey,
    pool_mint_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    deposit_data: DepositData,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Deposit(deposit_data).pack();

    let accounts = vec![
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(deposit_token_a_pubkey, false),
        AccountMeta::new(deposit_token_b_pubkey, false),
        AccountMeta::new(swap_token_a_pubkey, false),
        AccountMeta::new(swap_token_b_pubkey, false),
        AccountMeta::new(pool_mint_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'withdraw' instruction.
pub fn withdraw(
    program_id: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    pool_mint_pubkey: Pubkey,
    source_pubkey: Pubkey,
    swap_token_a_pubkey: Pubkey,
    swap_token_b_pubkey: Pubkey,
    destination_token_a_pubkey: Pubkey,
    destination_token_b_pubkey: Pubkey,
    admin_fee_a_pubkey: Pubkey,
    admin_fee_b_pubkey: Pubkey,
    withdraw_data: WithdrawData,
) -> Result<Instruction, ProgramError> {
    let data = SwapInstruction::Withdraw(withdraw_data).pack();

    let accounts = vec![
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(pool_mint_pubkey, false),
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new(swap_token_a_pubkey, false),
        AccountMeta::new(swap_token_b_pubkey, false),
        AccountMeta::new(destination_token_a_pubkey, false),
        AccountMeta::new(destination_token_b_pubkey, false),
        AccountMeta::new(admin_fee_a_pubkey, false),
        AccountMeta::new(admin_fee_b_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Swap instructions for stablecoins pool
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum StableSwapInstruction {
    ///   Initializes a new stable swap pool.
    ///
    ///   0. `[]` market config.
    ///   1. `[writable]` new stable-swap to create.
    ///   1. `[]` $swap_authority.
    ///   3. `[]` admin fee account for base token.
    ///   4. `[]` admin fee account for quote token.
    ///   5. `[]` base token account. Must be non zero, owned by $swap_authority.
    ///   6. `[]` quote token account. Must be non zero, owned by $swap_authority.
    ///   7. `[writable]` pool mint account mint by $swap_authority.
    ///   8. `[writable]` pool token account owned by user.
    ///   9. `[]` rent sysvar.
    ///   10. `[]` token program id.
    Initialize(StableInitializeData),

    ///   Swap the tokens in the pool.
    ///
    ///   0. `[]` market config.
    ///   1. `[writable]` stable-swap.
    ///   2. `[]` $market_authority to mint deltafi token.
    ///   3. `[]` $swap_authority.
    ///   4. `[signer]` $user_transfer_authority
    ///   5. `[writable]` SOURCE(base|quote) account, transferable by $user_transfer_authority.
    ///   6. `[writable]` (base|quote) token account to swap INTO. Must be the SOURCE token.
    ///   7.  `[]` Mint account of SOURCE token
    ///   8. `[writable]` (base|quote) token account to swap FROM. Must be the DESTINATION token.
    ///   9. `[writable]` DESTINATION(base|quote) account owned by user.
    ///   10. `[]` Mint account of DESITINATION token
    ///   11. `[writable]` rewards(DELTAFI) token account owned by user.
    ///   12. `[writable]` rewards(DELTAFI) source deltafi token account to issue reward.
    ///   13. `[writable]` (base|quote) admin fee account. Must have same mint as DESTINATION token.
    ///   14. `[]` token program id.
    ///   15. `[]` optional: user referrer data account.
    ///   16. `[writable]` optional: referrer token account.
    Swap(SwapData),

    ///   Deposit some tokens into the pool.  The output is a "pool" token representing ownership
    ///   into the pool. Inputs are converted to the current ratio.
    ///
    ///   0. `[]` stable-swap.
    ///   1. `[]` $swap_authority.
    ///   2. `[signer]` $user_transfer_authority.
    ///   3. `[writable]` base token account to deposit FROM, transferable by $user_transfer_authority.
    ///   4. `[writable]` quote token account to deposit FROM, transferable by $user_transfer_authority.
    ///   5. `[writable]` base token account to deposit INTO.
    ///   6. `[writable]` quote token account to deposit INTO.
    ///   7. `[writable]` pool mint account, mint by $swap_authority.
    ///   8. `[writable]` pool token account owned by user.
    ///   10. `[]` token program id.
    Deposit(DepositData),

    ///   Withdraw tokens from the pool at the current ratio.
    ///
    ///   0. `[]` stable-swap.
    ///   1. `[]` $swap_authority.
    ///   2. `[signer]` $user_transfer_authority.
    ///   3. `[writable]` pool mint account, mint by $swap_autority.
    ///   4. `[writable]` SOURCE pool token account, transferable by $user_transfer_authority.
    ///   5. `[writable]` base token account to withdraw FROM.
    ///   6. `[writable]` quote token account to withdraw FROM.
    ///   7. `[writable]` base token account to withdraw INTO.
    ///   8. `[writable]` quote token account to withdraw INTO.
    ///   9. `[writable]` admin fee account for base token.
    ///   10. `[writable]` admin fee account for quote token.
    ///   12. `[]` token program id.
    Withdraw(WithdrawData),

    ///   Swap the tokens in the pool (removed src and dest mint accounts).
    ///
    ///   0. `[]` market config.
    ///   1. `[writable]` stable-swap.
    ///   2. `[]` $market_authority to mint deltafi token.
    ///   3. `[]` $swap_authority.
    ///   4. `[signer]` $user_transfer_authority
    ///   5. `[writable]` SOURCE(base|quote) account, transferable by $user_transfer_authority.
    ///   6. `[writable]` (base|quote) token account to swap INTO. Must be the SOURCE token.
    ///   7. `[writable]` (base|quote) token account to swap FROM. Must be the DESTINATION token.
    ///   8. `[writable]` DESTINATION(base|quote) account owned by user.
    ///   9. `[writable]` rewards(DELTAFI) token account owned by user.
    ///   10. `[writable]` rewards(DELTAFI) source deltafi token account to issue reward.
    ///   11. `[writable]` (base|quote) admin fee account. Must have same mint as DESTINATION token.
    ///   12. `[]` token program id.
    ///   13. `[]` optional: user referrer data account.
    ///   14. `[writable]` optional: referrer token account.
    SwapV2(SwapData),
}

impl StableSwapInstruction {
    /// Unpacks a byte buffer into a [StableSwapInstruction](enum.StableSwapInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(SwapError::InstructionUnpackError)?;
        Ok(match tag {
            10 => {
                let (&nonce, rest) = rest
                    .split_first()
                    .ok_or(SwapError::InstructionUnpackError)?;
                let (slope, rest) = unpack_u64(rest)?;
                let (token_a_decimals, rest) = unpack_u8(rest)?;
                let (token_b_decimals, rest) = unpack_u8(rest)?;
                let (token_a_amount, rest) = unpack_u64(rest)?;
                let (token_b_amount, _) = unpack_u64(rest)?;
                Self::Initialize(StableInitializeData {
                    nonce,
                    slope,
                    token_a_decimals,
                    token_b_decimals,
                    token_a_amount,
                    token_b_amount,
                })
            }
            11 => {
                let (amount_in, rest) = unpack_u64(rest)?;
                let (minimum_amount_out, _) = unpack_u64(rest)?;
                Self::Swap(SwapData {
                    amount_in,
                    minimum_amount_out,
                })
            }
            12 => {
                let (token_a_amount, rest) = unpack_u64(rest)?;
                let (token_b_amount, rest) = unpack_u64(rest)?;
                let (min_mint_amount, _) = unpack_u64(rest)?;
                Self::Deposit(DepositData {
                    token_a_amount,
                    token_b_amount,
                    min_mint_amount,
                })
            }
            13 => {
                let (pool_token_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_a_amount, rest) = unpack_u64(rest)?;
                let (minimum_token_b_amount, _) = unpack_u64(rest)?;
                Self::Withdraw(WithdrawData {
                    pool_token_amount,
                    minimum_token_a_amount,
                    minimum_token_b_amount,
                })
            }
            14 => {
                let (amount_in, rest) = unpack_u64(rest)?;
                let (minimum_amount_out, _) = unpack_u64(rest)?;
                Self::SwapV2(SwapData {
                    amount_in,
                    minimum_amount_out,
                })
            }
            _ => return Err(SwapError::InvalidInstruction.into()),
        })
    }

    /// Packs a [SwapInstruction](enum.SwapInstruction.html) into a byte buffer.
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::Initialize(StableInitializeData {
                nonce,
                slope,
                token_a_decimals,
                token_b_decimals,
                token_a_amount,
                token_b_amount,
            }) => {
                buf.push(10);
                buf.extend_from_slice(&nonce.to_le_bytes());
                buf.extend_from_slice(&slope.to_le_bytes());
                buf.push(token_a_decimals);
                buf.push(token_b_decimals);
                buf.extend_from_slice(&token_a_amount.to_le_bytes());
                buf.extend_from_slice(&token_b_amount.to_le_bytes());
            }
            Self::Swap(SwapData {
                amount_in,
                minimum_amount_out,
            }) => {
                buf.push(11);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
            Self::Deposit(DepositData {
                token_a_amount,
                token_b_amount,
                min_mint_amount,
            }) => {
                buf.push(12);
                buf.extend_from_slice(&token_a_amount.to_le_bytes());
                buf.extend_from_slice(&token_b_amount.to_le_bytes());
                buf.extend_from_slice(&min_mint_amount.to_le_bytes());
            }
            Self::Withdraw(WithdrawData {
                pool_token_amount,
                minimum_token_a_amount,
                minimum_token_b_amount,
            }) => {
                buf.push(13);
                buf.extend_from_slice(&pool_token_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_a_amount.to_le_bytes());
                buf.extend_from_slice(&minimum_token_b_amount.to_le_bytes());
            }
            Self::SwapV2(SwapData {
                amount_in,
                minimum_amount_out,
            }) => {
                buf.push(14);
                buf.extend_from_slice(&amount_in.to_le_bytes());
                buf.extend_from_slice(&minimum_amount_out.to_le_bytes());
            }
        }
        buf
    }
}

/// Creates 'stable_initialize' instruction.
pub fn stable_initialize(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    admin_fee_a_pubkey: Pubkey,
    admin_fee_b_pubkey: Pubkey,
    token_a_pubkey: Pubkey,
    token_b_pubkey: Pubkey,
    pool_mint_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    init_data: StableInitializeData,
) -> Result<Instruction, ProgramError> {
    let data = StableSwapInstruction::Initialize(init_data).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, true),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(admin_fee_a_pubkey, false),
        AccountMeta::new_readonly(admin_fee_b_pubkey, false),
        AccountMeta::new_readonly(token_a_pubkey, false),
        AccountMeta::new_readonly(token_b_pubkey, false),
        AccountMeta::new(pool_mint_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new_readonly(rent::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'stable_swap' instruction.
pub fn stable_swap(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    market_authority_pubkey: Pubkey,
    swap_authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    source_pubkey: Pubkey,
    swap_source_pubkey: Pubkey,
    source_mint_info: Pubkey,
    swap_destination_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    destination_mint_info: Pubkey,
    reward_token_pubkey: Pubkey,
    source_reward_token_pubkey: Pubkey,
    admin_fee_destination_pubkey: Pubkey,
    user_referrer_data_pubkey: Option<Pubkey>,
    referrer_token_pubkey: Option<Pubkey>,
    swap_data: SwapData,
) -> Result<Instruction, ProgramError> {
    let data = StableSwapInstruction::Swap(swap_data).pack();

    let mut accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(market_authority_pubkey, false),
        AccountMeta::new_readonly(swap_authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new(swap_source_pubkey, false),
        AccountMeta::new_readonly(source_mint_info, false),
        AccountMeta::new(swap_destination_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new_readonly(destination_mint_info, false),
        AccountMeta::new(reward_token_pubkey, false),
        AccountMeta::new(source_reward_token_pubkey, false),
        AccountMeta::new(admin_fee_destination_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(user_referrer_data_pubkey) = user_referrer_data_pubkey {
        accounts.extend_from_slice(&[
            AccountMeta::new_readonly(user_referrer_data_pubkey, false),
            AccountMeta::new(referrer_token_pubkey.unwrap(), false),
        ]);
    }

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'stable_swap_v2' instruction.
pub fn stable_swap_v2(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    swap_pubkey: Pubkey,
    market_authority_pubkey: Pubkey,
    swap_authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    source_pubkey: Pubkey,
    swap_source_pubkey: Pubkey,
    swap_destination_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    reward_token_pubkey: Pubkey,
    source_reward_token_pubkey: Pubkey,
    admin_fee_destination_pubkey: Pubkey,
    user_referrer_data_pubkey: Option<Pubkey>,
    referrer_token_pubkey: Option<Pubkey>,
    swap_data: SwapData,
) -> Result<Instruction, ProgramError> {
    let data = StableSwapInstruction::SwapV2(swap_data).pack();

    let mut accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(market_authority_pubkey, false),
        AccountMeta::new_readonly(swap_authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new(swap_source_pubkey, false),
        AccountMeta::new(swap_destination_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new(reward_token_pubkey, false),
        AccountMeta::new(source_reward_token_pubkey, false),
        AccountMeta::new(admin_fee_destination_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    if let Some(user_referrer_data_pubkey) = user_referrer_data_pubkey {
        accounts.extend_from_slice(&[
            AccountMeta::new_readonly(user_referrer_data_pubkey, false),
            AccountMeta::new(referrer_token_pubkey.unwrap(), false),
        ]);
    }

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'stable_deposit' instruction.
pub fn stable_deposit(
    program_id: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    deposit_token_a_pubkey: Pubkey,
    deposit_token_b_pubkey: Pubkey,
    swap_token_a_pubkey: Pubkey,
    swap_token_b_pubkey: Pubkey,
    pool_mint_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    deposit_data: DepositData,
) -> Result<Instruction, ProgramError> {
    let data = StableSwapInstruction::Deposit(deposit_data).pack();

    let accounts = vec![
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(deposit_token_a_pubkey, false),
        AccountMeta::new(deposit_token_b_pubkey, false),
        AccountMeta::new(swap_token_a_pubkey, false),
        AccountMeta::new(swap_token_b_pubkey, false),
        AccountMeta::new(pool_mint_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// Creates a 'stable_withdraw' instruction.
pub fn stable_withdraw(
    program_id: Pubkey,
    swap_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    pool_mint_pubkey: Pubkey,
    source_pubkey: Pubkey,
    swap_token_a_pubkey: Pubkey,
    swap_token_b_pubkey: Pubkey,
    destination_token_a_pubkey: Pubkey,
    destination_token_b_pubkey: Pubkey,
    admin_fee_a_pubkey: Pubkey,
    admin_fee_b_pubkey: Pubkey,
    withdraw_data: WithdrawData,
) -> Result<Instruction, ProgramError> {
    let data = StableSwapInstruction::Withdraw(withdraw_data).pack();

    let accounts = vec![
        AccountMeta::new(swap_pubkey, false),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(pool_mint_pubkey, false),
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new(swap_token_a_pubkey, false),
        AccountMeta::new(swap_token_b_pubkey, false),
        AccountMeta::new(destination_token_a_pubkey, false),
        AccountMeta::new(destination_token_b_pubkey, false),
        AccountMeta::new(admin_fee_a_pubkey, false),
        AccountMeta::new(admin_fee_b_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

/// FARM INSTRUNCTION DATA
/// Initialize instruction data
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct FarmInitializeData {
    /// Withdraw fee numerator
    pub fee_numerator: u64,
    /// Withdraw fee denominator
    pub fee_denominator: u64,
    /// Rewards numerator
    pub rewards_numerator: u64,
    /// Rewards denominator
    pub rewards_denominator: u64,
    /// Bump seed
    pub bump_seed: u8,
}

/// Farm deposit instruction data
#[cfg_attr(feature = "fuzz", derive(Arbitrary, Clone))]
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct FarmDepositData {
    /// Amount to stake
    pub amount: u64,
}

/// Farm withdraw instruction data
#[cfg_attr(feature = "fuzz", derive(Arbitrary, Clone))]
#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct FarmWithdrawData {
    /// Amount to withdraw
    pub amount: u64,
}

/// Instructions supported by the pool FarmInfo program.
#[repr(C)]
#[derive(Debug, PartialEq)]
pub enum FarmInstruction {
    /// Intialize farm info
    Initialize(FarmInitializeData),
    /// Initialize farm user
    InitializeFarmUser,
    /// Farm claim
    Claim,
    /// Farm refresh
    Refresh,
    /// Farm deposit
    Deposit(FarmDepositData),
    /// Farm withdraw
    Withdraw(FarmWithdrawData),
}

impl FarmInstruction {
    /// Unpacks a byte buffer into a [FarmInstruction](enum.FarmInstruction.html).
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;
        Ok(match tag {
            20 => {
                let (fee_numerator, rest) = unpack_u64(rest)?;
                let (fee_denominator, rest) = unpack_u64(rest)?;
                let (rewards_numerator, rest) = unpack_u64(rest)?;
                let (rewards_denominator, rest) = unpack_u64(rest)?;
                let (bump_seed, _) = unpack_u8(rest)?;

                Self::Initialize(FarmInitializeData {
                    fee_numerator,
                    fee_denominator,
                    rewards_numerator,
                    rewards_denominator,
                    bump_seed,
                })
            }
            21 => Self::InitializeFarmUser,
            22 => Self::Claim,
            23 => Self::Refresh,
            24 => {
                let (amount, _) = unpack_u64(rest)?;
                Self::Deposit(FarmDepositData { amount })
            }
            25 => {
                let (amount, _) = unpack_u64(rest)?;
                Self::Withdraw(FarmWithdrawData { amount })
            }
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }

    /// Packs a [FarmInstruction](enum.FarmInstruction.html) into a byte buffer
    pub fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        match *self {
            Self::Initialize(FarmInitializeData {
                fee_numerator,
                fee_denominator,
                rewards_numerator,
                rewards_denominator,
                bump_seed,
            }) => {
                buf.push(20);
                buf.extend_from_slice(&fee_numerator.to_le_bytes());
                buf.extend_from_slice(&fee_denominator.to_le_bytes());
                buf.extend_from_slice(&rewards_numerator.to_le_bytes());
                buf.extend_from_slice(&rewards_denominator.to_le_bytes());
                buf.extend_from_slice(&bump_seed.to_le_bytes());
            }
            Self::InitializeFarmUser => {
                buf.push(21);
            }
            Self::Claim => buf.push(22),
            Self::Refresh => buf.push(23),
            Self::Deposit(FarmDepositData { amount }) => {
                buf.push(24);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            Self::Withdraw(FarmWithdrawData { amount }) => {
                buf.push(25);
                buf.extend_from_slice(&amount.to_le_bytes());
            }
        }

        buf
    }
}

/// Creates `FarmInitialize` instruction
pub fn farm_initialize(
    program_id: Pubkey,
    config_info_pubkey: Pubkey,
    swap_info_pubkey: Pubkey,
    farm_pool_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    pool_token_pubkey: Pubkey,
    admin_pubkey: Pubkey,
    init_data: FarmInitializeData,
) -> Result<Instruction, ProgramError> {
    let data = FarmInstruction::Initialize(init_data).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_info_pubkey, false),
        AccountMeta::new_readonly(swap_info_pubkey, false),
        AccountMeta::new(farm_pool_pubkey, true),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new_readonly(pool_token_pubkey, false),
        AccountMeta::new_readonly(admin_pubkey, true),
        AccountMeta::new_readonly(rent::id(), false),
    ];

    Ok(Instruction {
        program_id,
        data,
        accounts,
    })
}

/// Creates `FarmUserInitialize` instruction
pub fn farm_user_initialize(
    program_id: Pubkey,
    config_info_pubkey: Pubkey,
    farm_pool_pubkey: Pubkey,
    farm_user_pubkey: Pubkey,
    farm_owner_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = FarmInstruction::InitializeFarmUser.pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_info_pubkey, false),
        AccountMeta::new_readonly(farm_pool_pubkey, false),
        AccountMeta::new(farm_user_pubkey, false),
        AccountMeta::new_readonly(farm_owner_pubkey, true),
        AccountMeta::new_readonly(rent::id(), false),
    ];

    Ok(Instruction {
        program_id,
        data,
        accounts,
    })
}

/// Creates `FarmDeposit` instruction
pub fn farm_deposit(
    program_id: Pubkey,
    config_key: Pubkey,
    farm_pool_pubkey: Pubkey,
    user_transfer_authority_pubkey: Pubkey,
    source_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    farm_user_pubkey: Pubkey,
    farm_owner_pubkey: Pubkey,
    deposit_data: FarmDepositData,
) -> Result<Instruction, ProgramError> {
    let data = FarmInstruction::Deposit(deposit_data).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_key, false),
        AccountMeta::new(farm_pool_pubkey, false),
        AccountMeta::new_readonly(user_transfer_authority_pubkey, true),
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new(farm_user_pubkey, false),
        AccountMeta::new_readonly(farm_owner_pubkey, true),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        data,
        accounts,
    })
}

/// Creates `FarmWithdraw` instruction
pub fn farm_withdraw(
    program_id: Pubkey,
    config_key: Pubkey,
    farm_pool_pubkey: Pubkey,
    farm_user_pubkey: Pubkey,
    authority_pubkey: Pubkey,
    source_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    farm_owner_pubkey: Pubkey,
    withdraw_data: FarmWithdrawData,
) -> Result<Instruction, ProgramError> {
    let data = FarmInstruction::Withdraw(withdraw_data).pack();

    let accounts = vec![
        AccountMeta::new_readonly(config_key, false),
        AccountMeta::new(farm_pool_pubkey, false),
        AccountMeta::new(farm_user_pubkey, false),
        AccountMeta::new_readonly(authority_pubkey, false),
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new(destination_pubkey, false),
        AccountMeta::new_readonly(farm_owner_pubkey, true),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];

    Ok(Instruction {
        program_id,
        data,
        accounts,
    })
}

/// Creates farm claim instruction
pub fn farm_claim(
    program_id: Pubkey,
    config_pubkey: Pubkey,
    farm_pool_pubkey: Pubkey,
    farm_user_pubkey: Pubkey,
    farm_owner_pubkey: Pubkey,
    market_authority_pubkey: Pubkey,
    claim_destination_pubkey: Pubkey,
    claim_source_pubkey: Pubkey,
) -> Result<Instruction, ProgramError> {
    let data = FarmInstruction::Claim.pack();
    let accounts = vec![
        AccountMeta::new_readonly(config_pubkey, false),
        AccountMeta::new_readonly(farm_pool_pubkey, false),
        AccountMeta::new(farm_user_pubkey, false),
        AccountMeta::new_readonly(farm_owner_pubkey, true),
        AccountMeta::new_readonly(market_authority_pubkey, false),
        AccountMeta::new(claim_destination_pubkey, false),
        AccountMeta::new(claim_source_pubkey, false),
        AccountMeta::new_readonly(spl_token::id(), false),
    ];
    Ok(Instruction {
        program_id,
        accounts,
        data,
    })
}

fn unpack_u128(input: &[u8]) -> Result<(u128, &[u8]), ProgramError> {
    if input.len() < 16 {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (amount, rest) = input.split_at(16);
    let amount = amount
        .get(..16)
        .and_then(|slice| slice.try_into().ok())
        .map(u128::from_le_bytes)
        .ok_or(SwapError::InstructionUnpackError)?;
    Ok((amount, rest))
}

#[allow(dead_code)]
fn unpack_i64(input: &[u8]) -> Result<(i64, &[u8]), ProgramError> {
    if input.len() < 8 {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (amount, rest) = input.split_at(8);
    let amount = amount
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .map(i64::from_le_bytes)
        .ok_or(SwapError::InstructionUnpackError)?;
    Ok((amount, rest))
}

fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
    if input.len() < 8 {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (amount, rest) = input.split_at(8);
    let amount = amount
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(SwapError::InstructionUnpackError)?;
    Ok((amount, rest))
}

fn unpack_u8(input: &[u8]) -> Result<(u8, &[u8]), ProgramError> {
    if input.is_empty() {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (bytes, rest) = input.split_at(1);
    let value = bytes
        .get(..1)
        .and_then(|slice| slice.try_into().ok())
        .map(u8::from_le_bytes)
        .ok_or(SwapError::InstructionUnpackError)?;
    Ok((value, rest))
}

#[allow(dead_code)]
fn unpack_bytes32(input: &[u8]) -> Result<(&[u8; 32], &[u8]), ProgramError> {
    if input.len() < 32 {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (bytes, rest) = input.split_at(32);
    Ok((
        bytes
            .try_into()
            .map_err(|_| SwapError::InstructionUnpackError)?,
        rest,
    ))
}

#[allow(dead_code)]
fn unpack_pubkey(input: &[u8]) -> Result<(Pubkey, &[u8]), ProgramError> {
    if input.len() < PUBKEY_BYTES {
        return Err(SwapError::InstructionUnpackError.into());
    }
    let (key, rest) = input.split_at(PUBKEY_BYTES);
    let pk = Pubkey::new(key);
    Ok((pk, rest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        curve::{default_market_price, default_slope},
        state::{DEFAULT_TEST_FEES, DEFAULT_TEST_REWARDS},
    };

    #[test]
    fn test_instruction_type_check() {
        assert!(matches!(
            InstructionType::check(&[101u8, 1u8]),
            Some(InstructionType::Admin)
        ));
        assert!(matches!(
            InstructionType::check(&[1u8, 1u8]),
            Some(InstructionType::Swap)
        ));
        assert!(matches!(
            InstructionType::check(&[12u8, 1u8]),
            Some(InstructionType::StableSwap)
        ));
        assert!(matches!(
            InstructionType::check(&[21u8, 1u8]),
            Some(InstructionType::Farm)
        ));
        assert!(InstructionType::check(&[15u8, 1u8]).is_none());
    }

    #[test]
    fn test_pack_admin_init_config() {
        let fees = DEFAULT_TEST_FEES;
        let rewards = DEFAULT_TEST_REWARDS;
        let check = AdminInstruction::Initialize(AdminInitializeData {
            fees: fees.clone(),
            rewards: rewards.clone(),
        });
        let packed = check.pack();
        let mut expect = vec![100];
        let is_initialized = vec![1, fees.is_initialized as u8];
        expect.extend_from_slice(&is_initialized[0].to_le_bytes());
        expect.extend_from_slice(&fees.admin_trade_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_trade_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_withdraw_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_withdraw_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.trade_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.trade_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.withdraw_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.withdraw_fee_denominator.to_le_bytes());
        let is_initialized = vec![1, rewards.is_initialized as u8];
        expect.extend_from_slice(&is_initialized[0].to_le_bytes());
        expect.extend_from_slice(&rewards.decimals.to_le_bytes());
        expect.extend_from_slice(&rewards.reserved);
        expect.extend_from_slice(&rewards.trade_reward_numerator.to_le_bytes());
        expect.extend_from_slice(&rewards.trade_reward_denominator.to_le_bytes());
        expect.extend_from_slice(&rewards.trade_reward_cap.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_admin_set_new_fees() {
        let fees = DEFAULT_TEST_FEES;
        let check = AdminInstruction::SetNewFees(fees.clone());
        let packed = check.pack();
        let mut expect = vec![105];
        let is_initialized = vec![1, fees.is_initialized as u8];
        expect.extend_from_slice(&is_initialized[0].to_le_bytes());
        expect.extend_from_slice(&fees.admin_trade_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_trade_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_withdraw_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.admin_withdraw_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.trade_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.trade_fee_denominator.to_le_bytes());
        expect.extend_from_slice(&fees.withdraw_fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fees.withdraw_fee_denominator.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_admin_set_new_rewards() {
        let rewards = DEFAULT_TEST_REWARDS;
        let check = AdminInstruction::SetNewRewards(rewards.clone());
        let packed = check.pack();
        let mut expect = vec![106];
        let is_initialized = vec![1, rewards.is_initialized as u8];
        expect.extend_from_slice(&is_initialized[0].to_le_bytes());
        expect.extend_from_slice(&rewards.decimals.to_le_bytes());
        expect.extend_from_slice(&rewards.reserved);
        expect.extend_from_slice(&rewards.trade_reward_numerator.to_le_bytes());
        expect.extend_from_slice(&rewards.trade_reward_denominator.to_le_bytes());
        expect.extend_from_slice(&rewards.trade_reward_cap.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_set_farm_rewards() {
        let farm_rewards = FarmRewards {
            apr_numerator: 12,
            apr_denominator: 100,
        };
        let check = AdminInstruction::SetFarmRewards(farm_rewards.clone());
        let packed = check.pack();
        let mut expect = vec![107];
        expect.extend_from_slice(&farm_rewards.apr_numerator.to_le_bytes());
        expect.extend_from_slice(&farm_rewards.apr_denominator.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_set_slope() {
        let slope = 123_123_123u64;
        let check = AdminInstruction::SetSlope(slope);
        let packed = check.pack();
        let mut expect = vec![108];
        expect.extend_from_slice(&slope.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_set_decimals() {
        let base_decimals = 9u8;
        let quote_decimals = 6u8;
        let check = AdminInstruction::SetDecimals(base_decimals, quote_decimals);
        let packed = check.pack();
        let mut expect = vec![109];
        expect.extend_from_slice(&base_decimals.to_le_bytes());
        expect.extend_from_slice(&quote_decimals.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_set_swap_limit() {
        let swap_out_limit_percentage = 10u8;
        let check = AdminInstruction::SetSwapLimit(swap_out_limit_percentage);
        let packed = check.pack();
        let mut expect = vec![110];
        expect.extend_from_slice(&swap_out_limit_percentage.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_admin_pause() {
        let check = AdminInstruction::Pause;
        let packed = check.pack();
        let expect = vec![101];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_admin_unpause() {
        let check = AdminInstruction::Unpause;
        let packed = check.pack();
        let expect = vec![102];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_admin_set_fee_account() {
        let check = AdminInstruction::SetFeeAccount;
        let packed = check.pack();
        let expect = vec![103];
        assert_eq!(packed, expect);
        let unpacked = AdminInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_admin_error() {
        let result = AdminInstruction::unpack(&[111, 2]);
        let expect = Err(ProgramError::from(SwapError::InvalidInstruction));
        assert_eq!(expect, result);
    }

    #[test]
    fn test_initialize_config() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let market_authority_pubkey = Pubkey::new_unique();
        let deltafi_mint_pubkey = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let pyth_program_id = Pubkey::new_unique();
        let fees = DEFAULT_TEST_FEES;
        let rewards = DEFAULT_TEST_REWARDS;
        let deltafi_token_pubkey = Pubkey::new_unique();

        let result = initialize_config(
            program_id,
            config_pubkey,
            market_authority_pubkey,
            deltafi_mint_pubkey,
            admin_pubkey,
            pyth_program_id,
            fees.clone(),
            rewards.clone(),
            deltafi_token_pubkey,
        );

        assert!(result.is_ok());

        let mut expected_data = vec![100];
        let is_initialized = vec![1, fees.is_initialized as u8];
        expected_data.extend_from_slice(&is_initialized[0].to_le_bytes());
        expected_data.extend_from_slice(&fees.admin_trade_fee_numerator.to_le_bytes());
        expected_data.extend_from_slice(&fees.admin_trade_fee_denominator.to_le_bytes());
        expected_data.extend_from_slice(&fees.admin_withdraw_fee_numerator.to_le_bytes());
        expected_data.extend_from_slice(&fees.admin_withdraw_fee_denominator.to_le_bytes());
        expected_data.extend_from_slice(&fees.trade_fee_numerator.to_le_bytes());
        expected_data.extend_from_slice(&fees.trade_fee_denominator.to_le_bytes());
        expected_data.extend_from_slice(&fees.withdraw_fee_numerator.to_le_bytes());
        expected_data.extend_from_slice(&fees.withdraw_fee_denominator.to_le_bytes());
        let is_initialized = vec![1, rewards.is_initialized as u8];
        expected_data.extend_from_slice(&is_initialized[0].to_le_bytes());
        expected_data.extend_from_slice(&rewards.decimals.to_le_bytes());
        expected_data.extend_from_slice(&rewards.reserved);
        expected_data.extend_from_slice(&rewards.trade_reward_numerator.to_le_bytes());
        expected_data.extend_from_slice(&rewards.trade_reward_denominator.to_le_bytes());
        expected_data.extend_from_slice(&rewards.trade_reward_cap.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: market_authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: deltafi_mint_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: rent::id(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pyth_program_id,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: deltafi_token_pubkey,
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_pause() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let swap_pubkey = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();

        let result = pause(program_id, config_pubkey, swap_pubkey, admin_pubkey);
        let expected_data = vec![101];

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_unpause() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let swap_pubkey = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();

        let result = unpause(program_id, config_pubkey, swap_pubkey, admin_pubkey);
        let expected_data = vec![102];

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_set_fee_account() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let swap_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let new_fee_account_pubkey = Pubkey::new_unique();

        let result = set_fee_account(
            program_id,
            config_pubkey,
            swap_pubkey,
            authority_pubkey,
            admin_pubkey,
            new_fee_account_pubkey,
        );

        let expected_data = vec![103];

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: new_fee_account_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_commit_new_admin() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let deltafi_mint_pubkey = Pubkey::new_unique();
        let new_admin_key = Pubkey::new_unique();

        let result = commit_new_admin(
            program_id,
            config_pubkey,
            admin_pubkey,
            deltafi_mint_pubkey,
            new_admin_key,
        );

        let mut expected_data = vec![104];
        expected_data.extend_from_slice(new_admin_key.as_ref());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: deltafi_mint_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_set_new_fees() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let swap_pubkey = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let new_fees = DEFAULT_TEST_FEES;

        let result = set_new_fees(
            program_id,
            config_pubkey,
            swap_pubkey,
            admin_pubkey,
            new_fees.clone(),
        );

        let mut expected_data = vec![105];
        let is_initialized = vec![1, new_fees.is_initialized as u8];
        expected_data.extend_from_slice(&is_initialized[0].to_le_bytes());
        expected_data.extend_from_slice(&new_fees.admin_trade_fee_numerator.to_le_bytes());
        expected_data.extend_from_slice(&new_fees.admin_trade_fee_denominator.to_le_bytes());
        expected_data.extend_from_slice(&new_fees.admin_withdraw_fee_numerator.to_le_bytes());
        expected_data.extend_from_slice(&new_fees.admin_withdraw_fee_denominator.to_le_bytes());
        expected_data.extend_from_slice(&new_fees.trade_fee_numerator.to_le_bytes());
        expected_data.extend_from_slice(&new_fees.trade_fee_denominator.to_le_bytes());
        expected_data.extend_from_slice(&new_fees.withdraw_fee_numerator.to_le_bytes());
        expected_data.extend_from_slice(&new_fees.withdraw_fee_denominator.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_set_new_rewards() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let swap_pubkey = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let new_rewards = DEFAULT_TEST_REWARDS;

        let result = set_new_rewards(
            program_id,
            config_pubkey,
            swap_pubkey,
            admin_pubkey,
            new_rewards.clone(),
        );

        let mut expected_data = vec![106];
        let is_initialized = vec![1, new_rewards.is_initialized as u8];
        expected_data.extend_from_slice(&is_initialized[0].to_le_bytes());
        expected_data.extend_from_slice(&new_rewards.decimals.to_le_bytes());
        expected_data.extend_from_slice(&new_rewards.reserved);
        expected_data.extend_from_slice(&new_rewards.trade_reward_numerator.to_le_bytes());
        expected_data.extend_from_slice(&new_rewards.trade_reward_denominator.to_le_bytes());
        expected_data.extend_from_slice(&new_rewards.trade_reward_cap.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_set_farm_rewards() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let farm_pool_info = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let new_rewards = FarmRewards {
            apr_numerator: DEFAULT_TEST_REWARDS.trade_reward_numerator,
            apr_denominator: DEFAULT_TEST_REWARDS.trade_reward_denominator,
        };

        let result = set_farm_rewards(
            program_id,
            config_pubkey,
            farm_pool_info,
            admin_pubkey,
            new_rewards.clone(),
        );

        let mut expected_data = vec![107];
        expected_data.extend_from_slice(&new_rewards.apr_numerator.to_le_bytes());
        expected_data.extend_from_slice(&new_rewards.apr_denominator.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: farm_pool_info,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_set_slope() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let pool_info = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let slope = 123_123_123u64;

        let result = set_slope(program_id, config_pubkey, pool_info, admin_pubkey, slope);

        let mut expected_data = vec![108];
        expected_data.extend_from_slice(&slope.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pool_info,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_set_decimals() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let pool_info = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let token_a_decimals = 6u8;
        let token_b_decimals = 9u8;

        let result = set_decimals(
            program_id,
            config_pubkey,
            pool_info,
            admin_pubkey,
            token_a_decimals,
            token_b_decimals,
        );

        let mut expected_data = vec![109];
        expected_data.extend_from_slice(&token_a_decimals.to_le_bytes());
        expected_data.extend_from_slice(&token_b_decimals.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pool_info,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_set_swap_limit() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let pool_info = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let swap_out_limit_percentage = 10u8;

        let result = set_swap_limit(
            program_id,
            config_pubkey,
            pool_info,
            admin_pubkey,
            swap_out_limit_percentage,
        );

        let mut expected_data = vec![110];
        expected_data.extend_from_slice(&swap_out_limit_percentage.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pool_info,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_pack_swap_initialization() {
        let nonce: u8 = 255;
        let slope: u64 = default_slope().to_scaled_val().unwrap().try_into().unwrap();
        let mid_price = default_market_price().to_scaled_val().unwrap();
        let token_a_decimals = 9u8;
        let token_b_decimals = 9u8;
        let token_a_amount = 1000u64;
        let token_b_amount = 2000u64;
        let oracle_priority_flags = 0u8;
        let check = SwapInstruction::Initialize(InitializeData {
            nonce,
            slope,
            mid_price,
            token_a_decimals,
            token_b_decimals,
            token_a_amount,
            token_b_amount,
            oracle_priority_flags,
        });
        let packed = check.pack();
        let mut expect = vec![0];
        expect.extend_from_slice(&nonce.to_le_bytes());
        expect.extend_from_slice(&slope.to_le_bytes());
        expect.extend_from_slice(&mid_price.to_le_bytes());
        expect.extend_from_slice(&token_a_decimals.to_le_bytes());
        expect.extend_from_slice(&token_b_decimals.to_le_bytes());
        expect.extend_from_slice(&token_a_amount.to_le_bytes());
        expect.extend_from_slice(&token_b_amount.to_le_bytes());
        expect.extend_from_slice(&oracle_priority_flags.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_swap() {
        let amount_in: u64 = 1_000_000;
        let minimum_amount_out: u64 = 500_000;
        let check = SwapInstruction::Swap(SwapData {
            amount_in,
            minimum_amount_out,
        });
        let packed = check.pack();
        let mut expect = vec![1];
        expect.extend_from_slice(&amount_in.to_le_bytes());
        expect.extend_from_slice(&minimum_amount_out.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_swap_deposit() {
        let token_a_amount: u64 = 1_000_000;
        let token_b_amount: u64 = 500_000;
        let min_mint_amount: u64 = 500_000;
        let check = SwapInstruction::Deposit(DepositData {
            token_a_amount,
            token_b_amount,
            min_mint_amount,
        });
        let packed = check.pack();
        let mut expect = vec![2];
        expect.extend_from_slice(&token_a_amount.to_le_bytes());
        expect.extend_from_slice(&token_b_amount.to_le_bytes());
        expect.extend_from_slice(&min_mint_amount.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_swap_withdraw() {
        let minimum_token_a_amount: u64 = 1_000_000;
        let minimum_token_b_amount: u64 = 500_000;
        let pool_token_amount: u64 = 500_000;
        let check = SwapInstruction::Withdraw(WithdrawData {
            pool_token_amount,
            minimum_token_a_amount,
            minimum_token_b_amount,
        });
        let packed = check.pack();
        let mut expect = vec![3];
        expect.extend_from_slice(&pool_token_amount.to_le_bytes());
        expect.extend_from_slice(&minimum_token_a_amount.to_le_bytes());
        expect.extend_from_slice(&minimum_token_b_amount.to_le_bytes());
        assert_eq!(packed, expect);
        let unpacked = SwapInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_initialize() {
        let nonce: u8 = 255;
        let slope: u64 = default_slope().to_scaled_val().unwrap().try_into().unwrap();
        let mid_price = default_market_price().to_scaled_val().unwrap();

        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let swap_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let admin_fee_a_pubkey = Pubkey::new_unique();
        let admin_fee_b_pubkey = Pubkey::new_unique();
        let token_a_pubkey = Pubkey::new_unique();
        let token_b_pubkey = Pubkey::new_unique();
        let pool_mint_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let pyth_a_product_pubkey = Pubkey::new_unique();
        let pyth_a_pubkey = Pubkey::new_unique();
        let pyth_b_product_pubkey = Pubkey::new_unique();
        let pyth_b_pubkey = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let token_a_decimals = 9u8;
        let token_b_decimals = 9u8;
        let token_a_amount = 1_000_000u64;
        let token_b_amount = 1_000_000u64;
        let oracle_priority_flags = 0u8;
        let serum_market_pubkey = Pubkey::new_unique();
        let serum_bids_pubkey = Pubkey::new_unique();
        let serum_asks_pubkey = Pubkey::new_unique();
        let init_data = InitializeData {
            nonce,
            slope,
            mid_price,
            token_a_decimals,
            token_b_decimals,
            token_a_amount,
            token_b_amount,
            oracle_priority_flags,
        };
        let init_data_clone = InitializeData {
            nonce,
            slope,
            mid_price,
            token_a_decimals,
            token_b_decimals,
            token_a_amount,
            token_b_amount,
            oracle_priority_flags,
        };

        let result = initialize(
            program_id,
            config_pubkey,
            swap_pubkey,
            authority_pubkey,
            admin_fee_a_pubkey,
            admin_fee_b_pubkey,
            token_a_pubkey,
            token_b_pubkey,
            pool_mint_pubkey,
            destination_pubkey,
            pyth_a_product_pubkey,
            pyth_a_pubkey,
            pyth_b_product_pubkey,
            pyth_b_pubkey,
            admin_pubkey,
            serum_market_pubkey,
            serum_bids_pubkey,
            serum_asks_pubkey,
            init_data_clone,
        );

        let mut expected_data = vec![0];
        expected_data.extend_from_slice(&init_data.nonce.to_le_bytes());
        expected_data.extend_from_slice(&init_data.slope.to_le_bytes());
        expected_data.extend_from_slice(&init_data.mid_price.to_le_bytes());
        expected_data.extend_from_slice(&init_data.token_a_decimals.to_le_bytes());
        expected_data.extend_from_slice(&init_data.token_b_decimals.to_le_bytes());
        expected_data.extend_from_slice(&init_data.token_a_amount.to_le_bytes());
        expected_data.extend_from_slice(&init_data.token_b_amount.to_le_bytes());
        expected_data.extend_from_slice(&init_data.oracle_priority_flags.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_pubkey,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: admin_fee_a_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: admin_fee_b_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: token_a_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: token_b_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pool_mint_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: pyth_a_product_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pyth_a_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pyth_b_product_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pyth_b_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: serum_market_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: serum_bids_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: serum_asks_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: clock::id(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: rent::id(),
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_swap() {
        let amount_in: u64 = 1_000_000;
        let minimum_amount_out: u64 = 500_000;

        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let swap_pubkey = Pubkey::new_unique();
        let market_authority_pubkey = Pubkey::new_unique();
        let swap_authority_pubkey = Pubkey::new_unique();
        let user_transfer_authority_pubkey = Pubkey::new_unique();
        let source_pubkey = Pubkey::new_unique();
        let swap_source_pubkey = Pubkey::new_unique();
        let source_mint_pubkey = Pubkey::new_unique();
        let swap_destination_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let destination_mint_pubkey = Pubkey::new_unique();
        let reward_token_pubkey = Pubkey::new_unique();
        let reward_mint_pubkey = Pubkey::new_unique();
        let admin_fee_destination_pubkey = Pubkey::new_unique();
        let pyth_a_pubkey = Pubkey::new_unique();
        let pyth_b_pubkey = Pubkey::new_unique();
        let swap_data = SwapData {
            amount_in,
            minimum_amount_out,
        };

        let result = swap(
            program_id,
            config_pubkey,
            swap_pubkey,
            market_authority_pubkey,
            swap_authority_pubkey,
            user_transfer_authority_pubkey,
            source_pubkey,
            swap_source_pubkey,
            source_mint_pubkey,
            swap_destination_pubkey,
            destination_pubkey,
            destination_mint_pubkey,
            reward_token_pubkey,
            reward_mint_pubkey,
            admin_fee_destination_pubkey,
            pyth_a_pubkey,
            pyth_b_pubkey,
            None,
            None,
            swap_data.clone(),
        );

        let mut expected_data = vec![1];
        expected_data.extend_from_slice(&swap_data.amount_in.to_le_bytes());
        expected_data.extend_from_slice(&swap_data.minimum_amount_out.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: market_authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: user_transfer_authority_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: source_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: swap_source_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: source_mint_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: destination_mint_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: reward_token_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: reward_mint_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_fee_destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: pyth_a_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pyth_b_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_swap_v2() {
        let amount_in: u64 = 1_000_000;
        let minimum_amount_out: u64 = 500_000;

        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let swap_pubkey = Pubkey::new_unique();
        let market_authority_pubkey = Pubkey::new_unique();
        let swap_authority_pubkey = Pubkey::new_unique();
        let user_transfer_authority_pubkey = Pubkey::new_unique();
        let source_pubkey = Pubkey::new_unique();
        let swap_source_pubkey = Pubkey::new_unique();
        let swap_destination_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let reward_token_pubkey = Pubkey::new_unique();
        let reward_mint_pubkey = Pubkey::new_unique();
        let admin_fee_destination_pubkey = Pubkey::new_unique();
        let pyth_a_pubkey = Pubkey::new_unique();
        let pyth_b_pubkey = Pubkey::new_unique();
        let serum_market_pubkey = Pubkey::new_unique();
        let serum_bids_pubkey = Pubkey::new_unique();
        let serum_asks_pubkey = Pubkey::new_unique();
        let swap_data = SwapData {
            amount_in,
            minimum_amount_out,
        };

        let result = swap_v2(
            program_id,
            config_pubkey,
            swap_pubkey,
            market_authority_pubkey,
            swap_authority_pubkey,
            user_transfer_authority_pubkey,
            source_pubkey,
            swap_source_pubkey,
            swap_destination_pubkey,
            destination_pubkey,
            reward_token_pubkey,
            reward_mint_pubkey,
            admin_fee_destination_pubkey,
            pyth_a_pubkey,
            pyth_b_pubkey,
            serum_market_pubkey,
            serum_bids_pubkey,
            serum_asks_pubkey,
            None,
            None,
            swap_data.clone(),
        );

        let mut expected_data = vec![5];
        expected_data.extend_from_slice(&swap_data.amount_in.to_le_bytes());
        expected_data.extend_from_slice(&swap_data.minimum_amount_out.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: market_authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: user_transfer_authority_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: source_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: swap_source_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: swap_destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: reward_token_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: reward_mint_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_fee_destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: pyth_a_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pyth_b_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: serum_market_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: serum_bids_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: serum_asks_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_deposit() {
        let token_a_amount: u64 = 1_000_000;
        let token_b_amount: u64 = 500_000;
        let min_mint_amount: u64 = 500_000;

        let program_id = Pubkey::new_unique();
        let swap_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let user_transfer_authority_pubkey = Pubkey::new_unique();
        let deposit_token_a_pubkey = Pubkey::new_unique();
        let deposit_token_b_pubkey = Pubkey::new_unique();
        let swap_token_a_pubkey = Pubkey::new_unique();
        let swap_token_b_pubkey = Pubkey::new_unique();
        let pool_mint_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let deposit_data = DepositData {
            token_a_amount,
            token_b_amount,
            min_mint_amount,
        };

        let result = deposit(
            program_id,
            swap_pubkey,
            authority_pubkey,
            user_transfer_authority_pubkey,
            deposit_token_a_pubkey,
            deposit_token_b_pubkey,
            swap_token_a_pubkey,
            swap_token_b_pubkey,
            pool_mint_pubkey,
            destination_pubkey,
            deposit_data.clone(),
        );

        let mut expected_data = vec![2];
        expected_data.extend_from_slice(&deposit_data.token_a_amount.to_le_bytes());
        expected_data.extend_from_slice(&deposit_data.token_b_amount.to_le_bytes());
        expected_data.extend_from_slice(&deposit_data.min_mint_amount.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: swap_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: user_transfer_authority_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: deposit_token_a_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: deposit_token_b_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: swap_token_a_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: swap_token_b_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: pool_mint_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_withdraw() {
        let pool_token_amount: u64 = 500_000;
        let minimum_token_a_amount: u64 = 1_000_000;
        let minimum_token_b_amount: u64 = 500_000;

        let program_id = Pubkey::new_unique();
        let swap_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let user_transfer_authority_pubkey = Pubkey::new_unique();
        let pool_mint_pubkey = Pubkey::new_unique();
        let source_pubkey = Pubkey::new_unique();
        let swap_token_a_pubkey = Pubkey::new_unique();
        let swap_token_b_pubkey = Pubkey::new_unique();
        let destination_token_a_pubkey = Pubkey::new_unique();
        let destination_token_b_pubkey = Pubkey::new_unique();
        let admin_fee_a_pubkey = Pubkey::new_unique();
        let admin_fee_b_pubkey = Pubkey::new_unique();
        let withdraw_data = WithdrawData {
            pool_token_amount,
            minimum_token_a_amount,
            minimum_token_b_amount,
        };

        let result = withdraw(
            program_id,
            swap_pubkey,
            authority_pubkey,
            user_transfer_authority_pubkey,
            pool_mint_pubkey,
            source_pubkey,
            swap_token_a_pubkey,
            swap_token_b_pubkey,
            destination_token_a_pubkey,
            destination_token_b_pubkey,
            admin_fee_a_pubkey,
            admin_fee_b_pubkey,
            withdraw_data.clone(),
        );

        let mut expected_data = vec![3];
        expected_data.extend_from_slice(&withdraw_data.pool_token_amount.to_le_bytes());
        expected_data.extend_from_slice(&withdraw_data.minimum_token_a_amount.to_le_bytes());
        expected_data.extend_from_slice(&withdraw_data.minimum_token_b_amount.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: swap_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: user_transfer_authority_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pool_mint_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: source_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: swap_token_a_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: swap_token_b_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: destination_token_a_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: destination_token_b_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_fee_a_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: admin_fee_b_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_set_referrer() {
        let set_referrer_instruction = SwapInstruction::SetReferrer;
        let packed = set_referrer_instruction.pack();

        let expected = vec![4u8];
        assert_eq!(expected, packed);

        let unpacked = SwapInstruction::unpack(&expected);

        assert_eq!(unpacked, Ok(set_referrer_instruction));
    }

    #[test]
    fn test_pack_farm_initialization() {
        let fee_numerator = 1;
        let fee_denominator = 1;
        let rewards_numerator = 1;
        let rewards_denominator = 1;
        let bump_seed = 1;
        let check = FarmInstruction::Initialize(FarmInitializeData {
            fee_numerator,
            fee_denominator,
            rewards_numerator,
            rewards_denominator,
            bump_seed,
        });

        let packed = check.pack();
        let mut expect = vec![20];
        expect.extend_from_slice(&fee_numerator.to_le_bytes());
        expect.extend_from_slice(&fee_denominator.to_le_bytes());
        expect.extend_from_slice(&rewards_numerator.to_le_bytes());
        expect.extend_from_slice(&rewards_denominator.to_le_bytes());
        expect.extend_from_slice(&bump_seed.to_le_bytes());
        assert_eq!(packed, expect);

        let unpacked = FarmInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_farm_initialize_farm_user() {
        let check = FarmInstruction::InitializeFarmUser;

        let packed = check.pack();
        let expect = vec![21];

        assert_eq!(packed, expect);

        let unpacked = FarmInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_farm_claim() {
        let check = FarmInstruction::Claim;

        let packed = check.pack();
        let expect = vec![22];

        assert_eq!(packed, expect);

        let unpacked = FarmInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_farm_deposit() {
        let amount = 1_000_000;
        let check = FarmInstruction::Deposit(FarmDepositData { amount });

        let packed = check.pack();
        let mut expect = vec![24];
        expect.extend_from_slice(&amount.to_le_bytes());
        assert_eq!(packed, expect);

        let unpacked = FarmInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_pack_farm_withdraw() {
        let amount = 1_000_000;
        let check = FarmInstruction::Withdraw(FarmWithdrawData { amount });

        let packed = check.pack();
        let mut expect = vec![25];
        expect.extend_from_slice(&amount.to_le_bytes());
        assert_eq!(packed, expect);

        let unpacked = FarmInstruction::unpack(&expect).unwrap();
        assert_eq!(unpacked, check);
    }

    #[test]
    fn test_farm_initialize() {
        let fee_numerator = 1;
        let fee_denominator = 1;
        let rewards_numerator = 1;
        let rewards_denominator = 1;
        let bump_seed = 1;

        let program_id = Pubkey::new_unique();
        let config_info_pubkey = Pubkey::new_unique();
        let swap_info_pubkey = Pubkey::new_unique();
        let farm_pool_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let pool_token_pubkey = Pubkey::new_unique();
        let admin_pubkey = Pubkey::new_unique();
        let init_data = FarmInitializeData {
            fee_numerator,
            fee_denominator,
            rewards_numerator,
            rewards_denominator,
            bump_seed,
        };

        let result = farm_initialize(
            program_id,
            config_info_pubkey,
            swap_info_pubkey,
            farm_pool_pubkey,
            authority_pubkey,
            pool_token_pubkey,
            admin_pubkey,
            init_data,
        );

        let mut expected_data = vec![20];
        expected_data.extend_from_slice(&fee_numerator.to_le_bytes());
        expected_data.extend_from_slice(&fee_denominator.to_le_bytes());
        expected_data.extend_from_slice(&rewards_numerator.to_le_bytes());
        expected_data.extend_from_slice(&rewards_denominator.to_le_bytes());
        expected_data.extend_from_slice(&bump_seed.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_info_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: swap_info_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: farm_pool_pubkey,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: pool_token_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: admin_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: rent::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_farm_user_initialize() {
        let program_id = Pubkey::new_unique();
        let config_info_pubkey = Pubkey::new_unique();
        let farm_pool_pubkey = Pubkey::new_unique();
        let farm_user_pubkey = Pubkey::new_unique();
        let farm_owner_pubkey = Pubkey::new_unique();

        let result = farm_user_initialize(
            program_id,
            config_info_pubkey,
            farm_pool_pubkey,
            farm_user_pubkey,
            farm_owner_pubkey,
        );

        let expected_data = vec![21];

        let expected_account = vec![
            AccountMeta {
                pubkey: config_info_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: farm_pool_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: farm_user_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: farm_owner_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: rent::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_farm_deposit() {
        let amount = 1_000_000;
        let program_id = Pubkey::new_unique();
        let config_key = Pubkey::new_unique();
        let farm_pool_pubkey = Pubkey::new_unique();
        let user_transfer_authority_pubkey = Pubkey::new_unique();
        let source_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let farm_user_pubkey = Pubkey::new_unique();
        let farm_owner_pubkey = Pubkey::new_unique();
        let deposit_data = FarmDepositData { amount };

        let result = farm_deposit(
            program_id,
            config_key,
            farm_pool_pubkey,
            user_transfer_authority_pubkey,
            source_pubkey,
            destination_pubkey,
            farm_user_pubkey,
            farm_owner_pubkey,
            deposit_data,
        );

        let mut expected_data = vec![24];
        expected_data.extend_from_slice(&amount.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_key,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: farm_pool_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: user_transfer_authority_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: source_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: farm_user_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: farm_owner_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_farm_withdraw() {
        let amount = 1_000_000;
        let program_id = Pubkey::new_unique();
        let config_key = Pubkey::new_unique();
        let farm_pool_pubkey = Pubkey::new_unique();
        let farm_user_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let source_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let farm_owner_pubkey = Pubkey::new_unique();
        let withdraw_data = FarmWithdrawData { amount };

        let result = farm_withdraw(
            program_id,
            config_key,
            farm_pool_pubkey,
            farm_user_pubkey,
            authority_pubkey,
            source_pubkey,
            destination_pubkey,
            farm_owner_pubkey,
            withdraw_data,
        );

        let mut expected_data = vec![25];
        expected_data.extend_from_slice(&amount.to_le_bytes());

        let expected_account = vec![
            AccountMeta {
                pubkey: config_key,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: farm_pool_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: farm_user_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: source_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: farm_owner_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }

    #[test]
    fn test_farm_claim() {
        let program_id = Pubkey::new_unique();
        let config_pubkey = Pubkey::new_unique();
        let farm_pool_pubkey = Pubkey::new_unique();
        let farm_user_pubkey = Pubkey::new_unique();
        let farm_owner_pubkey = Pubkey::new_unique();
        let market_authority_pubkey = Pubkey::new_unique();
        let claim_destination_pubkey = Pubkey::new_unique();
        let claim_mint_pubkey = Pubkey::new_unique();

        let result = farm_claim(
            program_id,
            config_pubkey,
            farm_pool_pubkey,
            farm_user_pubkey,
            farm_owner_pubkey,
            market_authority_pubkey,
            claim_destination_pubkey,
            claim_mint_pubkey,
        );

        let expected_data = vec![22];
        let expected_account = vec![
            AccountMeta {
                pubkey: config_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: farm_pool_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: farm_user_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: farm_owner_pubkey,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: market_authority_pubkey,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: claim_destination_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: claim_mint_pubkey,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: spl_token::id(),
                is_signer: false,
                is_writable: false,
            },
        ];

        assert_eq!(result.as_ref().unwrap().data, expected_data);
        assert_eq!(result.as_ref().unwrap().accounts, expected_account);
    }
}
