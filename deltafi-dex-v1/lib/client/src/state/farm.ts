import { AccountInfo, PublicKey, Connection } from '@solana/web3.js';
import { blob, seq, struct, u8 } from 'buffer-layout';

import { AccountParser, bool, publicKey, u64 } from '../util/layout';
import { loadAccount } from '../util/account';

export interface FarmInfo {
  isInitialized: boolean;
  bumpSeed: number;
  configKey: PublicKey;
  poolMint: PublicKey;
  poolToken: PublicKey;
  reservedAmount: bigint;
  feeNumerator: bigint;
  feeDenominator: bigint;
  rewardsNumerator: bigint;
  rewardsDenominator: bigint;
}

/** @internal */
export const FarmInfoLayout = struct<FarmInfo>(
  [
    bool('isInitialized'),
    u8('bumpSeed'),
    publicKey('configKey'),
    publicKey('poolMint'),
    publicKey('poolToken'),
    u64('reservedAmount'),
    u64('feeNumerator'),
    u64('feeDenominator'),
    u64('rewardsNumerator'),
    u64('rewardsDenominator'),
    blob(64, 'reserved')
  ],
  'farmInfo'
);

export const FARM_INFO_SIZE = FarmInfoLayout.span;

export const isFarmInfo = (info: AccountInfo<Buffer>): boolean => {
  return info.data.length === FARM_INFO_SIZE;
};

export const parseFarmInfo: AccountParser<FarmInfo> = (pubkey: PublicKey, info: AccountInfo<Buffer>) => {
  if (!isFarmInfo(info)) return;

  const buffer = Buffer.from(info.data);
  const farmInfo = FarmInfoLayout.decode(buffer);

  if (!farmInfo.isInitialized) return;

  return {
    pubkey,
    info,
    data: farmInfo,
  };
};

export const loadFarmInfo = async (
  connection: Connection,
  key: string,
  farmProgramId: PublicKey
): Promise<{ key: string; data: FarmInfo }> => {
  const address = new PublicKey(key);
  const accountInfo = await loadAccount(connection, address, farmProgramId);

  const parsed = parseFarmInfo(address, accountInfo);

  if (!parsed) throw new Error('Failed to load farm info account');

  return {
    key,
    data: parsed.data,
  };
};

export interface FarmPosition {
  pool: PublicKey;
  depositedAmount: bigint;
  rewardsOwed: bigint;
  rewardsEstimated: bigint;
  cumulativeInterest: bigint;
  lastUpdateTs: bigint;
  nextClaimTs: bigint;
}

/** @internal */
export const FarmPositionLayout = struct<FarmPosition>([
  publicKey('pool'),
  u64('depositedAmount'),
  u64('rewardsOwed'),
  u64('rewardsEstimated'),
  u64('cumulativeInterest'),
  u64('lastUpdateTs'),
  u64('nextClaimTs'),
  u64('latestDepositSlot'),
]);

export const FARM_POSITION_SIZE = FarmPositionLayout.span;

export interface FarmUser {
  isInitialized: boolean;
  configKey: PublicKey;
  owner: PublicKey;
}

export interface FarmUserDataFlat {
  isInitialized: boolean;
  configKey: PublicKey;
  owner: PublicKey;
  positionLen: number;
  dataFlat: Buffer;
}

/** @internal */
export const FarmUserLayout = struct<FarmUserDataFlat>(
  [
    bool('isInitialized'),
    publicKey('configKey'),
    publicKey('farmPoolKey'),
    publicKey('owner'),
    u8('positionLen'),
    publicKey('dummy'),
    u64('depositedAmount'),
    u64('rewardsOwed'),
    u64('rewardsEstimated'),
    u64('cumulativeInterest'),
    u64('lastUpdateTs'),
    u64('nextClaimTs'),
    u64('latestDepositSlot'),
    blob(64, 'reserved'),
  ],
  'farmUser'
);

export const FARM_USER_SIZE = FarmUserLayout.span;

export const isFarmUser = (info: AccountInfo<Buffer>) => info.data.length === FARM_USER_SIZE;

export const parseFarmUser: AccountParser<FarmUser> = (pubkey: PublicKey, info: AccountInfo<Buffer>) => {
  if (!isFarmUser(info)) return;

  const buffer = Buffer.from(info.data);
  const farmUser = FarmUserLayout.decode(buffer);

  const { isInitialized } = farmUser;
  if (!isInitialized) return;
  return {
    pubkey,
    info,
    data: farmUser,
  };
};

export const loadFarmUser = async (
  connection: Connection,
  key: string,
  farmProgramId: PublicKey
): Promise<{ data: FarmUser; key: string }> => {
  const address = new PublicKey(key);
  const accountInfo = await loadAccount(connection, address, farmProgramId);

  const parsed = parseFarmUser(address, accountInfo);

  if (!parsed) throw new Error('Failed to load farm user account');

  return {
    key,
    data: parsed.data,
  };
};
