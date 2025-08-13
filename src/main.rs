#![allow(warnings)]

use std::sync::Arc;
use clap::Parser;
use workflow_core::channel::oneshot;
use workflow_log::prelude::*;
use kaspa_wrpc_client::prelude::*;
use kaspa_wrpc_client::result::Result;
use chrono::{DateTime, Utc};

// Internal modules
mod api_handlers;
mod cli;
mod database;
mod kaspa_connection;
mod models;
mod notification_handler;
mod transaction_processor;
mod web_server;

use web_server::AppState;
use cli::Args;
use database::DatabaseManager;
use kaspa_connection::KaspaConnection;
use web_server::WebServer;

// Custom timestamp sink for adding timestamps to log messages
pub struct TimestampSink;

impl workflow_log::Sink for TimestampSink {
    fn write(&self, _target: Option<&str>, level: workflow_log::Level, args: &std::fmt::Arguments<'_>) -> bool {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        let level_str = match level {
            workflow_log::Level::Trace => "TRACE",
            workflow_log::Level::Debug => "DEBUG", 
            workflow_log::Level::Info => "INFO",
            workflow_log::Level::Warn => "WARN",
            workflow_log::Level::Error => "ERROR",
        };
        
        println!("[{}] [{}] {}", timestamp, level_str, args);
        true // Return true to indicate we handled the message (prevents further processing)
    }
}



#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Initialize timestamp logging (disable default logging first)
    workflow_log::pipe(None); // Disable default console output
    let timestamp_sink = Arc::new(TimestampSink);
    workflow_log::pipe(Some(timestamp_sink));

    // Initialize database
    let db_manager = DatabaseManager::new(&args.database_path)
        .expect("Failed to initialize database");
    let k_posts_collection = db_manager.get_k_posts_collection();
    let k_replies_collection = db_manager.get_k_replies_collection();
    let k_broadcasts_collection = db_manager.get_k_broadcasts_collection();
    let k_votes_collection = db_manager.get_k_votes_collection();

    // Build Kaspa connection URL
    let rusty_kaspa_address_prefix = "ws://";
    let complete_rusty_kaspa_address = format!("{}{}", rusty_kaspa_address_prefix, args.rusty_kaspa_address);

    // Initialize Kaspa connection
    let kaspa_connection = KaspaConnection::try_new(
        NetworkId::with_suffix(NetworkType::Testnet, 10),
        Some(complete_rusty_kaspa_address),
        k_posts_collection,
        k_replies_collection,
        k_broadcasts_collection,
        k_votes_collection,
    )?;

    // Start Kaspa connection
    kaspa_connection.start().await?;

    // Initialize web server  
    let db_manager_arc = Arc::new(db_manager);
    let web_server = WebServer::new(db_manager_arc);

    // Setup shutdown handler
    let (shutdown_sender, shutdown_receiver) = oneshot::<()>();
    let kaspa_connection_clone = kaspa_connection.clone();

    ctrlc::set_handler(move || {
        log_info!("^SIGTERM - shutting down...");
        shutdown_sender
            .try_send(())
            .expect("Error sending shutdown signal...");
    })
    .expect("Unable to set the Ctrl+C signal handler");

    // Run web server with graceful shutdown
    tokio::select! {
        result = web_server.serve(&args.bind_address) => {
            if let Err(err) = result {
                log_error!("Web server error: {}", err);
            }
        }
        _ = shutdown_receiver.recv() => {
            log_info!("Shutdown signal received, stopping services...");
        }
    }

    // Stop Kaspa connection
    kaspa_connection_clone.stop().await?;
    log_info!("Application shutdown complete");
    Ok(())
}