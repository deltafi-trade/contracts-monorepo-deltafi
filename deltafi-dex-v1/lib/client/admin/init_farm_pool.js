const fs = require('fs');
const web3 = require('@solana/web3.js');
const token = require('@solana/spl-token');
const deltafi = require('../lib/index.cjs.js');

module.exports = async (deployContext, params) => {
    const {connection, payer} = deployContext;

    const farmPool = web3.Keypair.generate();
    const reserveToken = web3.Keypair.generate();
    const config = new web3.PublicKey(params.config);
    const swap = new web3.PublicKey(params.swap);
    const poolMint = new web3.PublicKey(params.poolMint);

    const [authority, bumpSeed] = await web3.PublicKey.findProgramAddress(
        [farmPool.publicKey.toBuffer()],
        deployContext.swapProgramId,
    );

    const balanceTokenAcc = await connection.getMinimumBalanceForRentExemption(token.AccountLayout.span);
    const balanceFarmPool = await connection.getMinimumBalanceForRentExemption(deltafi.FARM_INFO_SIZE);

    let transaction = new web3.Transaction()
        .add(
            web3.SystemProgram.createAccount({
                fromPubkey: payer.publicKey,
                newAccountPubkey: reserveToken.publicKey,
                lamports: balanceTokenAcc * 2,
                space: token.AccountLayout.span,
                programId: token.TOKEN_PROGRAM_ID,
            })
        )
        .add(token.Token.createInitAccountInstruction(
            token.TOKEN_PROGRAM_ID, poolMint, reserveToken.publicKey, authority));

    await web3.sendAndConfirmTransaction(
        connection, transaction, [payer, reserveToken], {maxRetries: 5}
    );
    console.log("Created reserved token account")

    const initData = {
        feeNumerator: BigInt(1),
        feeDenominator: BigInt(1000),
        rewardsNumerator: BigInt(1),
        rewardsDenominator: BigInt(500),
        bumpSeed,
    };

    transaction = new web3.Transaction()
        .add(
            web3.SystemProgram.createAccount({
                fromPubkey: payer.publicKey,
                newAccountPubkey: farmPool.publicKey,
                lamports: balanceFarmPool * 2,
                space: deltafi.FARM_INFO_SIZE,
                programId: deployContext.swapProgramId,
            })
        )
        .add(
            deltafi.createInitFarmInstruction(
                config,
                swap,
                farmPool.publicKey,
                authority,
                reserveToken.publicKey,
                deployContext.userOwner.publicKey,
                initData,
                deployContext.swapProgramId,
            )
        );

    await web3.sendAndConfirmTransaction(
        connection, transaction, [payer, farmPool, deployContext.userOwner], {maxRetries: 5}
    );

    const result = {};
    result["farm_pool_secret"] = Array.from(farmPool.secretKey);
    result["farm_pool_pubkey"] = farmPool.publicKey.toBase58();
    result["reserve_token_secret"] = Array.from(farmPool.secretKey);
    result["reserve_token_pubkey"] = reserveToken.publicKey.toBase58();

    return result;
}
