// File: src/key_inspector.rs
// Version: 1.0.1 - LMDB key structure investigation and debugging tools (FIXED)
// Tree: tari-lmdb-inspector/src/key_inspector.rs
//
// This module provides debugging tools to investigate how Tari stores transaction data
// in LMDB and understand the key structures used to link blocks to their transactions.
// Essential for understanding the database schema and building correct data readers.

use std::path::Path;
use lmdb_zero::{EnvBuilder, Database, ReadTransaction};
use lmdb_zero::DatabaseOptions;
use anyhow::Result;
use hex;

/// Check which LMDB databases are available in the Tari data directory
/// This helps identify what transaction tables exist and can be queried
/// 
/// # Arguments
/// * `path` - Path to the Tari LMDB database directory
/// 
/// # Returns
/// * `Result<()>` - Success if database can be opened, error otherwise
pub fn check_database_availability(path: &Path) -> Result<()> {
    let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;

    let mut builder = EnvBuilder::new()?;
    builder.set_maxdbs(40)?; // Tari uses many sub-databases

    let env = unsafe {
        builder.open(path_str, lmdb_zero::open::Flags::empty(), 0o600)?
    };

    // List of core Tari LMDB tables we're interested in investigating
    let tables = vec![
        ("headers", "Block headers with metadata"),
        ("kernels", "Transaction kernels (fees, signatures)"), 
        ("inputs", "Transaction inputs (spent outputs)"),
        ("utxos", "Transaction outputs (unspent)"),
        ("kernel_excess_index", "Index: kernel excess → block mapping"),
        ("txos_hash_to_index", "Index: output hash → index mapping"), 
        ("deleted_txo_hash_to_header_index", "Index: spent output → block mapping"),
        ("block_hashes", "Index: block hash → height mapping"),
        ("header_accumulated_data", "Accumulated blockchain data per block"),
        ("mmr_peak_data", "Merkle Mountain Range peak data"),
    ];

    println!("📋 Database Availability Check:");
    println!("Path: {:?}", path);
    println!("{}", "-".repeat(70));
    
    let mut available_count = 0;
    let mut total_count = 0;
    
    for (table_name, description) in tables {
        total_count += 1;
        match Database::open(&env, Some(table_name), &DatabaseOptions::defaults()) {
            Ok(_) => {
                println!("  ✅ {:25} - {}", table_name, description);
                available_count += 1;
            },
            Err(_) => {
                println!("  ❌ {:25} - Not found", table_name);
            },
        }
    }
    
    println!("{}", "-".repeat(70));
    println!("📊 Summary: {}/{} tables available", available_count, total_count);
    
    if available_count == 0 {
        anyhow::bail!("No Tari LMDB tables found. Check database path.");
    }

    Ok(())
}

/// Thorough investigation: Compare our linking hash to actual transaction table keys
/// This will show us if our theory is correct or if we need a different approach
pub fn investigate_transaction_keys_thoroughly(path: &Path, block_height: u64) -> Result<()> {
    let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;

    let mut builder = EnvBuilder::new()?;
    builder.set_maxdbs(5)?;
    builder.set_maxreaders(1)?;

    let env = unsafe {
        builder.open(path_str, lmdb_zero::open::Flags::empty(), 0o444)?
    };

    println!("\n🔍 Thorough Transaction Key Investigation for Block {}", block_height);
    println!("{}", "=".repeat(70));

    // Get block header and linking hash
    let headers_db = Database::open(&env, Some("headers"), &DatabaseOptions::defaults())?;
    let txn = ReadTransaction::new(&env)?;
    let access = txn.access();

    let height_bytes = block_height.to_le_bytes();
    let header_data: &[u8] = access.get(&headers_db, &height_bytes)
        .map_err(|_| anyhow::anyhow!("Block not found"))?;

    let linking_hash_bytes = &header_data[0..32];
    println!("Our linking hash: {}", hex::encode(linking_hash_bytes));

    // Test each transaction table systematically
    let tables = vec![
        ("kernels", "Transaction kernels"),
        ("utxos", "Transaction outputs"), 
        ("inputs", "Transaction inputs"),
    ];

    for (table_name, description) in tables {
        println!("\n📊 Testing {} - {}", table_name, description);
        
        match Database::open(&env, Some(table_name), &DatabaseOptions::defaults()) {
            Ok(db) => {
                investigate_single_table(&txn, &access, &db, table_name, linking_hash_bytes)?;
            },
            Err(e) => {
                println!("❌ Failed to open {}: {:?}", table_name, e);
            }
        }
    }

    Ok(())
}

