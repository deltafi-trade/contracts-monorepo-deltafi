import { TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { PublicKey, SYSVAR_CLOCK_PUBKEY, SYSVAR_RENT_PUBKEY, TransactionInstruction } from '@solana/web3.js';
import { struct, u8 } from 'buffer-layout';

import { decimal, u64 } from '../util';

export enum StableSwapInstruction {
  Initialize = 10,
  Swap,
  Deposit,
  Withdraw,
}

export interface InitializeStableData {
  nonce: number;
  slope: number | bigint;
}

/** @internal */
export const InitializeStableDataLayout = struct<InitializeStableData>(
  [
    u8('nonce'),
    u64('slope'),
    u8('tokenADecimals'),
    u8('tokenBDecimals'),
    u64('tokenAAmount'),
    u64('tokenBAmount'),
  ],
  'initData'
);

export const createInitStableSwapInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  authority: PublicKey,
  adminFeeKeyA: PublicKey,
  adminFeeKeyB: PublicKey,
  tokenA: PublicKey,
  tokenB: PublicKey,
  poolMint: PublicKey,
  poolToken: PublicKey,
  admin: PublicKey,
  initData: InitializeStableData,
  programId: PublicKey
): TransactionInstruction => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: true, isWritable: true },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminFeeKeyA, isSigner: false, isWritable: false },
    { pubkey: adminFeeKeyB, isSigner: false, isWritable: false },
    { pubkey: tokenA, isSigner: false, isWritable: false },
    { pubkey: tokenB, isSigner: false, isWritable: false },
    { pubkey: poolMint, isSigner: false, isWritable: true },
    { pubkey: poolToken, isSigner: false, isWritable: true },
    { pubkey: admin, isSigner: true, isWritable: false },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
  ];
  const dataLayout = struct([u8('instruction'), InitializeStableDataLayout]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: StableSwapInstruction.Initialize,
      initData,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    programId,
    data,
  });
};
