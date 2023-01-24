import { AccountInfo, PublicKey, Connection } from '@solana/web3.js';
import { struct, u8, blob } from 'buffer-layout';
import BigNumber from 'bignumber.js';

import { publicKey, u64, bool, AccountParser, decimal, loadAccount } from '../util';
import { Fees, FeesLayout } from './fees';
import { Rewards, RewardsLayout } from './rewards';
import { PoolState, PoolStateLayout } from './pool-state';

export enum SwapType {
  Normal = 0,
  Stable,
}

export interface SwapInfo {
  isInitialized: boolean;
  isPaused: boolean;
  nonce: number;
  swapType: SwapType;
  configKey: PublicKey;
  tokenA: PublicKey;
  tokenB: PublicKey;
  pythA: PublicKey;
  pythB: PublicKey;
  poolMint: PublicKey;
  tokenMintA: PublicKey;
  tokenMintB: PublicKey;
  adminFeeKeyA: PublicKey;
  adminFeeKeyB: PublicKey;
  fees: Fees;
  rewards: Rewards;
  poolState: PoolState;
  tokenADecimals: number,
  tokenBDecimals: number,
  swapOutLimitPercentage: number,
  oraclePriorityFlags: number;
  serumCombinedAddress: PublicKey;
}

/** @internal */
export const SwapInfoLayout = struct<SwapInfo>(
  [
    bool('isInitialized'),
    bool('isPaused'),
    u8('nonce'),
    u8('swapType'),
    publicKey('configKey'),
    publicKey('tokenA'),
    publicKey('tokenB'),
    publicKey('pythA'),
    publicKey('pythB'),
    publicKey('poolMint'),
    publicKey('tokenMintA'),
    publicKey('tokenMintB'),
    publicKey('adminFeeKeyA'),
    publicKey('adminFeeKeyB'),
    FeesLayout('fees'),
    RewardsLayout('rewards'),
    PoolStateLayout('poolState'),
    u8("tokenADecimals"),
    u8("tokenBDecimals"),
    u8("swapOutLimitPercentage"),
    u8("oraclePriorityFlags"),
    publicKey("serumCombinedAddress"),
    blob(28, 'reserved'),
  ],
  'swapInfo'
);

export const SWAP_INFO_SIZE = SwapInfoLayout.span;

export const isSwapInfo = (info: AccountInfo<Buffer>): boolean => {
  return info.data.length === SWAP_INFO_SIZE;
};

export const parseSwapInfo: AccountParser<SwapInfo> = (pubkey: PublicKey, info: AccountInfo<Buffer>) => {
  if (!isSwapInfo(info)) return;

  const buffer = Buffer.from(info.data);
  const swapInfo = SwapInfoLayout.decode(buffer);

  if (!swapInfo.isInitialized) return;

  return {
    pubkey,
    info,
    data: swapInfo,
  };
};

export const loadSwapInfo = async (
  connection: Connection,
  address: PublicKey,
  swapProgramId: PublicKey
): Promise<SwapInfo> => {
  const accountInfo = await loadAccount(connection, address, swapProgramId);

  const parsed = parseSwapInfo(address, accountInfo);

  if (!parsed) throw new Error('Failed to load swap account');

  return parsed.data;
};

export interface UserReferrerData {
  isInitialized: boolean;
  configKey: PublicKey;
  owner: PublicKey;
  referrer: PublicKey;
}

/** @internal */
export const UserReferrerDataLayout = struct<UserReferrerData>(
  [
    bool('isInitialized'),
    publicKey('configKey'),
    publicKey('owner'),
    publicKey('referrer'),
  ],
  'userReferrerData'
);

export const USER_REFERRER_DATA_SIZE = UserReferrerDataLayout.span;
