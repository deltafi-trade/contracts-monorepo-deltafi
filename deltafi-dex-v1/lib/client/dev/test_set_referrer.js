const fs = require('fs');
const os = require('os');
const web3 = require('@solana/web3.js');
const deltafi = require('../lib/index.cjs');
const token = require('@solana/spl-token');
const { UserReferrerDataLayout } = require('../lib/index.cjs.js');

async function run(deploymentName) {
    const deploymentConfigDir = __dirname + "/../admin/deployment/" + deploymentName;
    const deploymentConfig = JSON.parse(fs.readFileSync(
        deploymentConfigDir + "/config.json"));
    const secretDir = os.homedir() + "/.deltafi/keys/dex-v1";
    const payerSecret = JSON.parse(fs.readFileSync(
        secretDir + "/" + deploymentConfig.payerKeyName + ".json"));

    const codeBaseSharedPubkeysPath = deploymentConfigDir + "/output/shared_pubkeys.json";
    const sharedPubkeys = JSON.parse(fs.readFileSync(codeBaseSharedPubkeysPath));
    const configPubkey = new web3.PublicKey(sharedPubkeys["config"]);
    const mintPubkey = new web3.PublicKey(deploymentConfig["deltafiMint"]);

    const network = deploymentConfig.network;
    const swapProgramId = new web3.PublicKey(deploymentConfig.swapProgramId);
    const connection = new web3.Connection(deltafi.getClusterApiUrl(network), 'confirmed');
    const payer = web3.Keypair.fromSecretKey(Uint8Array.from(payerSecret));

    const userOwner = web3.Keypair.generate();
    const referrerToken = web3.Keypair.generate();

    const balanceForUserReferrerData = await connection.getMinimumBalanceForRentExemption(
        deltafi.USER_REFERRER_DATA_SIZE);

    const seed = "referrer";
    const userReferrerDataPubkey = await web3.PublicKey.createWithSeed(
        userOwner.publicKey,
        seed,
        swapProgramId,
    );
    const balanceTokenAccount = await connection.getMinimumBalanceForRentExemption(token.AccountLayout.span);
    const transaction = new web3.Transaction()
        .add(
            web3.SystemProgram.createAccount({
                fromPubkey: payer.publicKey,
                newAccountPubkey: referrerToken.publicKey,
                lamports: balanceTokenAccount * 2,
                space: token.AccountLayout.span,
                programId: token.TOKEN_PROGRAM_ID,
            })
        )
        .add(
            token.Token.createInitAccountInstruction(
                token.TOKEN_PROGRAM_ID,
                mintPubkey,
                referrerToken.publicKey,
                payer.publicKey
            )
        )
        .add(
            web3.SystemProgram.createAccountWithSeed({
                basePubkey: userOwner.publicKey,
                fromPubkey: payer.publicKey,
                newAccountPubkey: userReferrerDataPubkey,
                lamports: balanceForUserReferrerData * 2,
                space: deltafi.USER_REFERRER_DATA_SIZE,
                programId: swapProgramId,
                seed
            })
        )
        .add(
            deltafi.createSetReferrerInstruction(
                configPubkey,
                userOwner.publicKey,
                userReferrerDataPubkey,
                referrerToken.publicKey,
                swapProgramId,
            )
        );

    await web3.sendAndConfirmTransaction(
        connection, transaction, [payer, userOwner, referrerToken], {maxRetries: 5}
    );

    console.log("Parameters:");
    console.log("config key: " + configPubkey.toBase58());
    console.log("owner key: " + userOwner.publicKey.toBase58());
    console.log("referrer key: " + referrerToken.publicKey.toBase58());
    const userReferrerDataInfo = await connection.getAccountInfo(
        userReferrerDataPubkey);
    const userReferrerData = UserReferrerDataLayout.decode(userReferrerDataInfo.data);

    console.log("Got result");
    console.log("config key: " + userReferrerData.configKey.toBase58());
    console.log("owner key: " + userReferrerData.owner.toBase58());
    console.log("referrer key: " + userReferrerData.referrer.toBase58());
}

const deploymentName = process.argv[2];
console.log("Using deploymentName: " + deploymentName);
run(deploymentName)
    .then(() => console.info('Success!'))
    .catch((err) => {
        console.error(err);
        process.exit(1);
    });
