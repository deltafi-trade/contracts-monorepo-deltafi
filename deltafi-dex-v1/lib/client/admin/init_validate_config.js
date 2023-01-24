const { exit } = require("process");
const web3 = require('@solana/web3.js');
const deltafi = require('../lib/index.cjs');
const { MintLayout } = require('../lib/index.cjs.js');
const Ajv = require("ajv")


module.exports = async function validateConfig(config) {
    console.info("validating config file");

    const ajv = new Ajv();
    const isExisted = value => value ? true : false;
    const isNumber = value => typeof value === "number";
    const isPubkey = value => {
        try{
            new web3.PublicKey(value);
            return true;
        } catch {
            return false;
        }
    }

    // schema of the config file
    const schema = {
        properties: {
            network: {type: "string"},
            adminKeyName: {type: "string"},
            payerKeyName: {type: "string"},
            fees: {
                "not": {"type": "null"},
                type: "object",
                properties: {
                    adminTradeFeeDenominator: {type: "integer"},
                    adminTradeFeeNumerator: {type: "integer"},
                    adminWithdrawFeeNumerator: {type: "integer"},
                    adminWithdrawFeeDenominator: {type: "integer"},
                    tradeFeeNumerator: {type: "integer"},
                    tradeFeeDenominator: {type: "integer"},
                    withdrawFeeNumerator: {type: "integer"},
                    withdrawFeeDenominator: {type: "integer"},
                },
                required: ["adminTradeFeeDenominator", "adminTradeFeeNumerator", "adminWithdrawFeeNumerator",
                           "adminWithdrawFeeDenominator", "tradeFeeNumerator", "tradeFeeDenominator",
                           "withdrawFeeNumerator", "withdrawFeeDenominator"]
            },
            swapPools: {
                type: "array",
                items: [
                    {
                        type: "object",
                        properties: {
                            swapOutLimitPercentage: {type: "integer"},
                            slope: {type: "string"},
                            tokenA: {type: "string"},
                            tokenB: {type: "string"},
                            oraclePriority: {type: "string"}
                        },
                        required: ["slope", "tokenA", "tokenB", "swapOutLimitPercentage", "oraclePriority"]
                    }
                ]
            },
            stableSwapPools: {
                type: "array",
                items: [
                    {
                        type: "object",
                        properties: {
                            swapOutLimitPercentage: {type: "integer"},
                            slope: {type: "string"},
                            tokenA: {type: "string"},
                            tokenB: {type: "string"}
                        },
                        required: ["slope", "tokenA", "tokenB", "swapOutLimitPercentage"]
                    }
                ]
            }
        },
        required: ["network", "adminKeyName", "payerKeyName", "fees", "swapPools", "stableSwapPools"],
        additionalProperties: false
    }

    // validate the schema
    const validate = ajv.compile(schema)
    const valid = validate(config)
    if (!valid) console.error(validate.errors)

    const checkEvaluations = evaluationList => {
        for (const [evaluation, errorMsg] of evaluationList) {
            if (evaluation !== true) {
                throw Error(errorMsg);
            }
        }
        return true;
    }

    checkEvaluations([
        // checks for shared fields
        [config.fees.adminTradeFeeNumerator <= config.fees.adminTradeFeeDenominator, "invalid fees.adminTradeFeeNumerator and fees.adminTradeFeeDenominator"],
        [config.fees.adminWithdrawFeeNumerator <= config.fees.adminWithdrawFeeDenominator, "invalid fees.adminWithdrawFeeNumerator and fees.adminWithdrawFeeDenominator"],
        [config.fees.tradeFeeNumerator <= config.fees.tradeFeeDenominator, "invalid fees.tradeFeeNumerator and fees.tradeFeeDenominator"],
        [config.fees.withdrawFeeNumerator <= config.fees.withdrawFeeDenominator, "invalid fees.withdrawFeeNumerator and fees.withdrawFeeDenominator"],
    ]);

    const checkPoolInfo = (poolInfo, isStable) => {
        if (parseInt(poolInfo.slope) >= 1000000000000 || parseInt(poolInfo.slope) < 0) {
            throw new Error(poolInfo.tokenA, poolInfo.tokenB, poolInfo.slope);
        }
        if (poolInfo.swapOutLimitPercentage < 0 || poolInfo.swapOutLimitPercentage > 100) {
            throw new Error(poolInfo.tokenA, poolInfo.tokenB, poolInfo.swapOutLimitPercentage);
        }
        if (!isStable && poolInfo.oraclePriority != "PYTH_ONLY" && poolInfo.oraclePriority != "SERUM_ONLY") {
            throw new Error(poolInfo.tokenA + "-" + poolInfo.tokenB + " oraclePriority: " + poolInfo.oraclePriority);
        }
    }

    config.swapPools.forEach((swapInfo) => checkPoolInfo(swapInfo, false));
    config.stableSwapPools.forEach((swapInfo) => checkPoolInfo(swapInfo, true));

    let connection;
    try {
        connection = new web3.Connection(deltafi.getClusterApiUrl(config.network), 'confirmed');
    } catch(e) {
        console.error("invalid network", config.network);
        exit(1);
    }

    console.info("config file is validated");
}
