import { Fees, Rewards } from '../src';

export const DEFAULT_FEE_NUMERATOR = 5;
export const DEFAULT_FEE_DENOMINATOR = 1000;
export const DEFAULT_FEES: Fees = {
  adminTradeFeeNumerator: BigInt(DEFAULT_FEE_NUMERATOR),
  adminTradeFeeDenominator: BigInt(DEFAULT_FEE_DENOMINATOR),
  adminWithdrawFeeNumerator: BigInt(DEFAULT_FEE_NUMERATOR),
  adminWithdrawFeeDenominator: BigInt(DEFAULT_FEE_DENOMINATOR),
  tradeFeeNumerator: BigInt(DEFAULT_FEE_NUMERATOR),
  tradeFeeDenominator: BigInt(DEFAULT_FEE_DENOMINATOR),
  withdrawFeeNumerator: BigInt(DEFAULT_FEE_NUMERATOR),
  withdrawFeeDenominator: BigInt(DEFAULT_FEE_DENOMINATOR),
};

export const DEFAULT_THRESHOLD = 10000;
export const DEFAULT_REWARD_NUMERATOR = 1;
export const DEFAULT_REWARD_DENOMINATOR = 1000;
export const DEFAULT_REWARD_CAP = 100;
export const DEFAULT_LIQUIDITY_NUMERATOR = 1;
export const DEFAULT_LIQUIDITY_DENOMINATOR = 1000;
export const DEFAULT_REWARDS: Rewards = {
  isInitialized: true,
  decimals: 9,
  tradeRewardNumerator: BigInt(DEFAULT_REWARD_NUMERATOR),
  tradeRewardDenominator: BigInt(DEFAULT_REWARD_DENOMINATOR),
  tradeRewardCap: BigInt(DEFAULT_REWARD_CAP),
};

export const CLUSTER_URL = 'http://localhost:8899';
export const BOOTSTRAP_TIMEOUT = 10000;
export const AMP_FACTOR = 100;
export const K = 0.5;
export const I = 100;
export const MIN_AMP = 1;
export const MAX_AMP = 1000000;
export const MIN_RAMP_DURATION = 86400;
