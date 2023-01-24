const fs = require('fs');
const os = require('os');
const web3 = require('@solana/web3.js');
const deltafi = require('../lib/index.cjs');
const { loadFarmUser } = require('../lib/index.cjs.js');

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

    const poolPubkeys = JSON.parse(fs.readFileSync(
        deploymentConfigDir + "/output/SOL-SRM/result_pubkeys.json"));
    const farmPoolPubkey = new web3.PublicKey(poolPubkeys["farm_pool_SOL-SRM"]);

    const network = deploymentConfig.network;
    const swapProgramId = new web3.PublicKey(deploymentConfig.swapProgramId);
    const connection = new web3.Connection(deltafi.getClusterApiUrl(network), 'confirmed');
    const payer = web3.Keypair.fromSecretKey(Uint8Array.from(payerSecret));

    const userOwner = web3.Keypair.generate();

    const seed = ("farmUser" + farmPoolPubkey.toBase58()).substring(0, 32);
    const farmUserPubkey = await web3.PublicKey.createWithSeed(
        userOwner.publicKey,
        seed,
        swapProgramId,
    );

    const farmUserAccountBalance = await connection.getMinimumBalanceForRentExemption(
        deltafi.FARM_USER_SIZE);
    const transaction = new web3.Transaction()
        .add(
            web3.SystemProgram.createAccountWithSeed({
                basePubkey: userOwner.publicKey,
                fromPubkey: payer.publicKey,
                newAccountPubkey: farmUserPubkey,
                lamports: farmUserAccountBalance,
                space: deltafi.FARM_USER_SIZE,
                programId: swapProgramId,
                seed
            })
        )
        .add(
            deltafi.createInitFarmUserInstruction(
                configPubkey,
                farmPoolPubkey,
                farmUserPubkey,
                userOwner.publicKey,
                swapProgramId,
            )
        );

    await web3.sendAndConfirmTransaction(
        connection, transaction, [payer, userOwner], {maxRetries: 5}
    );

    console.log("seed: " + seed);
    console.log("config key: " + configPubkey.toBase58());
    console.log("owner key: " + userOwner.publicKey.toBase58());
    console.log("farm user key: " + farmUserPubkey.toBase58());

    const farmUser = await loadFarmUser(connection, farmUserPubkey.toBase58(), swapProgramId);
    console.log(farmUser);
}

const deploymentName = process.argv[2];
console.log("Using deploymentName: " + deploymentName);
run(deploymentName)
    .then(() => console.info('Success!'))
    .catch((err) => {
        console.error(err);
        process.exit(1);
    });
