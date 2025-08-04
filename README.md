Tari LMDB Inspector
Tari LMDB Inspector is a powerful, multi-interface tool for exploring and debugging Tari blockchain data stored in LMDB databases. It offers four modes—CLI, TUI (terminal dashboard), Web (browser-based dashboard with WebSocket), and key structure inspection—making it an essential utility for developers, node operators, and blockchain enthusiasts analyzing Tari's blockchain data.
Table of Contents

Features
Installation
Prerequisites
Build from Source


Usage
CLI Mode
TUI Mode
Web Mode
Key Inspection Mode


Database Structure
For Node Operators
Contributing
License
Acknowledgments

Features

CLI Mode: Query blocks, transactions, and block ranges via a command-line interface.
TUI Mode: Interactive terminal dashboard for real-time blockchain monitoring using ratatui.
Web Mode: Browser-based dashboard with real-time WebSocket updates, powered by axum.
Key Inspection Mode: Analyze LMDB key structures to understand Tari's data storage and linking.
Flexible Data Access: Works with any valid Tari LMDB database, with fallback to demo data for Web and Inspect modes if the database is unavailable.

Installation
Prerequisites

Rust: Install the Rust toolchain (cargo, rustc) via rustup.
Tari LMDB Database: A valid Tari mainnet database (e.g., ~/.tari/mainnet/data/base_node/db). Web and Inspect modes can use demo data if no database is available.
Optional: Install jq for parsing JSON output in CLI mode (sudo apt install jq on Debian/Ubuntu or equivalent).

Build from Source
Clone the repository and build the project:
git clone https://github.com/OIEIEIO/tari-lmdb-inspector.git
cd tari-lmdb-inspector
cargo build --release

The executable will be located at target/release/tari-lmdb-inspector.
Usage
Run the tool with the -d or --database flag to specify the Tari LMDB database path (defaults to ~/.tari/mainnet/data/base_node/db). Select an interface mode using one of the subcommands: cli, tui, web, or inspect.
CLI Mode
View block and transaction details directly in the terminal.
# Show details for block 64754
cargo run --release -- -d ~/.tari/mainnet/data/base_node/db cli --detail 64754

# Show the last 3 blocks
cargo run --release -- -d ~/.tari/mainnet/data/base_node/db cli --count 3

# Show blocks in range 64750-64754
cargo run --release -- -d ~/.tari/mainnet/data/base_node/db cli --range 64750-64754

TUI Mode
Launch an interactive terminal dashboard for real-time blockchain monitoring.
cargo run --release -- -d ~/.tari/mainnet/data/base_node/db tui --refresh 5

Controls:

q or Esc: Quit the dashboard.
r: Force a data refresh.

Web Mode
Start a web server with a browser-based dashboard and real-time WebSocket updates.
cargo run --release -- -d ~/.tari/mainnet/data/base_node/db web --port 8080 --bind 127.0.0.1


Access the dashboard at http://localhost:8080.
API endpoints:
GET /api/block/<height>: Retrieve details for a specific block.curl -s http://localhost:8080/api/block/64754 | jq


GET /api/blocks/range?start=X&end=Y: Fetch blocks in a range (max 1000 blocks).
GET /api/dashboard: Get dashboard data.
ws://localhost:8080/ws: Connect to WebSocket for real-time updates.



Key Inspection Mode
Debug LMDB key structures to understand how Tari stores transaction data.
# Run a simple prefix test for block 64754
cargo run --release -- -d ~/.tari/mainnet/data/base_node/db inspect --simple-test --block-height 64754

# Run a thorough key investigation for block 64754
cargo run --release -- -d ~/.tari/mainnet/data/base_node/db inspect --thorough --block-height 64754

# Inspect key structures of all tables
cargo run --release -- -d ~/.tari/mainnet/data/base_node/db inspect --all-tables

# Test multiple blocks for key patterns
cargo run --release -- -d ~/.tari/mainnet/data/base_node/db inspect --test-patterns

Database Structure
The tool interacts with Tari’s LMDB database, which includes the following tables (based on a sample mainnet database):



Table Name
Entry Count



Main DB
34


block_hashes
64,823


deleted_txo_hash_to_header_index
3,357,980


header_accumulated_data
64,823


headers
64,823


inputs
3,357,980


jmt_node_data
30,360,201


jmt_unique_key_data
7,734,870


jmt_value_data
7,734,870


kernel_excess_index
1,416,107


kernel_excess_sig_index
1,416,107


kernel_mmr_size_index
64,823


kernels
1,416,107


metadata
9


mmr_peak_data
64,823


monero_seed_height
33


monero_seed_height_index
33


orphan_accumulated_data
469


orphan_chain_tips
461


orphan_parent_map_index
469


orphans
469


payref_to_output_index
4,377,143


reorgs
536


txos_hash_to_index
4,377,143


utxo_commitment_index
1,019,163


utxos
4,377,143


bad_blocks
0


contract_index
0


template_registrations
0


unique_id_index
0


utxo_smt
0


validator_nodes
0


validator_nodes_activation_queue
0


validator_nodes_exit
0


validator_nodes_mapping
0


This table helps node operators and developers understand the scale and organization of Tari’s blockchain data.
For Node Operators
Tari node operators can leverage tari-lmdb-inspector to:

Monitor blockchain health in real-time using the TUI or Web dashboard.
Inspect specific blocks or transactions to troubleshoot issues.
Analyze LMDB key structures to optimize node performance or debug data inconsistencies.
Verify database integrity by checking table sizes and relationships.

Contributing
We welcome contributions to improve tari-lmdb-inspector! To contribute:

Fork the repository.
Create a feature branch (git checkout -b feature/your-feature).
Commit your changes (git commit -m "Add your feature").
Push to the branch (git push origin feature/your-feature).
Open a pull request on GitHub.

Please ensure your code adheres to Rust best practices and includes tests where applicable.
License
This project is licensed under the MIT License.
Acknowledgments

Built with Rust, axum, ratatui, and lmdb-zero.
Inspired by the Tari community’s need for robust blockchain debugging tools.
