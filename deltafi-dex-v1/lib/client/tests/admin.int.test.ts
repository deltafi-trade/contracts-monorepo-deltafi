import { Keypair, Connection, LAMPORTS_PER_SOL } from '@solana/web3.js';

import { createTestConfigInfo, newAccountWithLamports, sleep, TestConfigInfo } from './helpers';
import { CLUSTER_URL, BOOTSTRAP_TIMEOUT, DEFAULT_REWARDS, DEFAULT_FEES } from './constants';
import { loadConfig, SWAP_PROGRAM_ID } from '../src';

describe('e2e test for admin instructions', () => {
  // Cluster connection
  let connection: Connection;
  // Fee payer
  let payer: Keypair;
  // Test config
  let testConfigInfo: TestConfigInfo;

  beforeAll(async () => {
    // Bootstrap test env
    connection = new Connection(CLUSTER_URL, 'single');
    payer = await newAccountWithLamports(connection, LAMPORTS_PER_SOL);

    testConfigInfo = await createTestConfigInfo(connection, payer);

    sleep(5000);
  }, BOOTSTRAP_TIMEOUT);

  it('load configuration', async () => {
    const loadedConfig = await loadConfig(connection, testConfigInfo.config, SWAP_PROGRAM_ID);

    expect(loadedConfig.adminKey).toEqual(testConfigInfo.admin.publicKey);
    expect(loadedConfig.rewards.decimals.toString()).toEqual(
      DEFAULT_REWARDS.decimals.toString()
    );
    expect(loadedConfig.rewards.tradeRewardNumerator.toString()).toEqual(
      DEFAULT_REWARDS.tradeRewardNumerator.toString()
    );
    expect(loadedConfig.rewards.tradeRewardDenominator.toString()).toEqual(
      DEFAULT_REWARDS.tradeRewardDenominator.toString()
    );
    expect(loadedConfig.rewards.tradeRewardCap.toString()).toEqual(DEFAULT_REWARDS.tradeRewardCap.toString());
    expect(loadedConfig.fees.adminTradeFeeNumerator.toString()).toEqual(DEFAULT_FEES.adminTradeFeeNumerator.toString());
    expect(loadedConfig.fees.adminTradeFeeDenominator.toString()).toEqual(
      DEFAULT_FEES.adminTradeFeeDenominator.toString()
    );
    expect(loadedConfig.fees.adminWithdrawFeeNumerator.toString()).toEqual(
      DEFAULT_FEES.adminWithdrawFeeNumerator.toString()
    );
    expect(loadedConfig.fees.adminWithdrawFeeDenominator.toString()).toEqual(
      DEFAULT_FEES.adminWithdrawFeeDenominator.toString()
    );
    expect(loadedConfig.fees.tradeFeeNumerator.toString()).toEqual(DEFAULT_FEES.tradeFeeNumerator.toString());
    expect(loadedConfig.fees.tradeFeeDenominator.toString()).toEqual(DEFAULT_FEES.tradeFeeDenominator.toString());
    expect(loadedConfig.fees.withdrawFeeNumerator.toString()).toEqual(DEFAULT_FEES.withdrawFeeNumerator.toString());
    expect(loadedConfig.fees.withdrawFeeDenominator.toString()).toEqual(DEFAULT_FEES.withdrawFeeDenominator.toString());
  });
});
