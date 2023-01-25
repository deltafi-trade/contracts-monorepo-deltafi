const web3 = require('@solana/web3.js');
const token = require('@solana/spl-token');
const deltafi = require('../lib/index.cjs');
const fs = require('fs');

module.exports = async function init_config(deployContext) {
    const {connection, payer, userOwner, cacheDir, pythProgramId} = deployContext;

    const tmpDataPath = cacheDir + "/tmp_init_config.json";

    const payerMintKeypair = payer;
    const ownerMintKeypair = userOwner;

    const configKeypair = web3.Keypair.generate();
    const [marketAuthority] = await web3.PublicKey.findProgramAddress(
        [configKeypair.publicKey.toBuffer()],
        deployContext.swapProgramId,
    );
    const balanceForConfig = await connection.getMinimumBalanceForRentExemption(deltafi.CONFIG_SIZE);

    const fees = {
        adminTradeFeeNumerator: BigInt(1),
        adminTradeFeeDenominator: BigInt(2),
        adminWithdrawFeeNumerator: BigInt(1),
        adminWithdrawFeeDenominator: BigInt(4),
        tradeFeeNumerator: BigInt(5),
        tradeFeeDenominator: BigInt(1000),
        withdrawFeeNumerator: BigInt(1),
        withdrawFeeDenominator: BigInt(100),
    };

    const rewards = {
        decimals: 9,
        tradeRewardNumerator: BigInt(1),
        tradeRewardDenominator: BigInt(1000),
        tradeRewardCap: BigInt(100000000),
    };

    const balanceForToken =
        await connection.getMinimumBalanceForRentExemption(token.AccountLayout.span);
    const deltafiTokenKeyPair = web3.Keypair.generate();
    const transaction = new web3.Transaction()
        .add(
            web3.SystemProgram.createAccount({
                fromPubkey: payer.publicKey,
                newAccountPubkey: deltafiTokenKeyPair.publicKey,
                lamports: balanceForToken * 2,
                space: token.AccountLayout.span,
                programId: token.TOKEN_PROGRAM_ID,
            })
        )
        .add(
            token.Token.createInitAccountInstruction(
                token.TOKEN_PROGRAM_ID,
                deployContext.deltafiMint,
                deltafiTokenKeyPair.publicKey,
                marketAuthority)
        )
        .add(
            web3.SystemProgram.createAccount({
                fromPubkey: payerMintKeypair.publicKey,
                newAccountPubkey: configKeypair.publicKey,
                lamports: balanceForConfig * 3,
                space: deltafi.CONFIG_SIZE,
                programId: deployContext.swapProgramId,
            })
        )
        .add(
            deltafi.createAdminInitializeInstruction(
                configKeypair.publicKey,
                marketAuthority,
                deployContext.deltafiMint,
                ownerMintKeypair.publicKey,
                pythProgramId,
                deltafiTokenKeyPair.publicKey,
                { fees, rewards },
                deployContext.swapProgramId,
            )
        );

    console.log("deltafiMint: " + deployContext.deltafiMint.toBase58());
    console.log("deltafiToken: " + deltafiTokenKeyPair.publicKey.toBase58());
    await web3.sendAndConfirmTransaction(
        connection,
        transaction,
        [
            payerMintKeypair,
            ownerMintKeypair,
            configKeypair,
            deltafiTokenKeyPair
        ],
        {maxRetries: 5}
    );

    const result = {};

    result["config_pubkey"] = configKeypair.publicKey.toBase58();
    result["config_secret"] = Array.from(configKeypair.secretKey);
    result["deltafiToken_pubkey"] = deltafiTokenKeyPair.publicKey.toBase58();

    fs.writeFileSync(tmpDataPath, JSON.stringify(result), "utf-8");

    console.log("init config", "finished");
    console.log(result);
    return result;
}
