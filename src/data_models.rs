// File: src/data_models.rs
// Shared data structures and models for all interfaces

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_path: PathBuf,
}

/// Real-time dashboard data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub database_stats: DatabaseStats,
    pub recent_blocks: Vec<BlockInfo>,
    pub network_stats: NetworkStats,
    pub last_updated: u64, // Unix timestamp
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub utxos_count: usize,
    pub inputs_count: usize,
    pub kernels_count: usize,
    pub total_transactions: usize,
    pub total_io_records: usize,
}

/// Block information for dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub height: u64,
    pub hash: String,
    pub timestamp: u64,
    pub transaction_count: usize,
    pub interval_seconds: Option<i64>,
}

/// Network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub latest_block_height: u64,
    pub average_block_time: i64,
    pub transactions_per_second: f64,
    pub utxo_set_size: usize,
}

/// Transaction details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetail {
    pub inputs: Vec<InputInfo>,
    pub outputs: Vec<OutputInfo>,
    pub kernels: Vec<KernelInfo>,
}

/// Input information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputInfo {
    pub commitment: String,
    pub input_type: String,
    pub amount: Option<u64>,
}

/// Output information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputInfo {
    pub commitment: String,
    pub features: String,
    pub amount: Option<u64>,
    pub script_type: String,
}

/// Kernel information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelInfo {
    pub excess: String,
    pub fee: u64,
    pub lock_height: u64,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    /// Request dashboard data
    GetDashboard,
    
    /// Dashboard data response
    DashboardData { data: DashboardData },
    
    /// Request block details
    GetBlockDetail { height: u64 },
    
    /// Block detail response
    BlockDetail { 
        height: u64,
        block_info: BlockInfo,
        transactions: TransactionDetail 
    },
    
    /// Error response
    Error { message: String },
    
    /// Ping/Pong for connection health
    Ping,
    Pong,
}

impl Default for DashboardData {
    fn default() -> Self {
        Self {
            database_stats: DatabaseStats::default(),
            recent_blocks: Vec::new(),
            network_stats: NetworkStats::default(),
            last_updated: 0,
        }
    }
}

impl Default for DatabaseStats {
    fn default() -> Self {
        Self {
            utxos_count: 0,
            inputs_count: 0,
            kernels_count: 0,
            total_transactions: 0,
            total_io_records: 0,
        }
    }
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            latest_block_height: 0,
            average_block_time: 0,
            transactions_per_second: 0.0,
            utxo_set_size: 0,
        }
    }
}