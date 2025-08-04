// File: src/model.rs
// Version: 1.2.1

use serde::{Deserialize, Serialize};
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeaderLite {
    pub version: u16,
    pub height: u64, // Added to match cli_view.rs and decoder.rs
    pub previous_hash: String,
    pub timestamp: u64,
    pub nonce: u64,
    pub pow_algo: u8,
    pub confirmations: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockSummary {
    pub height: u64,
    pub hash: String,
    pub header: BlockHeaderLite,
}

#[derive(Debug)]
pub enum BlockFilter {
    LastN(usize),
    Range(u64, u64),
    Specific(u64),
}

#[derive(Debug)]
pub struct TransactionSummary {
    pub inputs: Vec<InputSummary>,
    pub outputs: Vec<OutputSummary>,
    pub kernels: Vec<KernelSummary>,
}

#[derive(Debug)]
pub struct InputSummary {
    pub commitment: String,
    pub input_type: String,
}

#[derive(Debug)]
pub struct OutputSummary {
    pub commitment: String,
    pub features: String,
    pub script_type: String,
}

#[derive(Debug)]
pub struct KernelSummary {
    pub excess: String,
    pub fee: u64,
    pub lock_height: u64,
}

#[derive(Debug)]
pub struct BlockDetailSummary {
    pub height: u64,
    pub hash: String,
    pub header: BlockHeaderLite,
    pub transactions: TransactionSummary,
}

impl BlockSummary {
    pub fn from_raw(k: &[u8], v: &[u8]) -> Result<Self> {
        if k.len() != 8 {
            return Err(anyhow!("Invalid key length for height"));
        }

        let height = u64::from_le_bytes(k.try_into().unwrap());
        let header: BlockHeaderLite = bincode::deserialize(v)?;
        let hash = blake3::hash(v).to_hex().to_string();

        Ok(BlockSummary {
            height,
            hash,
            header,
        })
    }
}