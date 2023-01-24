const fs = require('fs');
const web3 = require('@solana/web3.js');
const token = require('@solana/spl-token');
const deltafi = require('../lib/index.cjs.js');
const BigNumber = require('bignumber.js');

const PYTH_ONLY = 0;
const SERUM_ONLY = 1;
const invalidPythOrSerumAddress = new web3.PublicKey("66666666666666666666666666666666666666666666");

module.exports = {
    initPoolStep1: async (deployContext, poolInfoA, poolInfoB) => {
        const {connection, payer, userOwner} = deployContext;

        const poolA = web3.Keypair.generate();
        const poolB = web3.Keypair.generate();
        const balanceForToken = await connection.getMinimumBalanceForRentExemption(token.AccountLayout.span);

        let transaction = new web3.Transaction();
        transaction
        .add(
            web3.SystemProgram.createAccount({
                fromPubkey: payer.publicKey,
                newAccountPubkey: poolA.publicKey,
                lamports: balanceForToken * (poolInfoA.symbol == "SOL" ? 1 : 2),
                space: token.AccountLayout.span,
                programId: token.TOKEN_PROGRAM_ID,
            })
        )
        .add(
            token.Token.createInitAccountInstruction(token.TOKEN_PROGRAM_ID, poolInfoA.mint, poolA.publicKey, userOwner.publicKey)
        )
        .add(
            web3.SystemProgram.createAccount({
                fromPubkey: payer.publicKey,
                newAccountPubkey: poolB.publicKey,
                lamports: balanceForToken * (poolInfoB.symbol == "SOL" ? 1 : 2),
                space: token.AccountLayout.span,
                programId: token.TOKEN_PROGRAM_ID,
            })
        )
        .add(
            token.Token.createInitAccountInstruction(token.TOKEN_PROGRAM_ID, poolInfoB.mint, poolB.publicKey, userOwner.publicKey)
        )

        await web3.sendAndConfirmTransaction(
            connection, transaction, [payer, poolA, poolB], {maxRetries: 3});

        const result = {};
        result["poolA_secret"] = Array.from(poolA.secretKey);
        result["poolA_pubkey"] = poolA.publicKey.toBase58();
        result["poolB_secret"] = Array.from(poolB.secretKey);
        result["poolB_pubkey"] = poolB.publicKey.toBase58();

        console.log("init pool " + poolInfoA.symbol + "-" + poolInfoB.symbol + " step1", "finished");
        console.log(result);
        return result;
    },

    initPoolStep2: async (deployContext, params) => {
        const {connection, payer, userOwner} = deployContext;
        const poolPubkeyA = params.poolA_pubkey;
        const poolPubkeyB = params.poolB_pubkey;

        let transaction = new web3.Transaction();

        const swapAccount = web3.Keypair.generate();
        const poolMint = web3.Keypair.generate();
        const poolToken = web3.Keypair.generate();

        const [authority, nonce] = await web3.PublicKey.findProgramAddress(
            [swapAccount.publicKey.toBuffer()],
            deployContext.swapProgramId,
        );

        const balanceTokenAccount = await connection.getMinimumBalanceForRentExemption(token.AccountLayout.span);
        const balanceForMint = await connection.getMinimumBalanceForRentExemption(token.MintLayout.span);

        const poolA_publicKey = new web3.PublicKey(poolPubkeyA);
        const poolB_publicKey = new web3.PublicKey(poolPubkeyB);
        transaction = new web3.Transaction()
            .add(
                web3.SystemProgram.createAccount({
                    fromPubkey: payer.publicKey,
                    newAccountPubkey: poolMint.publicKey,
                    lamports: balanceForMint * 2,
                    space: token.MintLayout.span,
                    programId: token.TOKEN_PROGRAM_ID,
                })
            )
            .add(token.Token.createInitMintInstruction(
                token.TOKEN_PROGRAM_ID, poolMint.publicKey, params.poolMintDecimals, authority, null))
            .add(
                web3.SystemProgram.createAccount({
                    fromPubkey: payer.publicKey,
                    newAccountPubkey: poolToken.publicKey,
                    lamports: balanceTokenAccount * 2,
                    space: token.AccountLayout.span,
                    programId: token.TOKEN_PROGRAM_ID,
                })
            )
            .add(
                token.Token.createInitAccountInstruction(
                    token.TOKEN_PROGRAM_ID,
                    poolMint.publicKey,
                    poolToken.publicKey,
                    userOwner.publicKey
                )
            )
            .add(
                token.Token.createSetAuthorityInstruction(
                    token.TOKEN_PROGRAM_ID,
                    poolA_publicKey,
                    authority,
                    'AccountOwner',
                    userOwner.publicKey,
                    []
                )
            )
            .add(
                token.Token.createSetAuthorityInstruction(
                    token.TOKEN_PROGRAM_ID,
                    poolB_publicKey,
                    authority,
                    'AccountOwner',
                    userOwner.publicKey,
                    []
                )
            );

        await web3.sendAndConfirmTransaction(
            connection, transaction, [payer, userOwner, poolMint, poolToken], {maxRetries: 5});

        const result = {};
        result["swap_secret"] = Array.from(swapAccount.secretKey);
        result["swap_pubkey"] = swapAccount.publicKey.toBase58();
        result["mint_secret"] = Array.from(poolMint.secretKey);
        result["mint_pubkey"] = poolMint.publicKey.toBase58();
        result["token_secret"] = Array.from(poolToken.secretKey);
        result["token_pubkey"] = poolToken.publicKey.toBase58();
        result["authority"] = authority.toBase58();
        result["nonce"] = nonce;
        result["poolA_pubkey"] = poolPubkeyA;
        result["poolB_pubkey"] = poolPubkeyB;

        console.log("init pool " + params.poolName + " step2", "finished");
        console.log(result);
        return result;
    },

    initPoolStep3: async (deployContext, params) => {
        const {connection, payer} = deployContext;

        const nonce = params.nonce;
        const swapAccount = web3.Keypair.fromSecretKey(
            Uint8Array.from(
                params.swap_secret
            )
        );
        const swapAccount_publicKey = new web3.PublicKey(params.swap_pubkey);
        const balanceForSwapInfo = await connection.getMinimumBalanceForRentExemption(deltafi.SWAP_INFO_SIZE);
        const authority = new web3.PublicKey(params.authority);

        const config = new web3.PublicKey(params.config_pubkey);
        const adminFeeA = new web3.PublicKey(params.adminFeeA);
        const adminFeeB = new web3.PublicKey(params.adminFeeB);

        const poolA_publicKey = new web3.PublicKey(params.poolA_pubkey);
        const poolB_publicKey = new web3.PublicKey(params.poolB_pubkey);
        const poolMint_pubkey = new web3.PublicKey(params.poolMint_pubkey);
        const poolToken_pubkey = new web3.PublicKey(params.poolToken_pubkey);

        let oracle_priority_flags = 0;
        let serumMarketAddressPubkey, bidsPubkey, asksPubkey, pythProductA, pythA, pythProductB, pythB;

        if (!params.useStableSwap) {
            switch(params.oraclePriority) {
                case "PYTH_ONLY":
                    oracle_priority_flags = PYTH_ONLY;
                    pythProductA = new web3.PublicKey(params.pythProductA);
                    pythA = new web3.PublicKey(params.pythPriceA);
                    pythProductB = new web3.PublicKey(params.pythProductB);
                    pythB = new web3.PublicKey(params.pythPriceB);
                    serumMarketAddressPubkey = invalidPythOrSerumAddress;
                    bidsPubkey = invalidPythOrSerumAddress;
                    asksPubkey = invalidPythOrSerumAddress;
                    break;
                case "SERUM_ONLY":
                    oracle_priority_flags = SERUM_ONLY;
                    pythProductA = invalidPythOrSerumAddress;
                    pythA = invalidPythOrSerumAddress;
                    pythProductB = invalidPythOrSerumAddress;
                    pythB = invalidPythOrSerumAddress;
                    serumMarketAddressPubkey = params.serumMarketAddress;
                    bidsPubkey = params.serumBidsAddress;
                    asksPubkey = params.serumAsksAddress;
                    break;
                default:
                    throw Error("Invalid oraclePriority");
            }
            console.log("params.oraclePriority: " + params.oraclePriority);
        } else {
            pythProductA = new web3.PublicKey(params.pythProductA);
            pythA = new web3.PublicKey(params.pythPriceA);
            pythProductB = new web3.PublicKey(params.pythProductB);
            pythB = new web3.PublicKey(params.pythPriceB);
        }
        
        const tokenA = new token.Token(
            connection,
            new web3.PublicKey(params.tokenAMint),
            token.TOKEN_PROGRAM_ID,
            payer
        );

        const payerTokenA = await tokenA.getOrCreateAssociatedAccountInfo(
            payer.publicKey
        )

        const tokenB = new token.Token(
            connection,
            new web3.PublicKey(params.tokenBMint),
            token.TOKEN_PROGRAM_ID,
            payer
        );

        const payerTokenB = await tokenB.getOrCreateAssociatedAccountInfo(
            payer.publicKey
        )

        const depositTransaction = new web3.Transaction()
            .add(
                token.Token.createTransferInstruction(
                    token.TOKEN_PROGRAM_ID,
                    payerTokenA.address,
                    poolA_publicKey,
                    payer.publicKey,
                    [],
                    params.initTokenAmountA,
                )
            )
            .add(
                token.Token.createTransferInstruction(
                    token.TOKEN_PROGRAM_ID,
                    payerTokenB.address,
                    poolB_publicKey,
                    payer.publicKey,
                    [],
                    params.initTokenAmountB,
                )
            );

        const signature = await web3.sendAndConfirmTransaction(
            connection,
            depositTransaction,
            [payer],
            {maxRetries: 5}
        );

        console.log("transfered with signature", signature);
        const initData =
        params.useStableSwap
        ?
        {
            nonce,
            // 0.0001
            slope: BigInt(100000000),
            tokenADecimals: params.tokenADecimals,
            tokenBDecimals: params.tokenBDecimals,
            tokenAAmount: params.initTokenAmountA,
            tokenBAmount: params.initTokenAmountB,
        }
        :
        {
            nonce,
            // 0.1
            slope: BigInt(100000000000),
            midPrice: new BigNumber(51),
            tokenADecimals: params.tokenADecimals,
            tokenBDecimals: params.tokenBDecimals,
            tokenAAmount: params.initTokenAmountA,
            tokenBAmount: params.initTokenAmountB,
            oraclePriorityFlags: oracle_priority_flags,
        }

        const transaction = new web3.Transaction()
            .add(
                web3.SystemProgram.createAccount({
                    fromPubkey: payer.publicKey,
                    newAccountPubkey: swapAccount_publicKey,
                    lamports: balanceForSwapInfo * 2,
                    space: deltafi.SWAP_INFO_SIZE,
                    programId: deployContext.swapProgramId,
                })
            )
            .add(
                params.useStableSwap
                ?
                deltafi.createInitStableSwapInstruction(
                    config,
                    swapAccount_publicKey,
                    authority,
                    adminFeeA,
                    adminFeeB,
                    poolA_publicKey,
                    poolB_publicKey,
                    poolMint_pubkey,
                    poolToken_pubkey,
                    deployContext.userOwner.publicKey,
                    initData,
                    deployContext.swapProgramId,
                )
                :
                deltafi.createInitSwapInstruction(
                    config,
                    swapAccount_publicKey,
                    authority,
                    adminFeeA,
                    adminFeeB,
                    poolA_publicKey,
                    poolB_publicKey,
                    poolMint_pubkey,
                    poolToken_pubkey,
                    pythProductA,
                    pythA,
                    pythProductB,
                    pythB,
                    serumMarketAddressPubkey,
                    bidsPubkey,
                    asksPubkey,
                    deployContext.userOwner.publicKey,
                    initData,
                    deployContext.swapProgramId,
                )
            );

        await web3.sendAndConfirmTransaction(
            connection, transaction, [payer, swapAccount, deployContext.userOwner], {maxRetries: 5}
        );

        const result = {};
        result["swap_pubkey"] = params.swap_pubkey;
        result["mint_pubkey"] = params.poolMint_pubkey;
        console.log("init pool " + params.poolName + " step3", "finished");
        console.log(result);
        return result;
    }
}
