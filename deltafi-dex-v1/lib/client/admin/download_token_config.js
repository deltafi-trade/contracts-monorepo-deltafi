const fs = require('fs');
const https = require('https');
const path = require('path');

function readJsonFile(filePath) {
    if (!filePath || !fs.existsSync(filePath)) {
        throw Error("Invalid Json file path:", filePath);
    }
    return JSON.parse(fs.readFileSync(filePath));
}

function writeJsonFile(filePath, jsonObject) {
    const parentDir = path.dirname(filePath);
    if (!fs.existsSync(parentDir)) {
        fs.mkdirSync(parentDir, { recursive: true });
    }
    fs.writeFileSync(filePath, JSON.stringify(jsonObject, null, 2) + "\n", "utf-8");
}

async function main() {
    const solanaTokenListUrl = "https://raw.githubusercontent.com/solana-labs/token-list/main/src/tokens/solana.tokenlist.json";
    const tmpTokenListFile = "/tmp/token-list.json";
    https.get(solanaTokenListUrl,(res) => {
        const filePath = fs.createWriteStream(tmpTokenListFile);
        res.pipe(filePath);
        filePath.on('finish',() => {
            filePath.close();
            console.log('Download Completed');
        })
    })

    const officialTokenConfigs = readJsonFile(tmpTokenListFile).tokens;
    const inputTokenConfigs = readJsonFile('./token/mainnet-beta.input.json');
    const symbolToTokenConfig = {};
    for (const tokenConfig of officialTokenConfigs) {
        if (tokenConfig.chainId != 101) {
            continue;
        }

        // there are multiple tokens that has symbol UST, we have to add this condition
        // to get UST Wormhole
        if (tokenConfig.symbol === "UST" && tokenConfig.name != "UST (Wormhole)") {
            continue;
        }
        
        const inputTokenConfig = inputTokenConfigs.find(
            ({ symbol }) => symbol === tokenConfig.symbol);

        if (inputTokenConfig == null) {
            continue;
        }

        const { pythProductName } = inputTokenConfig;
        const { symbol, name, address, decimals, logoURI} = tokenConfig;

        symbolToTokenConfig[symbol] = {
            pythProductName,
            symbol,
            // we are using native solana instead of the wrapped one
            // force the SOL name to be "Solana"
            name: symbol === "SOL" ? "Solana" : name,
            mint: address,
            decimals,
            logoURI
        };
    }

    const foundTokenConfigs = inputTokenConfigs.map(({ symbol }) => symbolToTokenConfig[symbol]);
    writeJsonFile("./token/mainnet-beta.json", foundTokenConfigs);
}

try {
    main();
} catch (err) {
    console.info("Failed to download token config");
    console.error(err);
    process.exit(1);
}
