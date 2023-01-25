const web3 = require('@solana/web3.js');
const fs = require('fs');
const { Market } = require('@project-serum/serum');

async function getSerumAccountsAndMarketPrice(marketAddress, connection, serumProgramId) {
    const marketPubkey = new web3.PublicKey(marketAddress);
    const market = await Market.load(connection, marketPubkey, {}, serumProgramId);
    // Fetching orderbooks
    const bids = await market.loadBids(connection);
    const asks = await market.loadAsks(connection);
    // Asks L2 orderbook data
    const minAskPrice = asks.getL2(1)[0][0];
    // Bids L2 orderbook data
    const maxBidPrice = bids.getL2(1)[0][0];

    const marketPrice = (minAskPrice + maxBidPrice) / 2;
    const result = {};
    result["marketAddress"] = marketAddress;
    result["bidsAddress"] = market.bidsAddress.toBase58();
    result["asksAddress"] = market.asksAddress.toBase58();
    result["marketPrice"] = marketPrice;
    return result;
}

function getSerumConfigs(network) {
    const serumConfigs = JSON.parse(fs.readFileSync("./serum/" + network + ".json"));
    if (serumConfigs.serumProgramId === undefined) {
        throw Error("serumProgramId is not configured");
    }
    const result = {};
    result["serumProgramId"] = new web3.PublicKey(serumConfigs.serumProgramId);
    result["serumMarkets"] = serumConfigs.serumMarkets;
    return result;
}

module.exports = {
    getSerumNameToAccountsAndPrice: async (network, connection) => {
        const serumNameToAccountsAndPrice = {};
        if (network == "mainnet-beta") {
            const { serumProgramId, serumMarkets } = getSerumConfigs(network);
            for (const serumMarket of serumMarkets) {
                const serumResult = await getSerumAccountsAndMarketPrice(
                    serumMarket.marketAddress,
                    connection,
                    serumProgramId,
                );
                serumNameToAccountsAndPrice[serumMarket.marketName] = serumResult;
            }
        }
        return serumNameToAccountsAndPrice;
    },

    validateSerumMarket: async (poolName, marketAddress, mintA, mintB, connection, network) => {
        if (network != "mainnet-beta") {
            throw Error("Only mainnet-beta network can support SERUM_ONLY for pool: " + poolName);
        }
        const marketPubkey = new web3.PublicKey(marketAddress);
        const { serumProgramId } = getSerumConfigs(network);
        const market = await Market.load(connection, marketPubkey, {}, serumProgramId);
        const mintAPubkey = new web3.PublicKey(mintA);
        const mintBPubkey = new web3.PublicKey(mintB);

        if ( market.baseMintAddress.equals(mintAPubkey) && market.quoteMintAddress.equals(mintBPubkey) ) {
            return true;
        } else {
            throw new Error("Serum Market Token Mint verification failed for pool: " + poolName);
        }
    }
}
