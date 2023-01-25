import { struct, u8, u16, u32 } from 'buffer-layout';
import { bool, u64 } from '../util/layout';
export interface Rewards {
  isInitialized: boolean;
  decimals: number;
  reserved8: number,
  reserved16: number,
  reserved32: number,
  tradeRewardNumerator: bigint;
  tradeRewardDenominator: bigint;
  tradeRewardCap: bigint;
}

/** @internal */
export const RewardsLayout = (property = 'rewards') =>
  struct<Rewards>(
    [
      bool('isInitialized'),
      u8('decimals'),
      u8('reserved8'),
      u16('reserved16'),
      u32('reserved32'),
      u64('tradeRewardNumerator'),
      u64('tradeRewardDenominator'),
      u64('tradeRewardCap'),
    ],
    property
  );

export interface FarmRewards {
  aprNumerator: bigint;
  aprDenominator: bigint;
}

/** @internal */
export const FarmRewardsLayout = (property = 'farmRewards') =>
  struct<FarmRewards>([u64('aprNumerator'), u64('aprDenominator')], property);
