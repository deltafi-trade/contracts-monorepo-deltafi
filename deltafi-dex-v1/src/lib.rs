#![deny(missing_docs)]

//! An Uniswap-like program for the Solana blockchain.

pub mod admin;
pub mod curve;
pub mod entrypoint;
pub mod error;
pub mod instruction;
pub mod math;
pub mod processor;
pub mod pyth;
pub mod state;
pub mod utils;

// Export current solana-program types for downstream users who may also be
// building with a different solana-program version
pub use solana_program;

/// Serum-Dex V3 mainnet program id
pub const SERUM_DEX_V3_PROGRAM_ID: &str = "9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin";

const DUMMY_REFERRER_ADDRESS: &str = "66666666666666666666666666666666666666666666";
solana_program::declare_id!("D3UC98n8VwyUUJFQeNshAb1VeZWKXjgWMzvAzK7JX3r7");
