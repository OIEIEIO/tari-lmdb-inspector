// File: src/web_server.rs
// Web server with dashboard and WebSocket API - Block Height Monitoring

use anyhow::Result;
use axum::{
    extract::{ws::WebSocket, ws::Message, WebSocketUpgrade, State, Query},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, Router},
    Json,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json;
use serde::{Deserialize};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::{RwLock, broadcast};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use notify::{Watcher, RecursiveMode, Event};

use crate::data_models::{AppConfig, DashboardData, DatabaseStats, WebSocketMessage};
use crate::lmdb_reader::{read_lmdb_headers_with_filter, read_block_with_transactions, BlockFilter};

/// Query parameters for range search
#[derive(Deserialize)]
struct RangeQuery {
    start: u64,
    end: u64,
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub dashboard_data: Arc<RwLock<DashboardData>>,
    pub update_broadcaster: broadcast::Sender<DashboardData>,
}

/// Run the web server with block height monitoring
pub async fn run_web_mode(
    config: &AppConfig,
    bind: &str,
    port: u16,
    enable_cors: bool,
) -> Result<()> {
    // Create broadcast channel for dashboard updates
    let (update_tx, _update_rx) = broadcast::channel(100);
    
    let app_state = AppState {
        config: config.clone(),
        dashboard_data: Arc::new(RwLock::new(DashboardData::default())),
        update_broadcaster: update_tx,
    };

    // Update data initially
    update_dashboard_data(&app_state).await?;

    // Build our application with routes
    let mut app = Router::new()
        .route("/", get(dashboard_html))
        .route("/api/dashboard", get(get_dashboard_data))
        .route("/api/block/:height", get(get_block_detail))
        .route("/api/blocks/range", get(get_blocks_range))
        .route("/ws", get(websocket_handler))
        .with_state(app_state.clone());

    // Add CORS if enabled
    if enable_cors {
        app = app.layer(
            ServiceBuilder::new().layer(
                CorsLayer::new()
                    .allow_origin(tower_http::cors::Any)
                    .allow_methods(tower_http::cors::Any)
                    .allow_headers(tower_http::cors::Any),
            ),
        );
    }

    let addr: SocketAddr = format!("{}:{}", bind, port).parse()?;
    
    println!("🌐 Web dashboard available at: http://{}", addr);
    println!("🔌 WebSocket endpoint: ws://{}/ws", addr);
    println!("📊 API endpoints:");
    println!("   GET /api/dashboard - Dashboard data");
    println!("   GET /api/block/:height - Block details");
    println!("   GET /api/blocks/range?start=X&end=Y - Block ranges (max 1000)");
    println!("🔍 File system watcher: STARTING (monitoring LMDB changes)");
    
    // Start file system watcher (INSTEAD of polling)
    let watch_state = app_state.clone();
    tokio::spawn(async move {
        start_lmdb_file_watcher(watch_state).await;
    });

    // Start the server using axum 0.7 API
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// File system watcher for LMDB changes (zero CPU when idle)
async fn start_lmdb_file_watcher(state: AppState) {
    let database_path = state.config.database_path.clone();
    
    println!("📁 Watching: {}", database_path.display());
    println!("⚡ Zero-CPU monitoring - updates only when LMDB files change");
    
    // Create channel for file system events
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    
    // Setup file system watcher
    let watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        match res {
            Ok(event) => {
                // Only care about modify events on .mdb files
                if event.kind.is_modify() {
                    let has_mdb_files = event.paths.iter().any(|p| {
                        p.extension().map_or(false, |ext| ext == "mdb")
                    });
                    
                    if has_mdb_files {
                        if let Err(e) = tx.blocking_send(()) {
                            eprintln!("Failed to send file change event: {}", e);
                        }
                    }
                }
            }
            Err(e) => eprintln!("File watch error: {:?}", e),
        }
    });
    
    match watcher {
        Ok(mut watcher) => {
            // Watch the LMDB directory
            if let Err(e) = watcher.watch(&database_path, RecursiveMode::NonRecursive) {
                eprintln!("❌ Failed to start file watcher: {}", e);
                return;
            }
            
            println!("✅ File system watcher: ACTIVE");
            
            // Debouncing state
            let mut debounce_handle: Option<tokio::task::JoinHandle<()>> = None;
            
            // Listen for file change events
            while let Some(_) = rx.recv().await {
                // Cancel any pending update
                if let Some(handle) = debounce_handle.take() {
                    handle.abort();
                }
                
                // Schedule debounced update
                let update_state = state.clone();
                debounce_handle = Some(tokio::spawn(async move {
                    // Wait for writes to complete
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    
                    println!("📊 LMDB modified - updating dashboard...");
                    
                    if let Err(e) = update_dashboard_data(&update_state).await {
                        eprintln!("❌ Error updating dashboard: {}", e);
                    } else {
                        // Broadcast update to all WebSocket clients
                        let data = update_state.dashboard_data.read().await;
                        if let Err(e) = update_state.update_broadcaster.send(data.clone()) {
                            eprintln!("Warning: Failed to broadcast update: {}", e);
                        } else {
                            println!("✅ Dashboard updated (triggered by file change)");
                        }
                    }
                }));
            }
            
            // Keep watcher alive
            drop(watcher);
        }
        Err(e) => {
            eprintln!("❌ Failed to create file watcher: {}", e);
            eprintln!("💡 Falling back to manual refresh only");
        }
    }
}

