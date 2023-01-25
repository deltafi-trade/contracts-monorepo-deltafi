import { PublicKey, SYSVAR_RENT_PUBKEY, TransactionInstruction } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { struct, u8 } from 'buffer-layout';

import { Fees, FeesLayout, Rewards, RewardsLayout, FarmRewards, FarmRewardsLayout } from '../state';
import { publicKey } from '../util';
import { u64 } from '../util/layout';

export enum AdminInstruction {
  Initialize = 100,
  Pause,
  Unpause,
  SetFeeAccount,
  CommitNewAdmin,
  SetNewFees,
  SetNewRewards,
  SetFarmRewards,
  SetNewSlope,
  SetDecimals,
  SetSwapLimit,
}

export interface AdminInitializeData {
  fees: Fees;
  rewards: Rewards;
}

/** @internal */
export const AdminInitializeDataLayout = struct<AdminInitializeData>(
  [FeesLayout('fees'), RewardsLayout('rewards')],
  'initData'
);

export const createAdminInitializeInstruction = (
  config: PublicKey,
  authority: PublicKey,
  deltafiMint: PublicKey,
  adminKey: PublicKey,
  pythProgramId: PublicKey,
  deltafiToken: PublicKey,
  initData: AdminInitializeData,
  programId: PublicKey
): TransactionInstruction => {
  const keys = [
    { pubkey: config, isSigner: true, isWritable: true },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: deltafiMint, isSigner: false, isWritable: false },
    { pubkey: adminKey, isSigner: true, isWritable: false },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: pythProgramId, isSigner: false, isWritable: false },
    { pubkey: deltafiToken, isSigner: false, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction'), AdminInitializeDataLayout]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.Initialize,
      initData,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createPauseInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction')]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.Pause,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createUnpauseInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction')]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.Unpause,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetFeeAccountInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  authority: PublicKey,
  adminKey: PublicKey,
  newFeeAccount: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: false, isWritable: true },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminKey, isSigner: true, isWritable: false },
    { pubkey: newFeeAccount, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction')]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.SetFeeAccount,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createCommitNewAdminInstruction = (
  config: PublicKey,
  adminKey: PublicKey,
  deltafiMint: PublicKey,
  newAdminKey: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
    { pubkey: deltafiMint, isSigner: false, isWritable: true },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction'), publicKey('newAdminKey')]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.CommitNewAdmin,
      newAdminKey,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetNewFeesInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  newFees: Fees,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction'), FeesLayout('newFees')]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.SetNewFees,
      newFees,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetNewRewardsInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  newRewards: Rewards,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction'), RewardsLayout('newRewards')]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.SetNewRewards,
      newRewards,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetFarmRewardsInstruction = (
  config: PublicKey,
  farmPool: PublicKey,
  admin: PublicKey,
  newRewards: FarmRewards,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: farmPool, isSigner: false, isWritable: true },
    { pubkey: admin, isSigner: true, isWritable: false },
  ];

  const dataLayout = struct([u8('instruction'), FarmRewardsLayout('newRewards')]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.SetFarmRewards,
      newRewards,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetNewSlopeInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  newSlope: bigint,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction'), u64('newSlope')]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.SetNewSlope,
      newSlope,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};


export const createSetDecimalsInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  baseDecimals: number,
  quoteDecimals: number,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction'), u8('baseDecimals'), u8('quoteDecimals')]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.SetDecimals,
      baseDecimals,
      quoteDecimals,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};


export const createSetSwapLimitInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  swapOutLimitPercentage: number,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction'), u8('swapOutLimitPercentage')]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.SetSwapLimit,
      swapOutLimitPercentage,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};