/// Investigate a single transaction table to understand its key structure
fn investigate_single_table(
    txn: &ReadTransaction,
    access: &lmdb_zero::ConstAccessor,
    db: &lmdb_zero::Database,
    table_name: &str,
    our_linking_hash: &[u8],
) -> Result<()> {
    
    println!("🔍 Investigating {} table structure...", table_name);
    
    // Try creating cursor
    match txn.cursor(db) {
        Ok(mut cursor) => {
            println!("  ✅ Cursor created successfully");
            
            // Get first few entries to see actual key patterns
            match cursor.first::<[u8], [u8]>(access) {
                Ok((mut key, mut value)) => {
                    println!("  📊 Analyzing actual keys in {} table:", table_name);
                    
                    // Show first 5 keys to understand the pattern
                    for i in 0..5 {
                        println!("    Entry {}: Key length: {} bytes", i + 1, key.len());
                        
                        if key.len() >= 32 {
                            let key_prefix = &key[0..32];
                            println!("      Key prefix (32 bytes): {}", hex::encode(key_prefix));
                            
                            // Check if this prefix matches our linking hash
                            if key_prefix == our_linking_hash {
                                println!("      🎉 MATCH! This key starts with our linking hash!");
                            } else {
                                println!("      ❌ Different from our linking hash");
                            }
                        } else {
                            println!("      Key (full): {}", hex::encode(key));
                        }
                        
                        println!("      Value size: {} bytes", value.len());
                        
                        // Try to move to next entry
                        match cursor.next::<[u8], [u8]>(access) {
                            Ok((next_key, next_value)) => {
                                key = next_key;
                                value = next_value;
                            }
                            Err(_) => {
                                println!("    (End of table reached)");
                                break;
                            }
                        }
                    }
                    
                    // Now try seek_range with our linking hash
                    println!("  🔍 Testing seek_range with our linking hash...");
                    match cursor.seek_range_k::<[u8], [u8]>(access, our_linking_hash) {
                        Ok((found_key, _)) => {
                            println!("    ✅ Seek successful!");
                            if found_key.starts_with(our_linking_hash) {
                                println!("    🎉 Found key starting with our linking hash!");
                                println!("       Key: {}", hex::encode(&found_key[0..std::cmp::min(64, found_key.len())]));
                                
                                // Count how many entries have this prefix
                                let mut count = 1;
                                while let Ok((next_key, _)) = cursor.next::<[u8], [u8]>(access) {
                                    if next_key.starts_with(our_linking_hash) {
                                        count += 1;
                                        if count > 100 { break; } // Limit counting
                                    } else {
                                        break;
                                    }
                                }
                                println!("    📊 Total entries with our prefix: {}", count);
                                
                            } else {
                                println!("    ❌ Seek found key, but doesn't start with our hash");
                                println!("       Found: {}", hex::encode(&found_key[0..32]));
                                println!("       Expected: {}", hex::encode(our_linking_hash));
                            }
                        },
                        Err(e) => {
                            println!("    ❌ Seek failed: {:?}", e);
                        }
                    }
                    
                },
                Err(e) => {
                    println!("  ❌ Failed to get first entry: {:?}", e);
                }
            }
        },
        Err(e) => {
            println!("  ❌ Failed to create cursor: {:?}", e);
        }
    }
    
    Ok(())
}