/// Serve the main dashboard HTML page
async fn dashboard_html() -> Html<&'static str> {
    Html(include_str!("dashboard.html"))
}

/// Get dashboard data via REST API
async fn get_dashboard_data(State(state): State<AppState>) -> Json<DashboardData> {
    let data = state.dashboard_data.read().await;
    Json(data.clone())
}

/// Get block details via REST API
async fn get_block_detail(
    axum::extract::Path(height): axum::extract::Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match read_block_with_transactions(&state.config.database_path, height) {
        Ok(block_detail) => {
            let response = serde_json::json!({
                "height": block_detail.height,
                "hash": block_detail.hash,
                "header": {
                    "version": block_detail.header.version,
                    "timestamp": block_detail.header.timestamp,
                    "nonce": block_detail.header.nonce,
                    "previous_hash": block_detail.header.previous_hash,
                    "output_mr": block_detail.header.output_mr,
                    "kernel_mr": block_detail.header.kernel_mr,
                    "input_mr": block_detail.header.input_mr,
                    "total_kernel_offset": block_detail.header.total_kernel_offset,
                    "total_script_offset": block_detail.header.total_script_offset,
                    "pow_data_hash": block_detail.header.pow_data_hash,
                    "raw_header_length": block_detail.header.raw_header_length,
                    "pow_algorithm": block_detail.header.pow_algorithm
                },
                "transactions": {
                    "inputs": block_detail.transactions.inputs,
                    "outputs": block_detail.transactions.outputs,
                    "kernels": block_detail.transactions.kernels
                }
            });
            Ok(Json(response))
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

/// Get blocks in a range via REST API
async fn get_blocks_range(
    Query(params): Query<RangeQuery>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Validate range
    if params.start > params.end {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Limit range size to prevent huge queries
    let range_size = params.end - params.start + 1;
    if range_size > 1000 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    match read_lmdb_headers_with_filter(&state.config.database_path, "headers", BlockFilter::Range(params.start, params.end)) {
        Ok(blocks) => {
            let response = serde_json::json!({
                "start": params.start,
                "end": params.end,
                "total_found": blocks.len(),
                "blocks": blocks.iter().map(|block| {
                    serde_json::json!({
                        "height": block.height,
                        "hash": block.hash,
                        "timestamp": block.header.timestamp,
                        "previous_hash": block.header.previous_hash,
                        "output_mr": block.header.output_mr,
                        "kernel_mr": block.header.kernel_mr,
                        "input_mr": block.header.input_mr,
                        "total_kernel_offset": block.header.total_kernel_offset,
                        "total_script_offset": block.header.total_script_offset,
                        "pow_data_hash": block.header.pow_data_hash,
                        "raw_header_length": block.header.raw_header_length,
                        "pow_algorithm": block.header.pow_algorithm
                    })
                }).collect::<Vec<_>>()
            });
            Ok(Json(response))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// WebSocket connection handler
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

/// Handle individual WebSocket connections
async fn handle_websocket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Send initial dashboard data
    let dashboard_data = state.dashboard_data.read().await;
    let message = WebSocketMessage::DashboardData {
        data: dashboard_data.clone(),
    };
    
    if let Ok(json) = serde_json::to_string(&message) {
        if sender.send(Message::Text(json)).await.is_err() {
            return;
        }
    }
    drop(dashboard_data);

    // Subscribe to updates and spawn a task to handle them
    let mut update_receiver = state.update_broadcaster.subscribe();
    let (update_tx, mut update_rx) = tokio::sync::mpsc::channel(100);
    
    // Spawn task to forward broadcasts to this channel
    tokio::spawn(async move {
        while let Ok(dashboard_data) = update_receiver.recv().await {
            let message = WebSocketMessage::DashboardData { data: dashboard_data };
            if update_tx.send(message).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages and updates
    loop {
        tokio::select! {
            // Handle update messages
            update_msg = update_rx.recv() => {
                if let Some(message) = update_msg {
                    if let Ok(json) = serde_json::to_string(&message) {
                        if sender.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                }
            }
            
            // Handle incoming messages from client
            msg = receiver.next() => {
                if let Some(msg) = msg {
                    let msg = if let Ok(msg) = msg {
                        msg
                    } else {
                        break;
                    };

                    match msg {
                        Message::Text(text) => {
                            if let Ok(request) = serde_json::from_str::<WebSocketMessage>(&text) {
                                let response = handle_websocket_message(request, &state).await;
                                
                                if let Ok(json) = serde_json::to_string(&response) {
                                    if sender.send(Message::Text(json)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                } else {
                    break;
                }
            }
        }
    }
}

/// Handle individual WebSocket messages
async fn handle_websocket_message(
    message: WebSocketMessage,
    state: &AppState,
) -> WebSocketMessage {
    match message {
        WebSocketMessage::GetDashboard => {
            let data = state.dashboard_data.read().await;
            WebSocketMessage::DashboardData { data: data.clone() }
        }
        
        WebSocketMessage::GetBlockDetail { height } => {
            match read_block_with_transactions(&state.config.database_path, height) {
                Ok(block_detail) => {
                    let block_info = crate::data_models::BlockInfo {
                        height: block_detail.height,
                        hash: block_detail.hash.clone(),
                        timestamp: block_detail.header.timestamp,
                        transaction_count: block_detail.transactions.inputs.len() + 
                                         block_detail.transactions.outputs.len() + 
                                         block_detail.transactions.kernels.len(),
                        interval_seconds: None,
                        pow_algorithm: Some(block_detail.header.pow_algorithm.clone()),
                    };
                    
                    let transactions = crate::data_models::TransactionDetail {
                        inputs: block_detail.transactions.inputs.into_iter().map(|i| {
                            crate::data_models::InputInfo {
                                commitment: i.commitment,
                                input_type: i.input_type,
                                amount: None,
                            }
                        }).collect(),
                        outputs: block_detail.transactions.outputs.into_iter().map(|o| {
                            crate::data_models::OutputInfo {
                                commitment: o.commitment,
                                features: o.features,
                                amount: None,
                                script_type: o.script_type,
                            }
                        }).collect(),
                        kernels: block_detail.transactions.kernels.into_iter().map(|k| {
                            crate::data_models::KernelInfo {
                                excess: k.excess,
                                fee: k.fee,
                                lock_height: k.lock_height,
                            }
                        }).collect(),
                    };
                    
                    WebSocketMessage::BlockDetail {
                        height,
                        block_info,
                        transactions,
                    }
                }
                Err(e) => WebSocketMessage::Error {
                    message: format!("Failed to get block {}: {}", height, e),
                },
            }
        }
        
        WebSocketMessage::Ping => WebSocketMessage::Pong,
        
        _ => WebSocketMessage::Error {
            message: "Unsupported message type".to_string(),
        },
    }
}

/// Update dashboard data from LMDB (now only called when LMDB files change)
async fn update_dashboard_data(state: &AppState) -> Result<()> {
    println!("🔄 Reading LMDB data...");
    
    // Try to read real blocks and calculate real statistics
    let (recent_blocks, database_stats) = match read_lmdb_headers_with_filter(&state.config.database_path, "headers", BlockFilter::LastN(1000)) {
        Ok(blocks) => {
            println!("📊 Loaded {} blocks to cache for network analysis", blocks.len());
            
            // Convert and sort blocks by height (newest first)
            let mut recent_blocks: Vec<crate::data_models::BlockInfo> = blocks.into_iter().map(|block| {
                crate::data_models::BlockInfo {
                    height: block.height,
                    hash: block.hash,
                    timestamp: block.header.timestamp,
                    transaction_count: 0,
                    interval_seconds: None,
                    pow_algorithm: Some(block.header.pow_algorithm),
                }
            }).collect();
            
            // Sort by height descending (newest first)
            recent_blocks.sort_by(|a, b| b.height.cmp(&a.height));
            
            // Calculate intervals between consecutive blocks
            for i in 0..recent_blocks.len().saturating_sub(1) {
                let current = &recent_blocks[i];
                let previous = &recent_blocks[i + 1];
                
                if current.timestamp > previous.timestamp {
                    recent_blocks[i].interval_seconds = Some((current.timestamp - previous.timestamp) as i64);
                }
            }
            
            // Take top 200 for display (from 1000 available)
            let display_count = recent_blocks.len().min(200);
            recent_blocks.truncate(display_count);
            println!("🖥️  Displaying {} most recent blocks in dashboard", display_count);
            
            // Calculate REAL database statistics by counting actual LMDB entries
            let database_stats = calculate_real_database_stats(&state.config.database_path).await;
            
            (recent_blocks, database_stats)
        },
        Err(e) => {
            println!("⚠️  Could not read from LMDB ({}), using mock data", e);
            
            // Generate mock blocks for demo (200 blocks)
            let now = chrono::Utc::now().timestamp() as u64;
            let mock_blocks = (0..200).map(|i| {
                crate::data_models::BlockInfo {
                    height: 100000 - i,
                    hash: format!("0x{:064x}", 1000000 - i),
                    timestamp: now - (i * 120), // 2 minute intervals
                    transaction_count: 5 + (i % 3) as usize,
                    interval_seconds: if i < 199 { Some(120) } else { None },
                    pow_algorithm: Some("RandomXM".to_string()),
                }
            }).collect();
            
            let database_stats = DatabaseStats {
                utxos_count: 1_234_567,
                inputs_count: 987_654,
                kernels_count: 543_210,
                total_transactions: 543_210,
                total_io_records: 2_222_221,
            };
            
            (mock_blocks, database_stats)
        }
    };
    
    // Calculate network stats from the blocks
    let latest_height = recent_blocks.first().map(|b| b.height).unwrap_or(0);
    
    // Calculate average block time from intervals
    let valid_intervals: Vec<i64> = recent_blocks.iter()
        .filter_map(|b| b.interval_seconds)
        .filter(|&interval| interval > 0 && interval < 3600)
        .collect();
    
    let average_block_time = if !valid_intervals.is_empty() {
        valid_intervals.iter().sum::<i64>() / valid_intervals.len() as i64
    } else {
        120
    };
    
    let tps = if average_block_time > 0 {
        10.0 / average_block_time as f64 // Estimate 10 transactions per block
    } else {
        0.083 // ~1 transaction per 12 seconds
    };

    let network_stats = crate::data_models::NetworkStats {
        latest_block_height: latest_height,
        average_block_time,
        transactions_per_second: tps.max(0.001), // Minimum TPS
        utxo_set_size: database_stats.utxos_count,
    };

    // Update shared state
    let mut data = state.dashboard_data.write().await;
    data.database_stats = database_stats;
    data.recent_blocks = recent_blocks;
    data.network_stats = network_stats;
    data.last_updated = chrono::Utc::now().timestamp() as u64;
    
    println!("⚡ Full blockchain searchable via search/range queries");
    println!("✅ Dashboard ready - latest height: {}", latest_height);

    Ok(())
}

/// Calculate real database statistics by scanning LMDB
async fn calculate_real_database_stats(database_path: &std::path::Path) -> DatabaseStats {
    println!("🔍 Scanning LMDB for real statistics...");
    
    // Try to get real counts (this is expensive, so we do it occasionally)
    let (utxos_count, inputs_count, kernels_count) = tokio::task::spawn_blocking({
        let path = database_path.to_path_buf();
        move || {
            let mut utxos = 0;
            let mut inputs = 0; 
            let mut kernels = 0;
            
            // Try to count actual database entries
            if let Ok(mut builder) = lmdb_zero::EnvBuilder::new() {
                if builder.set_maxdbs(40).is_ok() {
                    if let Ok(env) = unsafe { builder.open(&path.to_string_lossy(), lmdb_zero::open::Flags::empty(), 0o600) } {
                        
                        // Count UTXOs
                        if let Ok(utxos_db) = lmdb_zero::Database::open(&env, Some("utxos"), &lmdb_zero::DatabaseOptions::defaults()) {
                            if let Ok(txn) = lmdb_zero::ReadTransaction::new(&env) {
                                utxos = count_db_entries_fast(&txn, &utxos_db);
                            }
                        }
                        
                        // Count Inputs  
                        if let Ok(inputs_db) = lmdb_zero::Database::open(&env, Some("inputs"), &lmdb_zero::DatabaseOptions::defaults()) {
                            if let Ok(txn) = lmdb_zero::ReadTransaction::new(&env) {
                                inputs = count_db_entries_fast(&txn, &inputs_db);
                            }
                        }
                        
                        // Count Kernels
                        if let Ok(kernels_db) = lmdb_zero::Database::open(&env, Some("kernels"), &lmdb_zero::DatabaseOptions::defaults()) {
                            if let Ok(txn) = lmdb_zero::ReadTransaction::new(&env) {
                                kernels = count_db_entries_fast(&txn, &kernels_db);
                            }
                        }
                    }
                }
            }
            
            (utxos, inputs, kernels)
        }
    }).await.unwrap_or((0, 0, 0));
    
    println!("📊 Database stats: UTXOs: {}, Inputs: {}, Kernels: {}", 
             utxos_count.to_string().as_str(), 
             inputs_count.to_string().as_str(), 
             kernels_count.to_string().as_str());
    
    DatabaseStats {
        utxos_count,
        inputs_count,
        kernels_count,
        total_transactions: kernels_count, // 1 kernel = 1 transaction
        total_io_records: utxos_count + inputs_count,
    }
}

/// Fast database entry counting without limits
fn count_db_entries_fast(txn: &lmdb_zero::ReadTransaction, db: &lmdb_zero::Database) -> usize {
    match txn.cursor(db) {
        Ok(mut cursor) => {
            let access = txn.access();
            if cursor.first::<[u8], [u8]>(&access).is_ok() {
                let mut count = 1;
                
                loop {
                    if cursor.next::<[u8], [u8]>(&access).is_err() {
                        break;
                    }
                    count += 1;
                    
                    // Show progress every 500k entries
                    if count % 500_000 == 0 {
                        print!("{}M.", count / 1_000_000);
                    }
                }
                
                println!(" {} total entries", count.to_string());
                count
            } else {
                0
            }
        },
        Err(_) => 0,
    }
}
