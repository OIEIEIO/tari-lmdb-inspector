// File: src/main.rs
// Version: 3.1.1 - Multi-interface Tari blockchain explorer with LMDB key structure investigation (FIXED CLI)
// Tree: tari-lmdb-inspector/src/main.rs

use std::path::PathBuf;
use clap::{Parser, Subcommand};
use anyhow::Result;

// Core modules for multi-interface functionality
mod lmdb_reader;
mod cli_interface;
mod tui_dashboard;
mod web_server;
mod data_models;

// New debugging module for LMDB key structure investigation
mod key_inspector;

use crate::data_models::AppConfig;

/// Command-line interface definition for the Tari LMDB Inspector
/// Supports multiple interface modes: CLI, TUI, Web, and debugging
#[derive(Parser)]
#[command(name = "tari-lmdb-inspector")]
#[command(about = "Multi-interface Tari blockchain explorer with TUI and Web dashboards")]
#[command(version = "3.1.1")]
pub struct Cli {
    /// Path to the Tari LMDB database directory
    /// Default: ~/.tari/mainnet/data/base_node/db
    #[arg(short, long, value_name = "DB_PATH")]
    pub database: PathBuf,

    /// Interface mode selection
    #[command(subcommand)]
    pub mode: InterfaceMode,
}

/// Available interface modes for the Tari LMDB Inspector
/// Each mode provides different visualization and interaction capabilities
#[derive(Subcommand)]
pub enum InterfaceMode {
    /// Classic CLI interface (original functionality)
    /// Provides command-line block and transaction viewing
    Cli {
        /// Show last N blocks
        #[arg(short, long, default_value = "3")]
        count: usize,
        
        /// Show specific block with transaction details
        #[arg(short, long)]
        detail: Option<u64>,
        
        /// Show blocks in range (format: start-end)
        #[arg(short, long)]
        range: Option<String>,
        
        /// Show specific block height
        #[arg(short, long)]
        block: Option<u64>,
    },
    
    /// Terminal UI dashboard (ratatui)
    /// Real-time blockchain monitoring with interactive interface
    Tui {
        /// Refresh interval in seconds
        #[arg(short, long, default_value = "5")]
        refresh: u64,
    },
    
    /// Web server with dashboard (axum + WebSocket)
    /// Browser-based dashboard with real-time updates
    Web {
        /// Server port
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Bind address
        #[arg(short, long, default_value = "127.0.0.1")]
        bind: String,
        
        /// Enable CORS for development
        #[arg(short, long)]
        cors: bool,
    },
    
    /// Investigate LMDB key structures (debugging tool)
    /// Helps understand how transaction data is stored and linked
    Inspect {
        /// Specific block height to investigate relationships
        #[arg(short = 'b', long)]
        block_height: Option<u64>,
        
        /// Show sample keys from all transaction tables
        #[arg(short = 'a', long)]
        all_tables: bool,
        
        /// Test multiple block heights to find patterns
        #[arg(short = 'p', long)]
        test_patterns: bool,
        
        /// Simple prefix test - check if block hash is used as key prefix
        #[arg(short = 's', long)]
        simple_test: bool,
        
        /// Thorough investigation - compare linking hash to actual transaction keys
        #[arg(short = 't', long)]
        thorough: bool,
    },
}

/// Main application entry point
/// Routes to appropriate interface mode based on CLI arguments
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Validate database path (but allow web mode to work with demo data)
    if !cli.database.exists() {
        match cli.mode {
            InterfaceMode::Web { .. } => {
                println!("⚠️  Database path does not exist: {:?}", cli.database);
                println!("🌐 Web mode will start with demo data");
            },
            InterfaceMode::Inspect { .. } => {
                println!("⚠️  Database path does not exist: {:?}", cli.database);
                println!("🔍 Inspector mode will show available investigation options");
            },
            _ => {
                anyhow::bail!("Database path does not exist: {:?}", cli.database);
            }
        }
    }
    
    // Create app configuration
    let config = AppConfig {
        database_path: cli.database,
    };
    
    // Route to appropriate interface based on selected mode
    match cli.mode {
        InterfaceMode::Cli { count, detail, range, block } => {
            println!("🔍 Tari LMDB Inspector - CLI Mode");
            cli_interface::run_cli_mode(&config, count, detail, range, block).await
        },
        
        InterfaceMode::Tui { refresh } => {
            println!("📊 Tari LMDB Inspector - Terminal Dashboard");
            tui_dashboard::run_tui_mode(&config, refresh).await
        },
        
        InterfaceMode::Web { port, bind, cors } => {
            println!("🌐 Tari LMDB Inspector - Web Server Mode");
            println!("Starting server at http://{}:{}", bind, port);
            web_server::run_web_mode(&config, &bind, port, cors).await
        },
        
        InterfaceMode::Inspect { block_height, all_tables, test_patterns, simple_test, thorough } => {
            println!("🔍 Tari LMDB Inspector - Key Structure Investigation");
            run_inspector_mode(&config, block_height, all_tables, test_patterns, simple_test, thorough).await
        },
    }
}

