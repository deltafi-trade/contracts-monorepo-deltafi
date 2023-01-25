import type { Connection, TransactionInstruction } from '@solana/web3.js';
import { Keypair, PublicKey, SystemProgram, Transaction } from '@solana/web3.js';
import { AccountLayout, MintLayout, Token, TOKEN_PROGRAM_ID } from '@solana/spl-token';

import { getMinBalanceRentForExempt, sendAndConfirmTransaction } from '../util';
import { SwapInfoLayout } from '../state';
import {
  InitializeData,
  createInitSwapInstruction,
  SwapData,
  DepositData,
  WithdrawData,
  createSwapInstruction,
  createDepositInstruction,
  createWithdrawInstruction,
} from '../instructions';
import { SWAP_PROGRAM_ID, DECIMALS } from '../constants';

export const initializeSwap = async (
  connection: Connection,
  payer: Keypair,
  owner: Keypair,
  market: PublicKey,
  tokenA: PublicKey,
  tokenB: PublicKey,
  adminFeeKeyA: PublicKey,
  adminFeeKeyB: PublicKey,
  pythProductA: PublicKey,
  pythA: PublicKey,
  pythProductB: PublicKey,
  pythB: PublicKey,
  adminPublicKey: PublicKey,
  serumMarket: PublicKey,
  serumBids: PublicKey,
  serumAsks: PublicKey,
  initData: InitializeData
) => {
  const swapAccount = Keypair.generate();
  const poolMint = Keypair.generate();
  const poolToken = Keypair.generate();

  const [authority] = await PublicKey.findProgramAddress([swapAccount.publicKey.toBuffer()], SWAP_PROGRAM_ID);

  const balanceTokenAccount = await getMinBalanceRentForExempt(connection, AccountLayout.span);
  const balanceForMint = await getMinBalanceRentForExempt(connection, MintLayout.span);
  const balanceForSwapInfo = await getMinBalanceRentForExempt(connection, SwapInfoLayout.span);

  const transaction = new Transaction()
    .add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: poolMint.publicKey,
        lamports: balanceForMint,
        space: MintLayout.span,
        programId: TOKEN_PROGRAM_ID,
      })
    )
    .add(Token.createInitMintInstruction(TOKEN_PROGRAM_ID, poolMint.publicKey, DECIMALS, authority, null))
    .add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: poolToken.publicKey,
        lamports: balanceTokenAccount,
        space: AccountLayout.span,
        programId: TOKEN_PROGRAM_ID,
      })
    )
    .add(Token.createInitAccountInstruction(TOKEN_PROGRAM_ID, poolMint.publicKey, poolToken.publicKey, owner.publicKey))
    .add(Token.createSetAuthorityInstruction(TOKEN_PROGRAM_ID, tokenA, authority, 'AccountOwner', owner.publicKey, []))
    .add(Token.createSetAuthorityInstruction(TOKEN_PROGRAM_ID, tokenB, authority, 'AccountOwner', owner.publicKey, []))
    .add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: swapAccount.publicKey,
        lamports: balanceForSwapInfo,
        space: SwapInfoLayout.span,
        programId: SWAP_PROGRAM_ID,
      })
    )
    .add(
      createInitSwapInstruction(
        market,
        swapAccount.publicKey,
        authority,
        adminFeeKeyA,
        adminFeeKeyB,
        tokenA,
        tokenB,
        poolMint.publicKey,
        poolToken.publicKey,
        pythProductA,
        pythA,
        pythProductB,
        pythB,
        adminPublicKey,
        serumMarket,
        serumBids,
        serumAsks,
        initData,
        SWAP_PROGRAM_ID
      )
    );

  await sendAndConfirmTransaction(
    'create and initialize SwapInfo account',
    connection,
    transaction,
    payer,
    swapAccount,
    poolMint,
    poolToken,
    owner
  );

  return {
    tokenSwap: swapAccount.publicKey,
    poolMint: poolMint.publicKey,
    poolToken: poolToken.publicKey,
  };
};