/// Simple test: Check if our block hash appears as a prefix in transaction tables
/// This will tell us if the composite key theory is correct
pub fn test_block_hash_as_prefix(path: &Path, block_height: u64) -> Result<()> {
    let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;

    let mut builder = EnvBuilder::new()?;
    builder.set_maxdbs(5)?;  // Even fewer databases
    builder.set_maxreaders(1)?;  // Limit readers

    let env = unsafe {
        // Use empty flags but with read permissions
        builder.open(path_str, lmdb_zero::open::Flags::empty(), 0o444)?
    };

    println!("\n🎯 Simple Prefix Test for Block {}", block_height);
    println!("{}", "=".repeat(50));

    // Get block header data (RAW)
    let headers_db = Database::open(&env, Some("headers"), &DatabaseOptions::defaults())?;
    let txn = ReadTransaction::new(&env)?;
    let access = txn.access();

    let height_bytes = block_height.to_le_bytes();
    let header_data: &[u8] = access.get(&headers_db, &height_bytes)
        .map_err(|_| anyhow::anyhow!("Block not found"))?;

    // Extract the LINKING HASH (first 32 bytes of raw data)
    let linking_hash = &header_data[0..32];
    println!("Linking hash (first 32 bytes): {}", hex::encode(linking_hash));
    
    // Also show computed hash for comparison
    use tari_core::blocks::BlockHeader;
    let header: BlockHeader = bincode::deserialize(header_data)?;
    let computed_hash = header.hash();
    println!("Computed block hash:            {}", hex::encode(computed_hash.as_slice()));
    println!("🔍 Testing if LINKING HASH appears as transaction prefix...");

    // Test kernels table with LINKING HASH (not computed hash)
    match Database::open(&env, Some("kernels"), &DatabaseOptions::defaults()) {
        Ok(kernels_db) => {
            println!("\n🔍 Kernels database opened successfully");
            
            // Try cursor with LINKING HASH as prefix
            match txn.cursor(&kernels_db) {
                Ok(mut cursor) => {
                    println!("  ✅ Cursor created successfully");
                    
                    // Try seek_range with LINKING HASH
                    match cursor.seek_range_k::<[u8], [u8]>(&access, linking_hash) {
                        Ok((key, _value)) => {
                            println!("  ✅ Seek successful!");
                            if key.starts_with(linking_hash) {
                                println!("     🎉 FOUND! Key starts with our LINKING hash");
                                println!("     Full key: {}", hex::encode(&key[0..std::cmp::min(64, key.len())]));
                                
                                // Count entries with this prefix
                                let mut count = 1;
                                while let Ok((next_key, _)) = cursor.next::<[u8], [u8]>(&access) {
                                    if next_key.starts_with(linking_hash) {
                                        count += 1;
                                        if count > 10 { break; }
                                    } else {
                                        break;
                                    }
                                }
                                println!("     Found {} kernel entries for this block", count);
                                println!("     ✅ THEORY CONFIRMED: Table hash IS the linking key!");
                            } else {
                                println!("     ❌ Key doesn't start with our linking hash");
                                println!("     Found key: {}", hex::encode(&key[0..32]));
                                println!("     Expected:   {}", hex::encode(linking_hash));
                            }
                        },
                        Err(e) => {
                            println!("  ❌ Seek failed: {:?}", e);
                        }
                    }
                },
                Err(e) => {
                    println!("  ❌ Cursor creation failed: {:?}", e);
                }
            }
        },
        Err(e) => {
            println!("❌ Failed to open kernels database: {:?}", e);
        }
    }

    Ok(())
}

