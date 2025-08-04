// File: src/cli_interface.rs
// Rewritten CLI interface with improved organization

use anyhow::Result;
use chrono::{Utc, TimeZone};
use crate::data_models::AppConfig;
use crate::lmdb_reader::{read_lmdb_headers_with_filter, read_block_with_transactions, BlockFilter};

/// Execute CLI mode operations
pub async fn run_cli_mode(
    config: &AppConfig,
    count: usize,
    detail: Option<u64>,
    range: Option<String>,
    block: Option<u64>,
) -> Result<()> {
    match detail {
        Some(height) => show_block_detail(config, height).await,
        None => show_block_list(config, count, range, block).await,
    }
}

/// Display detailed information for a specific block
async fn show_block_detail(config: &AppConfig, height: u64) -> Result<()> {
    let block_detail = read_block_with_transactions(&config.database_path, height)?;
    print_block_detail(&block_detail);
    Ok(())
}

/// Display a list of blocks based on filter criteria
async fn show_block_list(
    config: &AppConfig, 
    count: usize, 
    range: Option<String>, 
    block: Option<u64>
) -> Result<()> {
    let filter = create_block_filter(count, range, block)?;
    let summaries = read_lmdb_headers_with_filter(&config.database_path, "headers", filter)?;

    if summaries.is_empty() {
        println!("No blocks found matching the criteria.");
        return Ok(());
    }

    print_blocks_table(&summaries);
    print_block_statistics(&summaries);
    Ok(())
}

/// Create appropriate block filter from CLI arguments
fn create_block_filter(count: usize, range: Option<String>, block: Option<u64>) -> Result<BlockFilter> {
    match (block, range) {
        (Some(height), None) => Ok(BlockFilter::Specific(height)),
        (None, Some(range_str)) => parse_range_filter(range_str),
        (None, None) => Ok(BlockFilter::LastN(count)),
        (Some(_), Some(_)) => anyhow::bail!("Cannot specify both --block and --range options"),
    }
}

/// Parse range string into BlockFilter
fn parse_range_filter(range_str: String) -> Result<BlockFilter> {
    let parts: Vec<&str> = range_str.split('-').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid range format. Use: start-end (e.g., 100-110)");
    }
    
    let start = parts[0].parse::<u64>()?;
    let end = parts[1].parse::<u64>()?;
    
    if start > end {
        anyhow::bail!("Start height must be <= end height");
    }
    
    Ok(BlockFilter::Range(start, end))
}

/// Print blocks in a formatted table
fn print_blocks_table(summaries: &[crate::lmdb_reader::BlockSummary]) {
    println!();
    print_table_header();
    print_table_separator();
    
    for (i, summary) in summaries.iter().enumerate() {
        let timestamp_str = format_timestamp(summary.header.timestamp);
        let interval_str = calculate_interval(summaries, i);
        
        println!("│ {:>8} │ {:<64} │ {:<23} │ {:>10} │", 
            summary.height,
            summary.hash,
            timestamp_str,
            interval_str
        );
    }
    
    print_table_footer();
}

/// Print table header
fn print_table_header() {
    println!("╭─{:─<8}─┬─{:─<64}─┬─{:─<23}─┬─{:─<10}─╮", "", "", "", "");
    println!("│ {:^8} │ {:^64} │ {:^23} │ {:^10} │", "Height", "Hash", "Timestamp", "Interval");
}

/// Print table separator
fn print_table_separator() {
    println!("├─{:─<8}─┼─{:─<64}─┼─{:─<23}─┼─{:─<10}─┤", "", "", "", "");
}

/// Print table footer
fn print_table_footer() {
    println!("╰─{:─<8}─┴─{:─<64}─┴─{:─<23}─┴─{:─<10}─╯", "", "", "", "");
}

/// Calculate time interval between consecutive blocks
fn calculate_interval(summaries: &[crate::lmdb_reader::BlockSummary], index: usize) -> String {
    if index == 0 {
        return "─".to_string();
    }
    
    let prev_ts = summaries[index - 1].header.timestamp as i64;
    let curr_ts = summaries[index].header.timestamp as i64;
    let diff = curr_ts - prev_ts;
    
    if diff > 0 {
        format_duration(diff)
    } else {
        "⚠ -time".to_string()
    }
}

/// Print block statistics summary
fn print_block_statistics(summaries: &[crate::lmdb_reader::BlockSummary]) {
    if summaries.len() <= 1 {
        return;
    }
    
    let intervals = calculate_valid_intervals(summaries);
    if intervals.is_empty() {
        return;
    }
    
    let avg_interval = intervals.iter().sum::<i64>() / intervals.len() as i64;
    let min_interval = *intervals.iter().min().unwrap();
    let max_interval = *intervals.iter().max().unwrap();
    
    println!();
    println!("📊 Block Intervals: avg {}, min {}, max {}", 
        format_duration(avg_interval),
        format_duration(min_interval), 
        format_duration(max_interval)
    );
}