export const swap = async (
  connection: Connection,
  payer: Keypair,
  owner: Keypair,
  market: PublicKey,
  tokenSwap: PublicKey,
  userTransferAuthority: Keypair,
  source: PublicKey,
  swapSource: PublicKey,
  swapDestination: PublicKey,
  destination: PublicKey,
  rewardToken: PublicKey,
  rewardMint: PublicKey,
  adminFeeDestination: PublicKey,
  pythA: PublicKey,
  pythB: PublicKey,
  swapData: SwapData,
  approveInstructions: TransactionInstruction[]
) => {
  const [marketAuthority] = await PublicKey.findProgramAddress([market.toBuffer()], SWAP_PROGRAM_ID);
  const [swapAuthority] = await PublicKey.findProgramAddress([tokenSwap.toBuffer()], SWAP_PROGRAM_ID);

  const transaction = new Transaction();
  approveInstructions.forEach((tx) => transaction.add(tx));
  transaction.add(
    createSwapInstruction(
      market,
      tokenSwap,
      marketAuthority,
      swapAuthority,
      userTransferAuthority.publicKey,
      source,
      swapSource,
      swapDestination,
      destination,
      rewardToken,
      rewardMint,
      adminFeeDestination,
      pythA,
      pythB,
      swapData,
      SWAP_PROGRAM_ID
    )
  );

  await sendAndConfirmTransaction('swap', connection, transaction, payer, owner, userTransferAuthority);
};

export const deposit = async (
  connection: Connection,
  payer: Keypair,
  owner: Keypair,
  tokenSwap: PublicKey,
  userTransferAuthority: Keypair,
  depositTokenA: PublicKey,
  depositTokenB: PublicKey,
  swapTokenA: PublicKey,
  swapTokenB: PublicKey,
  poolMint: PublicKey,
  destination: PublicKey,
  pythA: PublicKey,
  pythB: PublicKey,
  despositData: DepositData,
  approveInstructions: TransactionInstruction[]
) => {
  const [swapAuthority] = await PublicKey.findProgramAddress([tokenSwap.toBuffer()], SWAP_PROGRAM_ID);

  const transaction = new Transaction();
  approveInstructions.forEach((tx) => transaction.add(tx));
  transaction.add(
    createDepositInstruction(
      tokenSwap,
      swapAuthority,
      userTransferAuthority.publicKey,
      depositTokenA,
      depositTokenB,
      swapTokenA,
      swapTokenB,
      poolMint,
      destination,
      pythA,
      pythB,
      despositData,
      SWAP_PROGRAM_ID
    )
  );

  await sendAndConfirmTransaction('deposit', connection, transaction, payer, owner, userTransferAuthority);
};

export const withdraw = async (
  connection: Connection,
  payer: Keypair,
  owner: Keypair,
  tokenSwap: PublicKey,
  userTransferAuthority: Keypair,
  source: PublicKey,
  swapTokenA: PublicKey,
  swapTokenB: PublicKey,
  destinationTokenA: PublicKey,
  destinationTokenB: PublicKey,
  poolMint: PublicKey,
  adminFeeA: PublicKey,
  adminFeeB: PublicKey,
  pythA: PublicKey,
  pythB: PublicKey,
  witdrawData: WithdrawData,
  approveInstructions: TransactionInstruction[]
) => {
  const [swapAuthority] = await PublicKey.findProgramAddress([tokenSwap.toBuffer()], SWAP_PROGRAM_ID);

  const transaction = new Transaction();
  approveInstructions.forEach((tx) => transaction.add(tx));
  transaction.add(
    createWithdrawInstruction(
      tokenSwap,
      swapAuthority,
      userTransferAuthority.publicKey,
      source,
      swapTokenA,
      swapTokenB,
      destinationTokenA,
      destinationTokenB,
      poolMint,
      adminFeeA,
      adminFeeB,
      pythA,
      pythB,
      witdrawData,
      SWAP_PROGRAM_ID
    )
  );

  await sendAndConfirmTransaction('withdraw', connection, transaction, payer, owner, userTransferAuthority);
};
