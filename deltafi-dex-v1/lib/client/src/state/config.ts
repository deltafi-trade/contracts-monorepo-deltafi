import { AccountInfo, PublicKey, Connection } from '@solana/web3.js';
import { struct, u8, blob } from 'buffer-layout';

import { publicKey, AccountParser, loadAccount } from '../util';
import { Fees, FeesLayout } from './fees';
import { Rewards, RewardsLayout } from './rewards';

export interface MintInfo {
  decimals: number
}

/** @internal */
export const MintLayout = struct<MintInfo>([blob(44), u8("decimals"), blob(37)], 'MintLayout');

export interface ConfigInfo {
  version: number;
  bumpSeed: number;
  adminKey: PublicKey;
  detafiMint: PublicKey;
  pythProgramId: PublicKey;
  fees: Fees;
  rewards: Rewards;
}

/** @internal */
export const ConfigInfoLayout = struct<ConfigInfo>(
  [
    u8('version'),
    u8('bumpSeed'),
    publicKey('adminKey'),
    publicKey('deltafiMint'),
    publicKey('pythProgramId'),
    FeesLayout('fees'),
    RewardsLayout('rewards'),
    publicKey('deltafiToken'),
    blob(128, 'reserved'),
  ],
  'configInfo'
);

export const CONFIG_SIZE = ConfigInfoLayout.span;

export const isConfigInfo = (info: AccountInfo<Buffer>): boolean => {
  return info.data.length === CONFIG_SIZE;
};

export const parserConfigInfo: AccountParser<ConfigInfo> = (pubkey: PublicKey, info: AccountInfo<Buffer>) => {
  if (!isConfigInfo(info)) return;

  const buffer = Buffer.from(info.data);
  const configInfo = ConfigInfoLayout.decode(buffer);

  if (!configInfo.version) return;

  return {
    pubkey,
    info,
    data: configInfo,
  };
};

export const loadConfig = async (
  connection: Connection,
  address: PublicKey,
  programId: PublicKey
): Promise<ConfigInfo> => {
  const accountInfo = await loadAccount(connection, address, programId);

  const parsed = parserConfigInfo(address, accountInfo);

  if (!parsed) throw new Error('Failed to load configuration account');

  return parsed.data;
};