/// Inspect the key structure of a specific LMDB database
/// Shows actual key formats, lengths, and sample data to understand storage schema
/// 
/// # Arguments
/// * `path` - Path to the Tari LMDB database directory  
/// * `db_name` - Name of the specific database to inspect
/// * `max_samples` - Maximum number of sample keys to show
/// 
/// # Returns
/// * `Result<()>` - Success if inspection completed, error otherwise
pub fn inspect_database_keys(path: &Path, db_name: &str, max_samples: usize) -> Result<()> {
    let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;

    let mut builder = EnvBuilder::new()?;
    builder.set_maxdbs(40)?;

    let env = unsafe {
        builder.open(path_str, lmdb_zero::open::Flags::empty(), 0o600)?
    };

    let db = Database::open(&env, Some(db_name), &DatabaseOptions::defaults())?;
    let txn = ReadTransaction::new(&env)?;
    let access = txn.access();
    let mut cursor = txn.cursor(&db)?;

    println!("🔍 Inspecting database: {}", db_name);
    println!("{}", "=".repeat(50));

    if let Ok((mut k, mut v)) = cursor.first::<[u8], [u8]>(&access) {
        for i in 0..max_samples {
            println!("\nSample {}: ", i + 1);
            println!("  Key length: {} bytes", k.len());
            println!("  Key (hex):  {}", hex::encode(k));
            
            // Attempt to interpret key as common formats
            if k.len() == 8 {
                let key_u64 = u64::from_le_bytes(k.try_into().unwrap());
                println!("  Key as u64 (LE): {} (could be block height/MMR index)", key_u64);
            } else if k.len() == 32 {
                println!("  Key type: 32-byte hash (block/transaction/commitment hash)");
            } else if k.len() == 4 {
                let key_u32 = u32::from_le_bytes(k.try_into().unwrap());
                println!("  Key as u32 (LE): {} (could be index/counter)", key_u32);
            } else {
                println!("  Key type: Custom length ({} bytes) - composite key", k.len());
            }
            
            println!("  Value size: {} bytes", v.len());
            
            // Show first 32 bytes of value in hex for pattern recognition
            let preview_len = std::cmp::min(32, v.len());
            println!("  Value preview: {}", hex::encode(&v[0..preview_len]));
            
            // Try to advance to next entry
            match cursor.next::<[u8], [u8]>(&access) {
                Ok((next_k, next_v)) => {
                    k = next_k;
                    v = next_v;
                }
                Err(_) => {
                    println!("\n  (End of database reached after {} entries)", i + 1);
                    break;
                }
            }
        }
    } else {
        println!("Database is empty!");
    }

    Ok(())
}

/// Inspect key structures of all important transaction-related tables
/// Provides comprehensive overview of how Tari stores and organizes blockchain data
/// 
/// # Arguments  
/// * `path` - Path to the Tari LMDB database directory
/// 
/// # Returns
/// * `Result<()>` - Success if all inspections completed, error otherwise
pub fn inspect_all_transaction_tables(path: &Path) -> Result<()> {
    println!("🔍 LMDB Key Structure Investigation");
    println!("{}", "=".repeat(60));
    
    // Core transaction tables in order of importance
    let tables = vec![
        ("headers", "Block headers - should be keyed by height"),
        ("kernels", "Transaction kernels - investigate key structure"), 
        ("inputs", "Transaction inputs - investigate key structure"),
        ("utxos", "Transaction outputs - investigate key structure"),
        ("kernel_excess_index", "Kernel index - may link blocks to kernels"),
        ("txos_hash_to_index", "Output index - may link outputs to indices"),
        ("deleted_txo_hash_to_header_index", "Spent output index - may link inputs to blocks"),
        ("block_hashes", "Block hash index - may map hashes to heights"),
    ];

    for (table, description) in tables {
        println!("\n📊 Table: {} - {}", table, description);
        match inspect_database_keys(path, table, 3) {
            Ok(_) => {},
            Err(e) => println!("❌ Failed to inspect {}: {}", table, e),
        }
        println!("{}", "-".repeat(60));
    }

    println!("\n💡 Key Structure Analysis Tips:");
    println!("  • 8-byte keys (u64): Likely block height, MMR position, or index");
    println!("  • 32-byte keys: Likely cryptographic hashes (block, tx, commitment)");
    println!("  • Other lengths: Composite keys or custom formats");
    println!("  • Look for patterns between related tables");

    Ok(())
}

