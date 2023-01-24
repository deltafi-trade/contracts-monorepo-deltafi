const fs = require('fs');
const path = require('path');
const os = require('os');
const web3 = require('@solana/web3.js');
const token = require('@solana/spl-token');
const deltafi = require('../lib/index.cjs');
const { PythHttpClient } = require('@pythnetwork/client');
const { SwapInfoLayout, FarmInfoLayout, WAD } = require('../lib/index.cjs.js');
const { MintLayout } = require('../lib/index.cjs.js');
const { Market } = require('@project-serum/serum');
const { getSerumNameToAccountsAndPrice, validateSerumMarket } = require('./serum_market_utils');

const pythProgramIds = {
    "mainnet-beta": new web3.PublicKey("FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH"),
    "testnet": new web3.PublicKey("8tfDNiaEyrV6Q1U4DEXrEigs9DoDtkugzFbybENEbCDz"),
    "localhost": new web3.PublicKey("8tfDNiaEyrV6Q1U4DEXrEigs9DoDtkugzFbybENEbCDz"),
}

function isEqual(objectA, objectB) {
    const keys = Object.keys(objectA);
    for (const key of keys) {
        if (objectA[key] != objectB[key]) {
            return false;
        }
    }
    return true;
}

async function readPythPrice(network, pythProgramId) {
    // Disable pyth on localhost
    if (network == "localhost") {
        return {};
    }

    const pythClient = new PythHttpClient(
        new web3.Connection(deltafi.getClusterApiUrl(network), 'confirmed'),
        pythProgramId);
    const pythResult = await pythClient.getData();

    const pythProductToPrice = {}
    pythResult.productPrice.forEach((value, key, _) => {
        pythProductToPrice[key] = value.price;
    });
    return pythProductToPrice;
}

async function checkAndUpdateConfig(deployContext, swapPool, poolPubkeys, deploymentConfig) {
    const poolInfoA = deployContext.tokenInfo[swapPool.tokenA]
    const poolInfoB = deployContext.tokenInfo[swapPool.tokenB]
    const poolName = poolInfoA.symbol + "-" + poolInfoB.symbol;
    const swapPubkey = new web3.PublicKey(poolPubkeys["pool_" + poolName + "_swap"]);
    const swapAccountInfo = await deployContext.connection.getAccountInfo(swapPubkey);
    const swapInfo = SwapInfoLayout.decode(swapAccountInfo.data);

    const farmPoolPubkey = new web3.PublicKey(poolPubkeys["farm_pool_" + poolName]);
    const farmPoolAccountInfo = await deployContext.connection.getAccountInfo(farmPoolPubkey);
    const farmInfo = FarmInfoLayout.decode(farmPoolAccountInfo.data);

    await updateRewards(deployContext, swapPool, poolPubkeys, swapInfo);
    await updateFarmRewards(deployContext, swapPool, poolPubkeys, farmInfo);
    await updateFees(deployContext, swapPool, poolPubkeys, deploymentConfig.fees, swapInfo);
    await updateSlope(deployContext, swapPool, poolPubkeys, swapInfo);
    await checkAndSetDecimals(deployContext, swapPool, poolPubkeys, swapInfo);
    await updateSwapLimit(deployContext, swapPool, poolPubkeys, swapInfo);
}

async function updateSwapLimit(deployContext, swapPool, poolPubkeys, swapInfo) {
    const poolInfoA = deployContext.tokenInfo[swapPool.tokenA]
    const poolInfoB = deployContext.tokenInfo[swapPool.tokenB]
    const poolName = poolInfoA.symbol + "-" + poolInfoB.symbol;

    const configPubkey = new web3.PublicKey(poolPubkeys["config"]);
    const swapPubkey = new web3.PublicKey(poolPubkeys["pool_" + poolName + "_swap"]);

    const swapOutLimitPercentage = swapPool.swapOutLimitPercentage;

    if (swapInfo.swapOutLimitPercentage === swapOutLimitPercentage) {
        console.log("no change to the swap limit of the pool", poolName);
        return
    }

    const transaction = new web3.Transaction()
    .add(
        deltafi.createSetSwapLimitInstruction(
            configPubkey,
            swapPubkey,
            deployContext.userOwner.publicKey,
            swapOutLimitPercentage,
            deployContext.swapProgramId,
        )
    );

    await web3.sendAndConfirmTransaction(
        deployContext.connection,
        transaction,
        [
            deployContext.userOwner,
        ],
        {maxRetries: 5}
    );

    console.log("New swap limit is set for the pool", poolName);
}

