import type { Connection } from '@solana/web3.js';
import { Keypair, PublicKey, SystemProgram, Transaction } from '@solana/web3.js';
import { MintLayout, AccountLayout, Token, TOKEN_PROGRAM_ID } from '@solana/spl-token';

import { getMinBalanceRentForExempt, sendAndConfirmTransaction } from '../util';
import { ConfigInfoLayout, Fees, Rewards, FarmRewards } from '../state';
import {
  AdminInitializeData,
  createAdminInitializeInstruction,
  createPauseInstruction,
  createUnpauseInstruction,
  createSetFeeAccountInstruction,
  createCommitNewAdminInstruction,
  createSetNewFeesInstruction,
  createSetNewRewardsInstruction,
  createSetFarmRewardsInstruction,
  createSetNewSlopeInstruction,
} from '../instructions';
import { DECIMALS, SWAP_PROGRAM_ID, PYTH_PROGRAM_ID } from '../constants';

export const initializeConfig = async (
  connection: Connection,
  payer: Keypair,
  adminAccount: Keypair,
  initData: AdminInitializeData
) => {
  const configAccount = Keypair.generate();
  const deltafiMint = Keypair.generate();
  const deltafiToken = Keypair.generate();

  const [marketAuthority] = await PublicKey.findProgramAddress([configAccount.publicKey.toBuffer()], SWAP_PROGRAM_ID);

  const balanceForConfig = await getMinBalanceRentForExempt(connection, ConfigInfoLayout.span);
  const balanceForMint = await getMinBalanceRentForExempt(connection, MintLayout.span);
  const balanceForToken = await getMinBalanceRentForExempt(connection, AccountLayout.span);
  const transaction = new Transaction()
    .add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: deltafiMint.publicKey,
        lamports: balanceForMint,
        space: MintLayout.span,
        programId: TOKEN_PROGRAM_ID,
      })
    )
    .add(
      Token.createInitMintInstruction(
        TOKEN_PROGRAM_ID,
        deltafiMint.publicKey,
        DECIMALS,
        marketAuthority,
        adminAccount.publicKey
      )
    )
    .add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: deltafiToken.publicKey,
        lamports: balanceForToken * 2,
        space: AccountLayout.span,
        programId: TOKEN_PROGRAM_ID,
      })
    )
    .add(
      Token.createInitAccountInstruction(
        TOKEN_PROGRAM_ID,
        deltafiMint.publicKey,
        deltafiToken.publicKey,
        marketAuthority
      )
    )
    .add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: configAccount.publicKey,
        lamports: balanceForConfig,
        space: ConfigInfoLayout.span,
        programId: SWAP_PROGRAM_ID
      })
    )
    .add(
      createAdminInitializeInstruction(
        configAccount.publicKey,
        marketAuthority,
        deltafiMint.publicKey,
        adminAccount.publicKey,
        PYTH_PROGRAM_ID,
        deltafiToken.publicKey,
        initData,
        SWAP_PROGRAM_ID
      )
    );

  await sendAndConfirmTransaction(
    'create and initialize ConfigInfo account',
    connection,
    transaction,
    payer,
    configAccount,
    adminAccount,
    deltafiMint,
    deltafiToken,
  );

  return { config: configAccount.publicKey, deltafiMint: deltafiMint.publicKey };
};

export const pause = async (
  connection: Connection,
  payer: Keypair,
  config: PublicKey,
  tokenSwap: PublicKey,
  admin: Keypair
) => {
  const transaction = new Transaction().add(
    createPauseInstruction(config, tokenSwap, admin.publicKey, SWAP_PROGRAM_ID)
  );

  await sendAndConfirmTransaction('pause swap pool', connection, transaction, payer, admin);
};

export const unpause = async (
  connection: Connection,
  payer: Keypair,
  config: PublicKey,
  tokenSwap: PublicKey,
  admin: Keypair
) => {
  const transaction = new Transaction().add(
    createUnpauseInstruction(config, tokenSwap, admin.publicKey, SWAP_PROGRAM_ID)
  );

  await sendAndConfirmTransaction('unpause swap pool', connection, transaction, payer, admin);
};

export const setFeeAccount = async (
  connection: Connection,
  payer: Keypair,
  config: PublicKey,
  tokenSwap: PublicKey,
  newFeeAccount: PublicKey,
  admin: Keypair
) => {
  const [authority] = await PublicKey.findProgramAddress([tokenSwap.toBuffer()], SWAP_PROGRAM_ID);

  const transaction = new Transaction().add(
    createSetFeeAccountInstruction(config, tokenSwap, authority, admin.publicKey, newFeeAccount, SWAP_PROGRAM_ID)
  );

  await sendAndConfirmTransaction('set new fee account', connection, transaction, payer, admin);
};

export const commitNewAdmin = async (
  connection: Connection,
  payer: Keypair,
  config: PublicKey,
  deltafiMint: PublicKey,
  newAdminKey: PublicKey,
  admin: Keypair
) => {
  const transaction = new Transaction().add(
    createCommitNewAdminInstruction(config, admin.publicKey, deltafiMint, newAdminKey, SWAP_PROGRAM_ID)
  );

  await sendAndConfirmTransaction('commit new admin account', connection, transaction, payer, admin);
};

export const commitNewFees = async (
  connection: Connection,
  payer: Keypair,
  config: PublicKey,
  tokenSwap: PublicKey,
  fees: Fees,
  admin: Keypair
) => {
  const transaction = new Transaction().add(
    createSetNewFeesInstruction(config, tokenSwap, admin.publicKey, fees, SWAP_PROGRAM_ID)
  );

  await sendAndConfirmTransaction('commit new fee configuration', connection, transaction, payer, admin);
};

export const commitNewRewards = async (
  connection: Connection,
  payer: Keypair,
  config: PublicKey,
  tokenSwap: PublicKey,
  rewards: Rewards,
  admin: Keypair
) => {
  const transaction = new Transaction().add(
    createSetNewRewardsInstruction(config, tokenSwap, admin.publicKey, rewards, SWAP_PROGRAM_ID)
  );

  await sendAndConfirmTransaction('commit new rewards configuration', connection, transaction, payer, admin);
};

export const setFarmRewards = async (
  connection: Connection,
  payer: Keypair,
  config: PublicKey,
  farmPool: PublicKey,
  rewards: FarmRewards,
  admin: Keypair
) => {
  const transaction = new Transaction().add(
    createSetFarmRewardsInstruction(config, farmPool, admin.publicKey, rewards, SWAP_PROGRAM_ID)
  );

  await sendAndConfirmTransaction('set farm rewards configuration', connection, transaction, payer, admin);
};

export const commitNewSlope = async (
  connection: Connection,
  payer: Keypair,
  config: PublicKey,
  tokenSwap: PublicKey,
  slope: bigint,
  admin: Keypair
) => {
  const transaction = new Transaction().add(
    createSetNewSlopeInstruction(config, tokenSwap, admin.publicKey, slope, SWAP_PROGRAM_ID)
  );

  await sendAndConfirmTransaction('set new slope', connection, transaction, payer, admin);
};
