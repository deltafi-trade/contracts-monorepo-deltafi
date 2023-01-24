const web3 = require('@solana/web3.js');
const fs = require('fs');

function readJsonFile(filePath) {
    if (!filePath || !fs.existsSync(filePath)) {
        throw Error("Invalid Json file path:", filePath);
    }
    return JSON.parse(fs.readFileSync(filePath));
}

async function generateFrontConfig(deploymentName) {
    const configPath = "./deployment/" + deploymentName + "/config.json";
    if (!fs.existsSync(configPath)) {
        return null;
    }
    const { network, swapProgramId, tokens, swapPools, stableSwapPools } = readJsonFile(configPath);

    const deploymentOutputDir = "./deployment/" + deploymentName + "/output";
    if (!fs.existsSync(deploymentOutputDir)) {
        return null;
    }

    const sharedPubkeyPath = deploymentOutputDir + "/shared_pubkeys.json"
    if (!fs.existsSync(sharedPubkeyPath)) {
        throw Error("not shared pubkey file");
    }

    const { config: marketConfigAddress, deltafiMint: deltafiTokenMint, deltafiToken } = readJsonFile(
        sharedPubkeyPath);

    const pythConfigs = JSON.parse(fs.readFileSync("./pyth/" + network + ".json"));
    const pythProductNameToConfig = {};
    for (const pythConfig of pythConfigs) {
        pythProductNameToConfig[pythConfig.productName] = pythConfig;
    }

    const [marketAuthority, bumpSeed] = await web3.PublicKey.findProgramAddress(
        [new web3.PublicKey(marketConfigAddress).toBuffer()],
        new web3.PublicKey(swapProgramId),
    );

    const frontendJson = {
        network,
        swapProgramId,
        marketConfigAddress,
        marketAuthority: marketAuthority.toBase58(),
        bumpSeed,
        deltafiTokenMint,
        deltafiToken,
    };

    if (network == "mainnet-beta") {
        const { serumProgramId } = readJsonFile("./serum/" + network + ".json");
        frontendJson["serumProgramId"] = serumProgramId;
    }

    const usedTokenSet = new Set();
    const poolInfo = [];
    for (const poolConfig of swapPools.concat(stableSwapPools)) {
        usedTokenSet.add(poolConfig.tokenA);
        usedTokenSet.add(poolConfig.tokenB);

        const subfolder = poolConfig.tokenA + "-" + poolConfig.tokenB;
        const subdeploymentOutputDir = deploymentOutputDir + "/" + subfolder;
        if (!fs.existsSync(subdeploymentOutputDir)) {
            continue;
        }

        const [baseToken, quoteToken] = subfolder.split("-");
        if (!baseToken || !quoteToken) {
            throw Error("wrong subfolder format");
        }

        const files = fs.readdirSync(subdeploymentOutputDir);
        if (files[0] !== "result_pubkeys.json") {
            throw Error("output folder has something other than the result");
        }
        const poolName = poolConfig["name"];

        const resultFilePath = subdeploymentOutputDir + "/" + files[0];
        const resultJson = JSON.parse(fs.readFileSync(resultFilePath));
        if (resultJson.config !== frontendJson.marketConfigAddress
            || resultJson.deltafiMint !== frontendJson.deltafiTokenMint
            || resultJson.network !== network) {
            throw Error("config files mismatch");
        }

        const poolInfoValue = {
            name: poolName,
            base: baseToken,
            quote: quoteToken,
            swap: resultJson["pool_" + subfolder + "_swap"],
            mint: resultJson["pool_" + subfolder + "_mint"],
            farm: resultJson["farm_pool_" + subfolder],
            token: resultJson["pool_" + subfolder + "_token"],
            decimals: resultJson["pool_" + subfolder + "_decimals"],
        };
        poolInfoValue["oraclePriority"] = poolConfig.oraclePriority;
        if (poolConfig.oraclePriority == "SERUM_ONLY") {
            poolInfoValue["serumMarket"] = resultJson["serumMarket"];
            poolInfoValue["serumBids"] = resultJson["serumBids"];
            poolInfoValue["serumAsks"] = resultJson["serumAsks"];
            poolInfo.push(poolInfoValue);
        } else {
            poolInfo.push(poolInfoValue);
        }
    }
    frontendJson["poolInfo"] = poolInfo;

    const tokenInfo = [];
    const tokenConfigs = JSON.parse(fs.readFileSync("./token/" + network + ".json"));
    for (const tokenConfig of tokenConfigs) {
        const { pythProductName, mint, symbol, decimals, name, logoURI } = tokenConfig;
        const pyth = pythProductNameToConfig[pythProductName];
        if (usedTokenSet.has(symbol)) {
            tokenInfo.push({ pyth, mint, symbol, decimals, name, logoURI });
        }
    }
    frontendJson["tokenInfo"] = tokenInfo;
    return frontendJson;
}

async function main() {
    const files = fs.readdirSync('./deployment');
    const fullFrontendConfig = {};
    for (const deploymentName of files) {
        const frontendConfig = await generateFrontConfig(deploymentName);
        if (frontendConfig != null) {
            fullFrontendConfig[deploymentName] = frontendConfig;
        }
    }
    console.info(JSON.stringify(fullFrontendConfig, null, 2));
}

try {
    main();
} catch (err) {
    console.info("Get frontend config failed");
    console.error(err);
    process.exit(1);
}