async function checkAndSetDecimals(deployContext, swapPool, poolPubkeys, swapInfo) {
    const poolInfoA = deployContext.tokenInfo[swapPool.tokenA]
    const poolInfoB = deployContext.tokenInfo[swapPool.tokenB]
    const poolName = poolInfoA.symbol + "-" + poolInfoB.symbol;

    const configPubkey = new web3.PublicKey(poolPubkeys["config"]);
    const swapPubkey = new web3.PublicKey(poolPubkeys["pool_" + poolName + "_swap"]);

    const tokenADecimals = deployContext.tokenInfo[swapPool.tokenA].decimals;
    const tokenBDecimals = deployContext.tokenInfo[swapPool.tokenB].decimals;

    // set decimals only if the decimals info is unset
    if (swapInfo.tokenADecimals === 0 && swapInfo.tokenBDecimals === 0) {
        const transaction = new web3.Transaction()
        .add(
            deltafi.createSetDecimalsInstruction(
                configPubkey,
                swapPubkey,
                deployContext.userOwner.publicKey,
                tokenADecimals,
                tokenBDecimals,
                deployContext.swapProgramId,
            )
        );

        await web3.sendAndConfirmTransaction(
            deployContext.connection,
            transaction,
            [
                deployContext.userOwner,
            ],
            {maxRetries: 5}
        );

        console.log("Setting decimals for", swapPubkey.toBase58(), "is successful");
        return;
    }

    // decimals info in the account should be set again
    // if the info in the accout is different from the config, there is an error
    if(tokenADecimals !== swapInfo.tokenADecimals || tokenBDecimals !== swapInfo.tokenBDecimals) {
        throw Error("Decimals info in", swapPubkey.toBase58(), "is set to different values from the config");
    }

    console.log("Decimals have already set");
}

async function updateRewards(deployContext, swapPool, poolPubkeys, swapInfo) {
    const currentRewards = swapInfo.rewards;

    const poolInfoA = deployContext.tokenInfo[swapPool.tokenA]
    const poolInfoB = deployContext.tokenInfo[swapPool.tokenB]
    const poolName = poolInfoA.symbol + "-" + poolInfoB.symbol;

    const configPubkey = new web3.PublicKey(poolPubkeys["config"]);
    const swapPubkey = new web3.PublicKey(poolPubkeys["pool_" + poolName + "_swap"]);
    const rewards = poolInfoA.rewards;

    rewards.tradeRewardNumerator = BigInt(rewards.tradeRewardNumerator);
    rewards.tradeRewardDenominator = BigInt(rewards.tradeRewardDenominator);
    rewards.tradeRewardCap = BigInt(rewards.tradeRewardCap);

    if (isEqual(rewards, currentRewards)) {
        console.log("no change to the reward data of pool", poolName);
        return;
    }

    const transaction = new web3.Transaction()
        .add(
            deltafi.createSetNewRewardsInstruction(
                configPubkey,
                swapPubkey,
                deployContext.userOwner.publicKey,
                rewards,
                deployContext.swapProgramId,
            )
        );

    await web3.sendAndConfirmTransaction(
        deployContext.connection,
        transaction,
        [
            deployContext.userOwner,
        ],
        {maxRetries: 5}
    );

    console.log("Updated rewards config for " + poolName);
}

