use crate::Args;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub workers: WorkerConfig,
    pub processing: ProcessingConfig,
    pub network: String,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub max_connections: usize,
}

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct ProcessingConfig {
    pub channel_name: String,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
}

impl AppConfig {
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
        // Validate network parameter
        let network = args.network.trim().to_string();
        if network != "testnet-10" && network != "mainnet" {
            panic!(
                "Invalid network type '{}'. Must be 'testnet-10' or 'mainnet'",
                network
            );
        }

        Self {
            database: DatabaseConfig {
                host: args
                    .db_host
                    .clone()
                    .unwrap_or_else(|| "localhost".to_string()),
                port: args.db_port.unwrap_or(5432),
                database: args
                    .db_name
                    .clone()
                    .unwrap_or_else(|| "your_database".to_string()),
                username: args
                    .db_user
                    .clone()
                    .unwrap_or_else(|| "your_user".to_string()),
                password: args
                    .db_password
                    .clone()
                    .unwrap_or_else(|| "your_password".to_string()),
                max_connections: args.db_max_connections.unwrap_or(10),
            },
            workers: WorkerConfig {
                count: args.workers.unwrap_or(4),
            },
            processing: ProcessingConfig {
                channel_name: args
                    .channel
                    .clone()
                    .unwrap_or_else(|| "transaction_channel".to_string()),
                retry_attempts: args.retry_attempts.unwrap_or(3),
                retry_delay_ms: args.retry_delay.unwrap_or(1000),
            },
            network,
        }
    }
}
