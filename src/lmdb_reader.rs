// File: src/lmdb_reader.rs
// Version: 2.9.0 - Fixed frontend to display computed hash instead of header bytes

use std::path::Path;
use lmdb_zero::{EnvBuilder, Database, ReadTransaction, ConstAccessor};
use lmdb_zero::DatabaseOptions;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use hex;
use tari_utilities::byte_array::ByteArray;

// Import Tari's actual structs
use tari_core::blocks::BlockHeader;
use tari_core::transactions::transaction_components::{TransactionInput, TransactionOutput, TransactionKernel};
use tari_common_types::types::FixedHash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInputRowData {
    pub input: TransactionInput,
    pub header_hash: FixedHash,
    pub spent_timestamp: u64,
    pub spent_height: u64,
    pub hash: FixedHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionOutputRowData {
    pub output: TransactionOutput,
    pub header_hash: FixedHash,
    pub hash: FixedHash,
    pub mined_height: u64,
    pub mined_timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionKernelRowData {
    pub kernel: TransactionKernel,
    pub header_hash: FixedHash,
    pub mmr_position: u64,
    pub hash: FixedHash,
}

#[derive(Debug)]
pub enum BlockFilter {
    LastN(usize),           // Show last N blocks
    Range(u64, u64),        // Show blocks from start to end (inclusive)
    Specific(u64),          // Show specific block height
}

#[derive(Debug)]
pub struct BlockHeaderLite {
    #[allow(dead_code)]
    pub version: u16,
    #[allow(dead_code)]
    pub height: u64,
    #[allow(dead_code)]
    pub previous_hash: String,
    pub timestamp: u64,
    #[allow(dead_code)]
    pub nonce: u64,
}

#[derive(Debug)]
pub struct BlockSummary {
    pub height: u64,
    pub hash: String,
    pub header: BlockHeaderLite,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub inputs: Vec<InputSummary>,
    pub outputs: Vec<OutputSummary>, 
    pub kernels: Vec<KernelSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InputSummary {
    pub commitment: String,
    pub input_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OutputSummary {
    pub commitment: String,
    pub features: String,
    #[allow(dead_code)]
    pub script_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
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

impl From<(u64, String, BlockHeader)> for BlockSummary {
    fn from((height, hash, header): (u64, String, BlockHeader)) -> Self {
        Self {
            height,
            hash,
            header: BlockHeaderLite {
                version: header.version,
                height: header.height,
                previous_hash: hex::encode(&header.prev_hash[..]),
                timestamp: header.timestamp.as_u64(),
                nonce: header.nonce,
            },
        }
    }
}

/// Read block headers with filtering options
pub fn read_lmdb_headers_with_filter(path: &Path, db_name: &str, filter: BlockFilter) -> Result<Vec<BlockSummary>> {
    let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;

    let mut builder = EnvBuilder::new()?;
    builder.set_maxdbs(32)?;

    let env = unsafe {
        builder.open(path_str, lmdb_zero::open::Flags::empty(), 0o600)?
    };

    let db = Database::open(&env, Some(db_name), &DatabaseOptions::defaults())?;
    let txn = ReadTransaction::new(&env)?;
    let access = txn.access();
    let mut cursor = txn.cursor(&db)?;

    let mut all_blocks = Vec::new();

    if let Ok((mut k, mut v)) = cursor.first::<[u8], [u8]>(&access) {
        loop {
            let height = u64::from_le_bytes(k.try_into().unwrap_or([0; 8]));
            let value = v;

            // Try to deserialize using Tari's actual BlockHeader struct
            match bincode::deserialize::<BlockHeader>(value) {
                Ok(block_header) => {
                    // Use the computed block hash (actual block hash for frontend display)
                    let block_hash = hex::encode(block_header.hash().as_slice());
                    
                    all_blocks.push(BlockSummary::from((height, block_hash, block_header)));
                },
                Err(e) => {
                    eprintln!("Failed to deserialize block header for height {}: {}", height, e);
                    // Continue to next block instead of failing completely
                }
            }

            match cursor.next::<[u8], [u8]>(&access) {
                Ok((next_k, next_v)) => {
                    k = next_k;
                    v = next_v;
                }
                Err(_) => break,
            }
        }
    }

    // Apply filter
    let summaries = match filter {
        BlockFilter::LastN(n) => {
            let start_idx = all_blocks.len().saturating_sub(n);
            all_blocks.into_iter().skip(start_idx).collect()
        },
        BlockFilter::Range(start, end) => {
            all_blocks.into_iter()
                .filter(|block| block.height >= start && block.height <= end)
                .collect()
        },
        BlockFilter::Specific(height) => {
            all_blocks.into_iter()
                .filter(|block| block.height == height)
                .collect()
        },
    };

    Ok(summaries)
}

/// Read a specific block with transaction details
pub fn read_block_with_transactions(path: &Path, height: u64) -> Result<BlockDetailSummary> {
    let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;

    let mut builder = EnvBuilder::new()?;
    builder.set_maxdbs(40)?;

    let env = unsafe {
        builder.open(path_str, lmdb_zero::open::Flags::empty(), 0o600)?
    };

    // Open databases as Results to match original
    let headers_db = Database::open(&env, Some("headers"), &DatabaseOptions::defaults())?;
    let utxos_result = Database::open(&env, Some("utxos"), &DatabaseOptions::defaults());
    let inputs_result = Database::open(&env, Some("inputs"), &DatabaseOptions::defaults());
    let kernels_result = Database::open(&env, Some("kernels"), &DatabaseOptions::defaults());

    println!("Database availability:");
    println!("  headers: ✅ Available");
    println!("  utxos: {}", if utxos_result.is_ok() { "✅ Available" } else { "❌ Not found" });
    println!("  inputs: {}", if inputs_result.is_ok() { "✅ Available" } else { "❌ Not found" });
    println!("  kernels: {}", if kernels_result.is_ok() { "✅ Available" } else { "❌ Not found" });

    let txn = ReadTransaction::new(&env)?;
    let access = txn.access();

    // Get block header
    let height_bytes = height.to_le_bytes();
    let header_data: &[u8] = access.get(&headers_db, &height_bytes)
        .map_err(|_| anyhow::anyhow!("Block not found at height {}", height))?;

    let block_header: BlockHeader = bincode::deserialize(header_data)?;
    let block_hash = hex::encode(&block_header.hash());

    let block_hash_bytes = block_header.hash();

    // Fetch outputs
    let mut outputs = Vec::new();
    if let Ok(ref utxos_db) = utxos_result {
        let mut cursor = txn.cursor(&*utxos_db)?;
        if cursor.seek_range_k::<[u8], [u8]>(&access, block_hash_bytes.as_slice()).is_ok() {
            loop {
                match cursor.get_current::<[u8], [u8]>(&access) {
                    Ok((key, value)) => {
                        if !key.starts_with(block_hash_bytes.as_slice()) {
                            break;
                        }
                        let row: TransactionOutputRowData = bincode::deserialize(value)?;
                        outputs.push(OutputSummary {
                            commitment: hex::encode(row.output.commitment.as_bytes()),
                            features: serde_json::to_string(&row.output.features).unwrap_or_default(),
                            script_type: format!("{:?}", row.output.script),
                        });
                        let _ = cursor.next::<[u8], [u8]>(&access);
                    }
                    Err(e) => return Err(anyhow::anyhow!("Cursor error in outputs: {}", e)),
                }
            }
        }
    }

    // Fetch inputs
    let mut inputs = Vec::new();
    if let Ok(ref inputs_db) = inputs_result {
        let mut cursor = txn.cursor(&*inputs_db)?;
        if cursor.seek_range_k::<[u8], [u8]>(&access, block_hash_bytes.as_slice()).is_ok() {
            loop {
                match cursor.get_current::<[u8], [u8]>(&access) {
                    Ok((key, value)) => {
                        if !key.starts_with(block_hash_bytes.as_slice()) {
                            break;
                        }
                        let row: TransactionInputRowData = bincode::deserialize(value)?;
                        inputs.push(InputSummary {
                            commitment: hex::encode(row.input.commitment()?.as_bytes()),
                            input_type: format!("{:?}", row.input),
                        });
                        let _ = cursor.next::<[u8], [u8]>(&access);
                    }
                    Err(e) => return Err(anyhow::anyhow!("Cursor error in inputs: {}", e)),
                }
            }
        }
    }

    // Fetch kernels
    let mut kernels = Vec::new();
    if let Ok(ref kernels_db) = kernels_result {
        let mut cursor = txn.cursor(&*kernels_db)?;
        if cursor.seek_range_k::<[u8], [u8]>(&access, block_hash_bytes.as_slice()).is_ok() {
            loop {
                match cursor.get_current::<[u8], [u8]>(&access) {
                    Ok((key, value)) => {
                        if !key.starts_with(block_hash_bytes.as_slice()) {
                            break;
                        }
                        let row: TransactionKernelRowData = bincode::deserialize(value)?;
                        kernels.push(KernelSummary {
                            excess: hex::encode(row.kernel.excess.as_bytes()),
                            fee: row.kernel.fee.0,
                            lock_height: row.kernel.lock_height,
                        });
                        let _ = cursor.next::<[u8], [u8]>(&access);
                    }
                    Err(e) => return Err(anyhow::anyhow!("Cursor error in kernels: {}", e)),
                }
            }
        }
    }

    // Get database summaries (existing code)
    let utxos_count = if let Ok(utxos_db) = utxos_result {
        count_database_entries(&txn, &access, &utxos_db, "UTXOs")
    } else {
        0
    };
    let inputs_count = if let Ok(inputs_db) = inputs_result {
        count_database_entries(&txn, &access, &inputs_db, "Inputs")
    } else {
        0
    };
    let kernels_count = if let Ok(kernels_db) = kernels_result {
        count_database_entries(&txn, &access, &kernels_db, "Kernels")
    } else {
        0
    };

    println!("📊 Transaction Database Summary:");
    println!("  💰 UTXOs (Outputs):     {:>8} transactions", utxos_count);
    println!("  📥 Inputs:              {:>8} transactions", inputs_count);
    println!("  ⚡ Kernels:             {:>8} transactions", kernels_count);
    println!("  📈 Total Transactions:  {:>8}", kernels_count);
    println!("  🔗 Total I/O Records:   {:>8}", utxos_count + inputs_count);

    Ok(BlockDetailSummary {
        height,
        hash: block_hash,
        header: BlockHeaderLite {
            version: block_header.version,
            height: block_header.height,
            previous_hash: hex::encode(&block_header.prev_hash[..]),
            timestamp: block_header.timestamp.as_u64(),
            nonce: block_header.nonce,
        },
        transactions: TransactionSummary {
            inputs,
            outputs,
            kernels,
        },
    })
}

/// Efficiently count database entries with progress and limits
fn count_database_entries(
    txn: &ReadTransaction,
    access: &ConstAccessor,
    db: &Database,
    db_type: &str
) -> usize {
    print!("🔍 Counting {} database entries... ", db_type);
    
    match txn.cursor(db) {
        Ok(mut cursor) => {
            if let Ok((_key, _value)) = cursor.first::<[u8], [u8]>(access) {
                let mut count = 1;
                let max_count = 10_000_000; // Increased limit to 10M
                
                // Count with progress indicators
                while count < max_count {
                    match cursor.next::<[u8], [u8]>(access) {
                        Ok((_next_key, _next_value)) => {
                            count += 1;
                            
                            // Show progress every 250k entries for faster counting
                            if count % 250_000 == 0 {
                                print!("{}M ", count / 1_000_000);
                            }
                        }
                        Err(_) => break, // End of database
                    }
                }
                
                if count >= max_count {
                    println!("→ 10M+ entries (stopped counting)");
                    count
                } else {
                    println!("→ {} total entries", count);
                    count
                }
            } else {
                println!("→ 0 entries (empty)");
                0
            }
        },
        Err(_) => {
            println!("→ Error accessing database");
            0
        }
    }
}

/// Default function to read last 10 headers
#[allow(dead_code)]
pub fn read_lmdb_headers(path: &Path, db_name: &str) -> Result<Vec<BlockSummary>> {
    read_lmdb_headers_with_filter(path, db_name, BlockFilter::LastN(10))
}