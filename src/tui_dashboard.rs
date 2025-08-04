// File: src/tui_dashboard.rs
// Terminal UI dashboard using ratatui

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Table, Row, Cell},
    Frame, Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};
use tokio::time::sleep;

use crate::data_models::{AppConfig, DashboardData, DatabaseStats};
use crate::lmdb_reader::{read_lmdb_headers_with_filter, BlockFilter};

/// Application state for TUI
pub struct TuiApp {
    pub config: AppConfig,
    pub dashboard_data: DashboardData,
    pub refresh_interval: u64,
    pub last_update: Instant,
    pub should_quit: bool,
}

impl TuiApp {
    pub fn new(config: AppConfig, refresh_interval: u64) -> Self {
        Self {
            config,
            dashboard_data: DashboardData::default(),
            refresh_interval,
            last_update: Instant::now(),
            should_quit: false,
        }
    }

    /// Update dashboard data
    pub async fn update_data(&mut self) -> Result<()> {
        // Simulate data loading - replace with actual LMDB calls
        let blocks = read_lmdb_headers_with_filter(&self.config.database_path, "headers", BlockFilter::LastN(10))?;
        
        // Convert to our data format
        self.dashboard_data.recent_blocks = blocks.into_iter().map(|block| {
            crate::data_models::BlockInfo {
                height: block.height,
                hash: block.hash,
                timestamp: block.header.timestamp,
                transaction_count: 5, // Placeholder
                interval_seconds: None,
            }
        }).collect();

        // Mock database stats - replace with real data
        self.dashboard_data.database_stats = DatabaseStats {
            utxos_count: 4_340_719,
            inputs_count: 3_336_822,
            kernels_count: 1_404_641,
            total_transactions: 1_404_641,
            total_io_records: 7_677_541,
        };

        self.dashboard_data.last_updated = chrono::Utc::now().timestamp() as u64;
        self.last_update = Instant::now();
        
        Ok(())
    }

    /// Handle keyboard input
    pub fn handle_input(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('r') => {
                // Force refresh
                self.last_update = Instant::now() - Duration::from_secs(self.refresh_interval);
            }
            _ => {}
        }
    }
}

/// Run the TUI dashboard
pub async fn run_tui_mode(
    config: &AppConfig,
    refresh: u64,
) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = TuiApp::new(config.clone(), refresh);
    
    // Initial data load
    app.update_data().await?;

    // Main event loop
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();

    loop {
        // Draw UI
        terminal.draw(|f| ui(f, &app))?;

        // Handle events
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.handle_input(key.code);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        // Check if should quit
        if app.should_quit {
            break;
        }

        // Update data if needed
        if app.last_update.elapsed() >= Duration::from_secs(app.refresh_interval) {
            app.update_data().await?;
        }

        // Small async sleep to prevent busy waiting
        sleep(Duration::from_millis(100)).await;
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    println!("👋 Tari LMDB Inspector - Dashboard closed");
    
    Ok(())
}

/// Render the UI
fn ui(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(8),  // Database stats
            Constraint::Min(10),    // Recent blocks
            Constraint::Length(3),  // Footer
        ])
        .split(f.area());

    // Header
    render_header(f, chunks[0], app);
    
    // Database statistics
    render_database_stats(f, chunks[1], &app.dashboard_data.database_stats);
    
    // Recent blocks
    render_recent_blocks(f, chunks[2], &app.dashboard_data.recent_blocks);
    
    // Footer
    render_footer(f, chunks[3]);
}

/// Render header section
fn render_header(f: &mut Frame, area: Rect, app: &TuiApp) {
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("🔍 ", Style::default().fg(Color::Yellow)),
            Span::styled("Tari LMDB Inspector", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(" - Terminal Dashboard", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("Database: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:?}", app.config.database_path), Style::default().fg(Color::White)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title("Tari Blockchain Explorer"));
    
    f.render_widget(header, area);
}

/// Render database statistics
fn render_database_stats(f: &mut Frame, area: Rect, stats: &DatabaseStats) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)])
        .split(area);

    // UTXOs
    let utxos_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("💰 UTXOs"))
        .gauge_style(Style::default().fg(Color::Green))
        .percent(((stats.utxos_count as f64 / 5_000_000.0) * 100.0) as u16)
        .label(format!("{}", stats.utxos_count));
    f.render_widget(utxos_gauge, chunks[0]);

    // Inputs
    let inputs_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("📥 Inputs"))
        .gauge_style(Style::default().fg(Color::Blue))
        .percent(((stats.inputs_count as f64 / 4_000_000.0) * 100.0) as u16)
        .label(format!("{}", stats.inputs_count));
    f.render_widget(inputs_gauge, chunks[1]);

    // Kernels (Transactions)
    let kernels_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("⚡ Transactions"))
        .gauge_style(Style::default().fg(Color::Yellow))
        .percent(((stats.kernels_count as f64 / 2_000_000.0) * 100.0) as u16)
        .label(format!("{}", stats.kernels_count));
    f.render_widget(kernels_gauge, chunks[2]);
}

/// Render recent blocks
fn render_recent_blocks(f: &mut Frame, area: Rect, blocks: &[crate::data_models::BlockInfo]) {
    let header_cells = ["Height", "Hash", "Timestamp", "TXs"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = blocks.iter().map(|block| {
        let hash_short = if block.hash.len() > 16 {
            format!("{}...", &block.hash[..16])
        } else {
            block.hash.clone()
        };
        
        let timestamp = chrono::DateTime::from_timestamp(block.timestamp as i64, 0)
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "Invalid".to_string());

        Row::new(vec![
            Cell::from(block.height.to_string()),
            Cell::from(hash_short),
            Cell::from(timestamp),
            Cell::from(block.transaction_count.to_string()),
        ])
    });

    let widths = [
        Constraint::Length(8),
        Constraint::Length(20),
        Constraint::Length(12),
        Constraint::Length(6),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("📊 Recent Blocks"));

    f.render_widget(table, area);
}

/// Render footer
fn render_footer(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new("Press 'q' to quit, 'r' to refresh")
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    
    f.render_widget(footer, area);
}