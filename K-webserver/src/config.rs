#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
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
pub struct ServerConfig {
    pub bind_address: String,
    pub request_timeout: u64,
    pub rate_limit: u32,
}

impl AppConfig {
    pub fn from_args(args: &crate::Args, worker_threads: usize) -> Self {
        // Calculate default db connections as worker_threads * 3, with a minimum of 10
        let default_db_connections = std::cmp::max(worker_threads * 3, 10);
        let max_connections = args.db_max_connections.unwrap_or(default_db_connections);

        Self {
            database: DatabaseConfig {
                host: args.db_host.clone(),
                port: args.db_port,
                database: args.db_name.clone(),
                username: args.db_user.clone(),
                password: args.db_password.clone(),
                max_connections,
            },
            server: ServerConfig {
                bind_address: args.bind_address.clone(),
                request_timeout: args.request_timeout,
                rate_limit: args.rate_limit,
            },
        }
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
}
