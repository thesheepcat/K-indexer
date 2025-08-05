use clap::Parser;

/// CLI arguments for K-indexer application
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Kaspa node address to connect to
    #[arg(short, long)]
    pub rusty_kaspa_address: String,

    /// Database path (optional, defaults to "k-indexer.db")
    #[arg(short, long, default_value = "k-indexer.db")]
    pub database_path: String,

    /// Web server bind address (optional, defaults to "0.0.0.0:3000")
    #[arg(short, long, default_value = "0.0.0.0:3000")]
    pub bind_address: String,
}