/// Run the LMDB key structure investigation mode
/// This debugging tool helps understand how Tari stores transaction data
/// 
/// # Arguments
/// * `config` - Application configuration with database path
/// * `block_height` - Optional specific block to investigate
/// * `all_tables` - Whether to show sample keys from all tables
/// * `test_patterns` - Whether to test multiple blocks for patterns
/// * `simple_test` - Whether to run simple prefix test
/// * `thorough` - Whether to run thorough key investigation
async fn run_inspector_mode(
    config: &AppConfig, 
    block_height: Option<u64>, 
    all_tables: bool, 
    test_patterns: bool,
    simple_test: bool,
    thorough: bool,
) -> Result<()> {
    let db_path = &config.database_path;
    
    println!("🚀 Starting LMDB Key Structure Investigation");
    println!("Database path: {:?}", db_path);
    println!("{}", "=".repeat(70));
    
    // Always check database availability first
    println!("📋 Checking database availability...");
    key_inspector::check_database_availability(db_path)?;
    
    // Execute investigation based on provided flags
    if thorough {
        let test_height = block_height.unwrap_or(64754);
        println!("\n🔍 Running thorough transaction key investigation for block {}...", test_height);
        key_inspector::investigate_transaction_keys_thoroughly(db_path, test_height)?;
        return Ok(());
    }
    
    if simple_test {
        let test_height = block_height.unwrap_or(64754);
        println!("\n🎯 Running simple prefix test for block {}...", test_height);
        key_inspector::test_block_hash_as_prefix(db_path, test_height)?;
        return Ok(());
    }
    
    if all_tables {
        println!("\n🔍 Inspecting all table key structures...");
        key_inspector::inspect_all_transaction_tables(db_path)?;
    }
    
    if let Some(height) = block_height {
        println!("\n🔗 Investigating block-to-transaction relationships for height {}...", height);
        key_inspector::investigate_block_to_transaction_links(db_path, height)?;
    }
    
    if test_patterns {
        println!("\n📊 Testing multiple blocks for key/linking patterns...");
        // Test the last few blocks to find patterns
        let test_heights = [64754, 64753, 64752]; // Adjust to current tip as needed
        for height in test_heights {
            println!("\n--- Testing Block {} ---", height);
            match key_inspector::investigate_block_to_transaction_links(db_path, height) {
                Ok(_) => println!("✅ Block {} investigation completed", height),
                Err(e) => println!("❌ Error investigating block {}: {}", height, e),
            }
        }
    }
    
    // If no specific options provided, run a comprehensive basic investigation
    if block_height.is_none() && !all_tables && !test_patterns && !simple_test && !thorough {
        println!("\n🚀 Running comprehensive basic investigation...");
        
        // Step 1: Inspect table structures
        println!("\n🔍 STEP 1: Inspecting table key structures...");
        key_inspector::inspect_all_transaction_tables(db_path)?;
        
        // Step 2: Test with a recent block
        println!("\n🔗 STEP 2: Testing block-to-transaction relationships...");
        key_inspector::investigate_block_to_transaction_links(db_path, 64754)?;
        
        // Step 3: Provide guidance for next steps
        println!("\n💡 INVESTIGATION COMPLETE");
        println!("Next steps:");
        println!("  • Use -b/--block-height <HEIGHT> to investigate a specific block");
        println!("  • Use -a/--all-tables to see detailed key structures");
        println!("  • Use -p/--test-patterns to analyze multiple blocks");
        println!("  • Use -s/--simple-test for simple prefix testing");
        println!("  • Use -t/--thorough for comprehensive key investigation");
        println!("  • Review output above to understand LMDB key strategies");
    }
    
    Ok(())
}