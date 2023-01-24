import { PublicKey } from '@solana/web3.js';
import BigNumber from 'bignumber.js';

/// swap program id
//export const SWAP_PROGRAM_ID = new PublicKey('4KKPonMmrpqbeAJtRzrhdoMipE6AZ1H9f2uLTsphxHi8'); // mainnet

export const SWAP_PROGRAM_ID = new PublicKey('DEH6htv2rzSkpC3aFK7VKiCiPrpoeQ4CXQboAKPsPgRL'); // testnet, need this to pass jstest on CI

//export const SWAP_PROGRAM_ID = new PublicKey('CHeL13CBiwHSSE9GMNg3DHdtiTr5WGRbWV547EFyRH5b'); // localhost

/// pyth program id
//export const PYTH_PROGRAM_ID = new PublicKey('9unXTMia7Zivwe9VwwNAfvUXjtur5oFKVBhN3mvPu5MZ');
export const PYTH_PROGRAM_ID = new PublicKey('FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH');   // mainnet

/// swap directions - sell base
export const SWAP_DIRECTION_SELL_BASE = 0;

/// swap directions - sell quote
export const SWAP_DIRECTION_SELL_QUOTE = 1;

/** @internal */
export const DECIMALS = 9;

/** @internal */
export const WAD = new BigNumber('1e+12');
