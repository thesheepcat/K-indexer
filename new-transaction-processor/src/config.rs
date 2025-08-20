use serde::{Deserialize, Serialize};
use std::fs;
use anyhow::Result;
use crate::Args;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub workers: WorkerConfig,
    pub processing: ProcessingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub max_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    pub channel_name: String,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
}

impl AppConfig {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn connection_string(&self) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.database.username,
            self.database.password,
            self.database.host,
            self.database.port,
            self.database.database
        )
    }

    pub fn from_args(args: &Args) -> Self {
        Self {
            database: DatabaseConfig {
                host: args.db_host.clone().unwrap_or_else(|| "localhost".to_string()),
                port: args.db_port.unwrap_or(5432),
                database: args.db_name.clone().unwrap_or_else(|| "your_database".to_string()),
                username: args.db_user.clone().unwrap_or_else(|| "your_user".to_string()),
                password: args.db_password.clone().unwrap_or_else(|| "your_password".to_string()),
                max_connections: args.db_max_connections.unwrap_or(10),
            },
            workers: WorkerConfig {
                count: args.workers.unwrap_or(4),
            },
            processing: ProcessingConfig {
                channel_name: args.channel.clone().unwrap_or_else(|| "transaction_channel".to_string()),
                retry_attempts: args.retry_attempts.unwrap_or(3),
                retry_delay_ms: args.retry_delay.unwrap_or(1000),
            },
        }
    }

    pub fn apply_args(&mut self, args: &Args) {
        if let Some(ref host) = args.db_host {
            self.database.host = host.clone();
        }
        if let Some(port) = args.db_port {
            self.database.port = port;
        }
        if let Some(ref database) = args.db_name {
            self.database.database = database.clone();
        }
        if let Some(ref username) = args.db_user {
            self.database.username = username.clone();
        }
        if let Some(ref password) = args.db_password {
            self.database.password = password.clone();
        }
        if let Some(max_connections) = args.db_max_connections {
            self.database.max_connections = max_connections;
        }
        if let Some(workers) = args.workers {
            self.workers.count = workers;
        }
        if let Some(ref channel) = args.channel {
            self.processing.channel_name = channel.clone();
        }
        if let Some(retry_attempts) = args.retry_attempts {
            self.processing.retry_attempts = retry_attempts;
        }
        if let Some(retry_delay) = args.retry_delay {
            self.processing.retry_delay_ms = retry_delay;
        }
    }
}