async function updateFarmRewards(deployContext, swapPool, poolPubkeys, farmInfo) {
    const currentFarmPool = farmInfo;

    const poolInfoA = deployContext.tokenInfo[swapPool.tokenA]
    const poolInfoB = deployContext.tokenInfo[swapPool.tokenB]
    const poolName = poolInfoA.symbol + "-" + poolInfoB.symbol;

    const configPubkey = new web3.PublicKey(poolPubkeys["config"]);
    const farmPoolPubkey = new web3.PublicKey(poolPubkeys["farm_pool_" + poolName]);
    const farmReward = poolInfoA.farmRewards;

    if (currentFarmPool.rewardsNumerator === BigInt(farmReward.aprNumerator) &&
        currentFarmPool.rewardsDenominator === BigInt(farmReward.aprDenominator)
    ) {
        console.log("no change to the apr data of farm pool", poolName);
        return;
    }

    farmReward.aprNumerator = BigInt(farmReward.aprNumerator);
    farmReward.aprDenominator = BigInt(farmReward.aprDenominator);

    const transaction = new web3.Transaction()
        .add(
            deltafi.createSetFarmRewardsInstruction(
                configPubkey,
                farmPoolPubkey,
                deployContext.userOwner.publicKey,
                farmReward,
                deployContext.swapProgramId,
            )
        );

    await web3.sendAndConfirmTransaction(
        deployContext.connection,
        transaction,
        [
            deployContext.userOwner,
        ],
        {maxRetries: 3}
    );

    console.log("Updated farm rewards config for " + poolName);
}

async function updateFees(deployContext, swapPool, poolPubkeys, fees, swapInfo) {
    const curretFees = swapInfo.fees;

    const poolInfoA = deployContext.tokenInfo[swapPool.tokenA]
    const poolInfoB = deployContext.tokenInfo[swapPool.tokenB]
    const poolName = poolInfoA.symbol + "-" + poolInfoB.symbol;

    const configPubkey = new web3.PublicKey(poolPubkeys["config"]);
    const swapPubkey = new web3.PublicKey(poolPubkeys["pool_" + poolName + "_swap"]);

    fees.adminTradeFeeNumerator = BigInt(fees.adminTradeFeeNumerator);
    fees.adminTradeFeeDenominator = BigInt(fees.adminTradeFeeDenominator);
    fees.adminWithdrawFeeNumerator = BigInt(fees.adminWithdrawFeeNumerator);
    fees.adminWithdrawFeeDenominator = BigInt(fees.adminWithdrawFeeDenominator);
    fees.tradeFeeNumerator = BigInt(fees.tradeFeeNumerator);
    fees.tradeFeeDenominator = BigInt(fees.tradeFeeDenominator);
    fees.withdrawFeeNumerator = BigInt(fees.withdrawFeeNumerator);
    fees.withdrawFeeDenominator = BigInt(fees.withdrawFeeDenominator);

    if (isEqual(fees, curretFees)) {
        console.log("no change to the fees of pool", poolName);
        return;
    }

    const transaction = new web3.Transaction()
        .add(
            deltafi.createSetNewFeesInstruction(
                configPubkey,
                swapPubkey,
                deployContext.userOwner.publicKey,
                fees,
                deployContext.swapProgramId,
            )
        );

    await web3.sendAndConfirmTransaction(
        deployContext.connection,
        transaction,
        [
            deployContext.userOwner,
        ],
        {maxRetries: 5}
    );

    console.log("Updated fees config for " + poolName);
}

async function updateSlope(deployContext, swapPool, poolPubkeys, swapInfo) {
    const currentSlope = BigInt(swapInfo.poolState.slope * WAD);

    const poolInfoA = deployContext.tokenInfo[swapPool.tokenA]
    const poolInfoB = deployContext.tokenInfo[swapPool.tokenB]
    const poolName = poolInfoA.symbol + "-" + poolInfoB.symbol;

    const configPubkey = new web3.PublicKey(poolPubkeys["config"]);
    const swapPubkey = new web3.PublicKey(poolPubkeys["pool_" + poolName + "_swap"]);
    const newSlope = BigInt(swapPool.slope);

    if (currentSlope === newSlope) {
        console.log("no change to the slope of pool", poolName);
        return;
    }

    const transaction = new web3.Transaction()
        .add(
            deltafi.createSetNewSlopeInstruction(
                configPubkey,
                swapPubkey,
                deployContext.userOwner.publicKey,
                newSlope,
                deployContext.swapProgramId,
            )
        );

    await web3.sendAndConfirmTransaction(
        deployContext.connection,
        transaction,
        [
            deployContext.userOwner,
        ],
        {maxRetries: 5}
    );

    console.log("Updated slope for " + poolName);
}

function getUsdPrice(deployContext, tokenInfo) {
    if ('fixedUsdPrice' in tokenInfo.pyth) {
        return tokenInfo.pyth.fixedUsdPrice;
    }
    return deployContext.pythProductToPrice[tokenInfo.pyth.productName];
}