/// Calculate valid time intervals between blocks
fn calculate_valid_intervals(summaries: &[crate::lmdb_reader::BlockSummary]) -> Vec<i64> {
    summaries.windows(2)
        .map(|pair| pair[1].header.timestamp as i64 - pair[0].header.timestamp as i64)
        .filter(|&diff| diff > 0)
        .collect()
}

/// Format Unix timestamp to human-readable string
fn format_timestamp(timestamp: u64) -> String {
    Utc.timestamp_opt(timestamp as i64, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| format!("Invalid: {}", timestamp))
}

/// Format duration in seconds to human-readable string
fn format_duration(seconds: i64) -> String {
    match seconds {
        s if s < 60 => format!("{}s", s),
        s if s < 3600 => {
            let mins = s / 60;
            let secs = s % 60;
            if secs == 0 { format!("{}m", mins) } else { format!("{}m {}s", mins, secs) }
        },
        s => {
            let hours = s / 3600;
            let mins = (s % 3600) / 60;
            format!("{}h {}m", hours, mins)
        }
    }
}

/// Print detailed block information
fn print_block_detail(block: &crate::lmdb_reader::BlockDetailSummary) {
    println!();
    println!("🔍 Block Detail View");
    
    print_block_header(block);
    print_transaction_summary(block);
    print_transaction_details(block);
    
    println!("╰─{:─<70}─╯", "");
}

/// Print block header information
fn print_block_header(block: &crate::lmdb_reader::BlockDetailSummary) {
    println!("╭─{:─<70}─╮", "");
    
    let hash_display = truncate_hash(&block.hash, 48);
    println!("│ Height: {:>8}  Hash: {:<48} │", block.height, hash_display);
    
    let timestamp_str = format_timestamp(block.header.timestamp);
    println!("│ Timestamp: {:<25} Nonce: {:>15} │", timestamp_str, block.header.nonce);
    
    let prev_hash_display = truncate_hash(&block.header.previous_hash, 50);
    println!("│ Previous Hash: {:<50} │", prev_hash_display);
    
    println!("├─{:─<70}─┤", "");
}

/// Print transaction summary
fn print_transaction_summary(block: &crate::lmdb_reader::BlockDetailSummary) {
    println!("│ 📊 Transaction Summary:                                             │");
    println!("│   Inputs:  {:>3}  Outputs: {:>3}  Kernels: {:>3}                        │",
        block.transactions.inputs.len(),
        block.transactions.outputs.len(), 
        block.transactions.kernels.len()
    );
    println!("├─{:─<70}─┤", "");
}

/// Print detailed transaction information
fn print_transaction_details(block: &crate::lmdb_reader::BlockDetailSummary) {
    print_inputs_section(&block.transactions.inputs);
    print_outputs_section(&block.transactions.outputs);
    print_kernels_section(&block.transactions.kernels);
}

/// Print transaction inputs section
fn print_inputs_section(inputs: &[crate::lmdb_reader::InputSummary]) {
    if inputs.is_empty() {
        return;
    }
    
    println!("│ 📥 Transaction Inputs:                                              │");
    for (i, input) in inputs.iter().take(3).enumerate() {
        let commitment_display = truncate_hash(&input.commitment, 20);
        println!("│   {}: {} [{}]                     │", 
            i + 1, commitment_display, input.input_type);
    }
    
    if inputs.len() > 3 {
        println!("│   ... and {} more inputs                                         │", 
            inputs.len() - 3);
    }
    println!("├─{:─<70}─┤", "");
}

/// Print transaction outputs section
fn print_outputs_section(outputs: &[crate::lmdb_reader::OutputSummary]) {
    if outputs.is_empty() {
        return;
    }
    
    println!("│ 📤 Transaction Outputs:                                             │");
    for (i, output) in outputs.iter().take(3).enumerate() {
        let commitment_display = truncate_hash(&output.commitment, 20);
        println!("│   {}: {} [{}]                     │", 
            i + 1, commitment_display, output.features);
    }
    
    if outputs.len() > 3 {
        println!("│   ... and {} more outputs                                        │", 
            outputs.len() - 3);
    }
    println!("├─{:─<70}─┤", "");
}

/// Print transaction kernels section
fn print_kernels_section(kernels: &[crate::lmdb_reader::KernelSummary]) {
    if kernels.is_empty() {
        return;
    }
    
    println!("│ ⚡ Transaction Kernels:                                             │");
    for (i, kernel) in kernels.iter().take(3).enumerate() {
        let excess_display = truncate_hash(&kernel.excess, 20);
        println!("│   {}: {} Fee: {} Lock: {}                │", 
            i + 1, excess_display, kernel.fee, kernel.lock_height);
    }
    
    if kernels.len() > 3 {
        println!("│   ... and {} more kernels                                        │", 
            kernels.len() - 3);
    }
}

/// Truncate hash string to specified length with ellipsis
fn truncate_hash(hash: &str, max_len: usize) -> String {
    if hash.len() > max_len {
        hash[..max_len].to_string()
    } else {
        hash.to_string()
    }
}
