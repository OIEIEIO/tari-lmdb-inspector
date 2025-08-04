// File: src/cli_view.rs
// Version: v1.2.0

use crate::model::BlockHeaderLite;

pub fn render_block_headers(headers: &[BlockHeaderLite]) {
    println!("┌──── Height ────┬────── Timestamp ────┬──── PoW ──┬──── Confirmations ──┐");
    for h in headers {
        println!(
            "│ {:>12} │ {:>18} │ {:>8} │ {:>18} │",
            h.height,
            h.timestamp,
            h.pow_algo,
            h.confirmations
        );
    }
    println!("└────────────────┴────────────────────┴────────────┴────────────────────┘");
}