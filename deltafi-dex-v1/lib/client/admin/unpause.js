const web3 = require('@solana/web3.js');
const token = require('@solana/spl-token');
const deltafi = require('../lib/index.cjs');

const oracleProgramId = new web3.PublicKey('8tfDNiaEyrV6Q1U4DEXrEigs9DoDtkugzFbybENEbCDz');

async function main() {
  let connection = new web3.Connection(web3.clusterApiUrl('testnet'), 'confirmed');

  const payerMintKeypair = web3.Keypair.fromSecretKey(
    Uint8Array.from([
      172, 104, 50, 164, 205, 231, 155, 208, 3, 67, 204, 240, 165, 54, 208, 11, 48, 206, 188, 92, 26, 103, 13, 169, 137,
      132, 43, 188, 157, 162, 61, 15, 155, 122, 61, 128, 56, 99, 206, 118, 35, 170, 153, 103, 153, 105, 14, 156, 6, 240,
      35, 230, 182, 184, 226, 107, 160, 239, 84, 105, 142, 250, 209, 149,
    ])
  );

  console.log('Payer pubkey: ', payerMintKeypair.publicKey.toBase58());

  const ownerMintKeypair = web3.Keypair.fromSecretKey(
    Uint8Array.from([
      31, 182, 122, 142, 130, 181, 114, 40, 114, 229, 10, 214, 188, 71, 141, 80, 159, 10, 154, 190, 8, 60, 36, 32, 39,
      47, 133, 40, 105, 88, 10, 30, 95, 233, 181, 223, 48, 102, 213, 81, 57, 136, 130, 214, 87, 6, 127, 119, 103, 114,
      43, 49, 151, 106, 144, 140, 251, 73, 30, 171, 223, 99, 115, 247,
    ])
  );

  console.log('Owner pubkey: ', ownerMintKeypair.publicKey.toBase58());

  const configPubKey = new web3.PublicKey('2DTTQ5EFWLRAfCb3M5yb9jbz61cbqirr9WzmbzWi9Q2t'); // Config
  const poolPubKey = new web3.PublicKey('EdJu4CF21nGmi21Yyu4tK7JfVn7XxKJEekCJF5UafJNJ'); // SwapInfo

  console.log("config pubkey: ", configPubKey);
  console.log("pool pubkey: ", poolPubKey);

  console.log("program id: ", deltafi.SWAP_PROGRAM_ID);

  await deltafi.unpause(connection, payerMintKeypair, configPubKey, poolPubKey, ownerMintKeypair);

  console.info('UnPausing pool... ', poolPubKey.toBase58());
}

main()
  .then(() => console.info('Success!'))
  .catch((err) => console.error(err));