function writeJsonFile(filePath, jsonObject) {
    const parentDir = path.dirname(filePath);
    if (!fs.existsSync(parentDir)) {
        fs.mkdirSync(parentDir, { recursive: true });
    }
    fs.writeFileSync(filePath, JSON.stringify(jsonObject, null, 2) + "\n", "utf-8");
}

function shouldReset(deploymentName, isReset) {
    return !deploymentName.startsWith("mainnet") && isReset;
}

async function getOrCreateAdminFeeAccount(deployContext, tokenInfo) {
    const tokenObject = new token.Token(
        deployContext.connection,
        new web3.PublicKey(tokenInfo.mint),
        token.TOKEN_PROGRAM_ID,
        deployContext.payer);
    const tokenAccount = await tokenObject.getOrCreateAssociatedAccountInfo(
        deployContext.userOwner.publicKey);
    return tokenAccount.address.toBase58();
}

async function run(deploymentName, isReset) {
    const cacheDir = os.homedir() + "/tmp/deployment/" + deploymentName;
    if (!fs.existsSync(cacheDir)) {
        fs.mkdirSync(cacheDir)
    }

    if (isReset) {
        const files = fs.readdirSync(cacheDir, err => {
            if (err) throw err;
        });

        for (const file of files) {
            const fullPath = path.join(cacheDir, file);
            if (!fs.lstatSync(fullPath).isFile()) {
                continue;
            }
            fs.unlinkSync(fullPath, err => {
                if (err) throw err;
            });
        }
    }

    const validateConfig = require('./init_validate_config');

    const deployConfigPath = "./deployment/" + deploymentName + "/config.json";
    const combinedDeployConfigPath = "./deployment/" + deploymentName + "/combined-config.json";
    const deploymentConfig = JSON.parse(fs.readFileSync(deployConfigPath));
    await validateConfig(deploymentConfig);

    const network = deploymentConfig.network;
    const pythConfigs = JSON.parse(fs.readFileSync("./pyth/" + network + ".json"));
    const pythProductNameToConfig = {};
    for (const pythConfig of pythConfigs) {
        pythProductNameToConfig[pythConfig.productName] = pythConfig;
    }

    const usedTokenSet = new Set();
    for (const poolConfig of deploymentConfig.swapPools.concat(deploymentConfig.stableSwapPools)) {
        usedTokenSet.add(poolConfig.tokenA);
        usedTokenSet.add(poolConfig.tokenB);
    }

    const connection = new web3.Connection(deltafi.getClusterApiUrl(network), 'confirmed');
    const tokenConfigs = JSON.parse(fs.readFileSync("./token/" + network + ".json"));
    const rewardConfigs = JSON.parse(fs.readFileSync("./token/rewards/" + network + ".json"));
    const tokenInfo = {};
    for (const symbol of usedTokenSet) {
        const tokenConfig = tokenConfigs.find((config) => symbol === config.symbol);
        if (tokenConfig == null) {
            throw Error("Cannot find token config for " + tokenConfig.symbol);
        }

        const rewardConfig = rewardConfigs.find((config) => symbol === config.symbol);
        const { rewards, farmRewards } = rewardConfig;
        const { mint, decimals, pythProductName } = tokenConfig;

        const info = {
            mint,
            decimals,
            rewards,
            farmRewards,
            symbol,
            pyth: pythProductNameToConfig[pythProductName],
        }
        if (info.decimals != info.rewards.decimals) {
            throw Error("Reward decimals doesn't make token decimals");
        }

        const mintAccountInfo = await connection.getAccountInfo(new web3.PublicKey(info.mint));
        const mintInfo =  MintLayout.decode(mintAccountInfo.data);
        if (mintInfo.decimals != decimals) {
            throw Error("token decimals doesn't match config: " + symbol);
        }

        tokenInfo[symbol] = info;
    }
    deploymentConfig["tokens"] = tokenInfo;

    // Write the combined deploy config.
    writeJsonFile(combinedDeployConfigPath, deploymentConfig);

    const swapPools = deploymentConfig.swapPools;
    const stableSwapPools = deploymentConfig.stableSwapPools;

    const existingUniquePairs = {};
    const validatePool = (pool) => {
        const a = pool.tokenA;
        const b = pool.tokenB;
        if (!a || !b || a === b || !tokenInfo[a] || !tokenInfo[b]) {
            throw Error("invalid pool name:", pool);
        }

        const uniquePair = (a < b) ? a + "-" + b : b + "-" + a;
        if (existingUniquePairs[uniquePair]) {
            throw Error("token pair already exists: " + uniquePair);
        }
        existingUniquePairs[uniquePair] = true;
    }
    deploymentConfig.swapPools.forEach(pool => validatePool(pool));
    deploymentConfig.stableSwapPools.forEach(pool => validatePool(pool));

    // get serum configuration
    const serumNameToAccountsAndPrice = await getSerumNameToAccountsAndPrice(network, connection);
    console.info(serumNameToAccountsAndPrice);

    const secretDir = os.homedir() + "/.deltafi/keys/dex-v1";
    const adminSecret = JSON.parse(fs.readFileSync(secretDir + "/" + deploymentConfig.adminKeyName + ".json"));
    const payerSecret = JSON.parse(fs.readFileSync(secretDir + "/" + deploymentConfig.payerKeyName + ".json"));
    const swapProgramId = deploymentConfig.swapProgramId;
    const pythProgramId = pythProgramIds[network];
    const deployContext = {
        network,
        connection,
        payer: web3.Keypair.fromSecretKey(Uint8Array.from(payerSecret)),
        userOwner: web3.Keypair.fromSecretKey(Uint8Array.from(adminSecret)),
        cacheDir,
        swapProgramId: new web3.PublicKey(swapProgramId),
        deltafiMint:  new web3.PublicKey(deploymentConfig.deltafiMint),
        pythProgramId,
        pythProductToPrice: await readPythPrice(network, pythProgramId),
        tokenInfo,
    };

    const initConfig = require("./init_config");
    const {initPoolStep1, initPoolStep2, initPoolStep3} = require("./init_pool");
    const initFarmPool = require("./init_farm_pool");

    console.log("Running initConfig");
    const codeBaseSharedPubkeysPath = "./deployment/" + deploymentName + "/output/shared_pubkeys.json";
    let sharedPubkeys = {};
    if (fs.existsSync(codeBaseSharedPubkeysPath) && !shouldReset(deploymentName, isReset)) {
        console.log("Config already exists, reading from " + codeBaseSharedPubkeysPath);
        sharedPubkeys = JSON.parse(fs.readFileSync(codeBaseSharedPubkeysPath));
    } else {
        const configResult = await initConfig(deployContext)
        sharedPubkeys = {
            config: configResult["config_pubkey"],
            deltafiMint: deploymentConfig.deltafiMint,
            deltafiToken: configResult["deltafiToken_pubkey"],
        }
        writeJsonFile(codeBaseSharedPubkeysPath, sharedPubkeys);
    }
    console.log("Finished initConfig");

    /// processing function for swap pool and stable swap pool
    const initializePool = async (swapPool, useStableSwap) => {
        const poolInfoA = tokenInfo[swapPool.tokenA]
        const poolInfoB = tokenInfo[swapPool.tokenB]
        const poolName = poolInfoA.symbol + "-" + poolInfoB.symbol;
        const oraclePriority = swapPool.oraclePriority;

        const outputDir = cacheDir + "/output/" + poolName;
        const secretsPath = outputDir + "/result_secrets.json";
        const pubkeysPath = outputDir + "/result_pubkeys.json";

        const codeBasePubkeysDir = "./deployment/" + deploymentName + "/output/" + poolName;
        const codeBasePubkeysPath = codeBasePubkeysDir +  "/result_pubkeys.json";
        if (fs.existsSync(codeBasePubkeysPath) && !shouldReset(deploymentName, isReset)) {
            console.log("Pool " + poolName + " has been created already. skipping.");
            return JSON.parse(fs.readFileSync(codeBasePubkeysPath));
        }

        if (oraclePriority == "SERUM_ONLY") {
            await validateSerumMarket(
                poolName,
                serumNameToAccountsAndPrice[poolName].marketAddress,
                poolInfoA.mint,
                poolInfoB.mint,
                connection,
                network,
            );
        }

        const adminFeeA = await getOrCreateAdminFeeAccount(deployContext, poolInfoA);
        const adminFeeB = await getOrCreateAdminFeeAccount(deployContext, poolInfoB);
        console.info("admin fee A: ", adminFeeA);
        console.info("admin fee B: ", adminFeeB);

        const secrets = {
            network: network,
        }

        const pubkeys = {
            ...sharedPubkeys,
            network: network,
            adminFeeA,
            adminFeeB,
            oraclePriority,
        }

        console.log("Running initPoolStep1 " + poolName);
        const step1Result = await initPoolStep1(deployContext, poolInfoA, poolInfoB)
        .then(res => {
            secrets["pool_" + poolName + "_" + poolInfoA.symbol] = res["poolA_secret"];
            secrets["pool_" + poolName + "_" + poolInfoB.symbol] = res["poolB_secret"];

            pubkeys["pool_" + poolName + "_" + poolInfoA.symbol] = res["poolA_pubkey"];
            pubkeys["pool_" + poolName + "_" + poolInfoB.symbol] = res["poolB_pubkey"];

            return {
                poolA_pubkey: res["poolA_pubkey"],
                poolB_pubkey: res["poolB_pubkey"],
                symbolA: poolInfoA.symbol,
                symbolB: poolInfoB.symbol,
                poolName: poolName,
                poolMintDecimals: poolInfoA.decimals,
                oraclePriority: oraclePriority,
            };
        });

        const step2Result = await initPoolStep2(deployContext, step1Result)
        .then(res => {
            secrets["pool_" + poolName + "_swap"] = res["swap_secret"];
            secrets["pool_" + poolName + "_mint"] = res["mint_secret"];
            secrets["pool_" + poolName + "_token"] = res["token_secret"];
            secrets["pool_" + poolName + "_nonce"] = res["nonce"];

            pubkeys["pool_" + poolName + "_authority"] = res["authority"];
            pubkeys["pool_" + poolName + "_swap"] = res["swap_pubkey"];
            pubkeys["pool_" + poolName + "_mint"] = res["mint_pubkey"];
            pubkeys["pool_" + poolName + "_token"] = res["token_pubkey"];
            pubkeys["pool_" + poolName + "_decimals"] = poolInfoA.decimals;

            let result = {
                swap_secret: res["swap_secret"],
                swap_pubkey: res["swap_pubkey"],
                authority: res["authority"],
                config_pubkey: sharedPubkeys["config"],
                adminFeeA,
                adminFeeB,
                poolA_pubkey: res["poolA_pubkey"],
                poolB_pubkey: res["poolB_pubkey"],
                nonce: res["nonce"],
                poolMint_pubkey: res["mint_pubkey"],
                poolToken_pubkey: res["token_pubkey"],
                tokenAMint: poolInfoA.mint,
                tokenBMint: poolInfoB.mint,
                poolName: poolName,
                useStableSwap,
                tokenADecimals: poolInfoA.decimals,
                tokenBDecimals: poolInfoB.decimals,
            };

            if (useStableSwap || (!useStableSwap && oraclePriority == "PYTH_ONLY")) {
                const pythUsdPriceA = getUsdPrice(deployContext, poolInfoA);
                const pythUsdPriceB = getUsdPrice(deployContext, poolInfoB);
                initTokenAmountA = Math.floor((10 ** poolInfoA.decimals) / 10 / pythUsdPriceA);
                initTokenAmountB = Math.floor((10 ** poolInfoB.decimals) / 10 / pythUsdPriceB);
                console.log("Pyth usd price A: " + pythUsdPriceA + " init token amount A: " + initTokenAmountA);
                console.log("Pyth usd price B: " + pythUsdPriceB + " init token amount B: " + initTokenAmountB);

                result["pythPriceA"] = poolInfoA.pyth.price;
                result["pythProductA"] = poolInfoA.pyth.product;
                result["pythPriceB"] = poolInfoB.pyth.price;
                result["pythProductB"] = poolInfoB.pyth.product;
                result["initTokenAmountA"] = initTokenAmountA;
                result["initTokenAmountB"] = initTokenAmountB;

                if (!useStableSwap) {
                    result["oraclePriority"] = oraclePriority;
                }
            } else if (!useStableSwap && oraclePriority == "SERUM_ONLY") {
                // For serum market, tokenB is usually stableCoin or SOL with pyth price avaiable
                const usdPriceB = getUsdPrice(deployContext, poolInfoB);
                if (usdPriceB == undefined) {
                    throw Error("No usd price for token : ", poolInfoB.symbol);
                }
                const usdPriceA = serumNameToAccountsAndPrice[poolName].marketPrice * usdPriceB;
                console.info(usdPriceB, serumNameToAccountsAndPrice[poolName].marketPrice, usdPriceA);
                initTokenAmountA = Math.floor((10 ** poolInfoA.decimals) / 10 / usdPriceA);
                initTokenAmountB = Math.floor((10 ** poolInfoB.decimals) / 10 / usdPriceB);
                console.log("usd price A: " + usdPriceA + " init token amount A: " + initTokenAmountA);
                console.log("usd price B: " + usdPriceB + " init token amount B: " + initTokenAmountB);

                result["serumMarketAddress"] = serumNameToAccountsAndPrice[poolName].marketAddress;
                result["serumBidsAddress"] = serumNameToAccountsAndPrice[poolName].bidsAddress;
                result["serumAsksAddress"] = serumNameToAccountsAndPrice[poolName].asksAddress;
                result["initTokenAmountA"] = initTokenAmountA;
                result["initTokenAmountB"] = initTokenAmountB;
                result["oraclePriority"] = oraclePriority;
                console.info(result);
                pubkeys["serumMarket"] = result["serumMarketAddress"];
                pubkeys["serumBids"] = result["serumBidsAddress"];
                pubkeys["serumAsks"] = result["serumAsksAddress"];
            } else {
                throw Error("invalid oraclePriority for pool :", poolName);
            }
            return result;
        });

        const step3Result = await initPoolStep3(deployContext, step2Result)
        .then(res => (
            {
                config: sharedPubkeys["config"],
                swap: res["swap_pubkey"],
                poolMint: res["mint_pubkey"],
                poolName: poolName
            }
        ));

        await initFarmPool(deployContext, step3Result)
        .then(res => {
            secrets["farm_pool_" + poolName] = res["farm_pool_secret"];
            secrets["farm_pool_" + poolName + "_reserve_token"] = res["reserve_token_secret"];
            pubkeys["farm_pool_" + poolName] = res["farm_pool_pubkey"];
            pubkeys["farm_pool_" + poolName + "_reserve_token"] = res["reserve_token_pubkey"];
        });

        const backupOutputDir = outputDir + new Date().getTime();
        const backupSecretsPath = backupOutputDir + "/result_secrets.json";
        const backupPubkeysPath = backupOutputDir + "/result_pubkeys.json";

        if (!fs.existsSync(backupOutputDir)) {
            fs.mkdirSync(backupOutputDir, { recursive: true });
        }
        if (fs.existsSync(secretsPath)) {
            fs.renameSync(secretsPath, backupSecretsPath);
        }
        if (fs.existsSync(pubkeysPath)) {
            fs.renameSync(pubkeysPath, backupPubkeysPath);
        }

        writeJsonFile(secretsPath, secrets);
        writeJsonFile(pubkeysPath, pubkeys);

        console.log("Output secret path: " + secretsPath);
        console.log("Output pubkey path: " + pubkeysPath);
        writeJsonFile(codeBasePubkeysPath, pubkeys);
        return pubkeys;
    }

    /// process list of swap pool
    for (const swapPool of swapPools) {
        const pubkeys = await initializePool(swapPool, useStableSwap=false);
        await checkAndUpdateConfig(deployContext, swapPool, pubkeys, deploymentConfig);
    }
    /// process list of stable swap pool
    for (const stableSwapPool of stableSwapPools) {
        const pubkeys = await initializePool(stableSwapPool, useStableSwap=true);
        await checkAndUpdateConfig(deployContext, stableSwapPool, pubkeys, deploymentConfig);
    }
}

const deploymentName = process.argv[2];
const isReset = process.argv.includes("--reset");
console.log("Using deploymentName: " + deploymentName + ", isReset: " + isReset);
run(deploymentName, isReset)
    .then(() => console.info('Success!'))
    .catch((err) => {
        console.error(err);
        process.exit(1);
    });