/// Investigate how a specific block height links to its transaction data
/// Tests different key strategies to understand the storage schema
/// 
/// # Arguments
/// * `path` - Path to the Tari LMDB database directory
/// * `block_height` - Block height to investigate
/// 
/// # Returns  
/// * `Result<()>` - Success if investigation completed, error otherwise
pub fn investigate_block_to_transaction_links(path: &Path, block_height: u64) -> Result<()> {
    let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;

    let mut builder = EnvBuilder::new()?;
    builder.set_maxdbs(40)?;

    let env = unsafe {
        builder.open(path_str, lmdb_zero::open::Flags::empty(), 0o600)?
    };

    println!("\n🔗 Block-to-Transaction Link Investigation for Height {}", block_height);
    println!("{}", "=".repeat(70));

    // First, get the block header to extract metadata
    let headers_db = Database::open(&env, Some("headers"), &DatabaseOptions::defaults())?;
    let txn = ReadTransaction::new(&env)?;
    let access = txn.access();

    let height_bytes = block_height.to_le_bytes();
    let header_data: &[u8] = access.get(&headers_db, &height_bytes)
        .map_err(|_| anyhow::anyhow!("Block not found at height {}", block_height))?;

    // Parse the header using Tari's BlockHeader struct
    use tari_core::blocks::BlockHeader;
    let header: BlockHeader = bincode::deserialize(header_data)?;
    let block_hash = header.hash();
    
    println!("📋 Block Information:");
    println!("  Height: {}", block_height);
    println!("  Hash: {}", hex::encode(block_hash.as_slice()));
    println!("  Timestamp: {}", header.timestamp.as_u64());
    println!("  Kernel MMR Size: {}", header.kernel_mmr_size);
    println!("  Output SMT Size: {}", header.output_smt_size);
    println!("  Previous Hash: {}", hex::encode(&header.prev_hash[..]));

    // Test different key strategies for each transaction table
    println!("\n🔍 Testing Transaction Table Key Strategies:");
    test_transaction_table_keys(&env, &txn, &access, "kernels", block_height, &block_hash, header.kernel_mmr_size)?;
    test_transaction_table_keys(&env, &txn, &access, "utxos", block_height, &block_hash, header.output_smt_size)?;
    test_transaction_table_keys(&env, &txn, &access, "inputs", block_height, &block_hash, 0)?;

    // Investigate index tables for potential linking mechanisms
    println!("\n🔗 Investigating Index Tables:");
    investigate_index_tables(&env, &txn, &access, block_height, &block_hash)?;

    Ok(())
}

/// Test various key strategies against a transaction table to find the correct approach
/// This is crucial for understanding how to query transaction data for a specific block
/// 
/// # Arguments
/// * `env` - LMDB environment handle
/// * `txn` - Read transaction handle  
/// * `access` - Database accessor
/// * `table_name` - Name of the table to test
/// * `block_height` - Block height to use in key tests
/// * `block_hash` - Block hash to use in key tests
/// * `mmr_size` - MMR size from block header to use in key tests
fn test_transaction_table_keys(
    env: &lmdb_zero::Environment,
    txn: &ReadTransaction,
    access: &lmdb_zero::ConstAccessor,
    table_name: &str,
    block_height: u64,
    block_hash: &tari_common_types::types::FixedHash,
    mmr_size: u64,
) -> Result<()> {
    
    match Database::open(env, Some(table_name), &DatabaseOptions::defaults()) {
        Ok(db) => {
            println!("\n🔍 Testing {} table key strategies:", table_name);
            
            // Define various key strategies to test
            let strategies = vec![
                ("Block height (u64 LE)", block_height.to_le_bytes().to_vec()),
                ("Block hash (32 bytes)", block_hash.as_slice().to_vec()),
                ("MMR size (u64 LE)", mmr_size.to_le_bytes().to_vec()),
                ("Height as u32", (block_height as u32).to_le_bytes().to_vec()),
            ];

            let mut found_any = false;
            for (strategy_name, key_bytes) in strategies {
                match access.get::<[u8], [u8]>(&db, &key_bytes) {
                    Ok(value) => {
                        println!("  ✅ {} - FOUND! Value size: {} bytes", strategy_name, value.len());
                        found_any = true;
                        
                        // Show preview of successful value
                        let preview = hex::encode(&value[0..std::cmp::min(32, value.len())]);
                        println!("     Value preview: {}...", preview);
                    },
                    Err(_) => println!("  ❌ {} - Not found", strategy_name),
                }
            }

            // Show actual key structure for context - create cursor more carefully
            match txn.cursor(&db) {
                Ok(mut cursor) => {
                    match cursor.first::<[u8], [u8]>(access) {
                        Ok((first_key, first_value)) => {
                            println!("  📊 Actual key structure in {}:", table_name);
                            println!("     Key length: {} bytes", first_key.len());
                            println!("     Key hex: {}", hex::encode(&first_key[0..std::cmp::min(32, first_key.len())]));
                            println!("     Value size: {} bytes", first_value.len());
                            
                            if !found_any {
                                println!("     💡 Keys appear to be composite - investigating prefix matching");
                            }
                        },
                        Err(e) => {
                            println!("  ⚠️  Error reading first entry from {}: {:?}", table_name, e);
                        }
                    }
                    // Cursor will be automatically dropped here
                },
                Err(e) => {
                    println!("  ⚠️  Error creating cursor for {}: {:?}", table_name, e);
                }
            }
        },
        Err(e) => {
            println!("\n❌ {} table not accessible: {:?}", table_name, e);
        }
    }

    Ok(())
}

/// Investigate index tables that may provide block-to-transaction mappings
/// These tables often contain the linking logic between blocks and their components
/// 
/// # Arguments
/// * `env` - LMDB environment handle  
/// * `txn` - Read transaction handle
/// * `access` - Database accessor
/// * `block_height` - Block height to investigate
/// * `block_hash` - Block hash to investigate
fn investigate_index_tables(
    env: &lmdb_zero::Environment,
    txn: &ReadTransaction,
    access: &lmdb_zero::ConstAccessor,
    block_height: u64,
    block_hash: &tari_common_types::types::FixedHash,
) -> Result<()> {
    
    // Index tables that may contain block-to-transaction mappings
    let index_tables = vec![
        ("kernel_excess_index", "May map kernel excess → block/position"),
        ("txos_hash_to_index", "May map output hash → index/position"),
        ("deleted_txo_hash_to_header_index", "May map spent output → block"),
        ("block_hashes", "May map block hash → height"),
        ("header_accumulated_data", "May contain transaction counts per block"),
    ];

    for (table_name, description) in index_tables {
        match Database::open(env, Some(table_name), &DatabaseOptions::defaults()) {
            Ok(db) => {
                println!("\n🔍 Index table: {} - {}", table_name, description);
                
                // Test if block height or hash can be used as keys
                let height_bytes = block_height.to_le_bytes();
                match access.get::<[u8], [u8]>(&db, &height_bytes) {
                    Ok(value) => {
                        println!("  ✅ Block height key found! Value size: {} bytes", value.len());
                        let preview = hex::encode(&value[0..std::cmp::min(32, value.len())]);
                        println!("     Value: {}...", preview);
                    },
                    Err(_) => {
                        // Try block hash
                        match access.get::<[u8], [u8]>(&db, block_hash.as_slice()) {
                            Ok(value) => {
                                println!("  ✅ Block hash key found! Value size: {} bytes", value.len());
                                let preview = hex::encode(&value[0..std::cmp::min(32, value.len())]);
                                println!("     Value: {}...", preview);
                            },
                            Err(_) => {
                                println!("  ❌ Neither block height nor hash found as keys");
                                
                                // Show sample key structure
                                match txn.cursor(&db) {
                                    Ok(mut cursor) => {
                                        if let Ok((sample_key, _)) = cursor.first::<[u8], [u8]>(access) {
                                            println!("     Sample key: {} bytes, hex: {}", 
                                                    sample_key.len(),
                                                    hex::encode(&sample_key[0..std::cmp::min(16, sample_key.len())]));
                                        }
                                    },
                                    Err(e) => {
                                        println!("     ⚠️ Error creating cursor: {:?}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Err(e) => {
                println!("\n❌ Index table {} not accessible: {:?}", table_name, e);
            }
        }
    }

    println!("\n💡 Investigation Summary:");
    println!("  • If index tables use block height/hash keys → Direct linking possible");
    println!("  • If not → May need to scan transaction tables or use MMR positions");
    println!("  • Check header_accumulated_data for transaction count metadata");
    println!("  • Index tables may contain arrays/lists of transaction component IDs");

    Ok(())
}