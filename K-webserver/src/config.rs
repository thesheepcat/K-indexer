
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
}

impl AppConfig {
    pub fn from_args(args: &crate::Args) -> Self {
        Self {
            database: DatabaseConfig {
                host: args.db_host.clone(),
                port: args.db_port,
                database: args.db_name.clone(),
                username: args.db_user.clone(),
                password: args.db_password.clone(),
                max_connections: args.db_max_connections,
            },
            server: ServerConfig {
                bind_address: args.bind_address.clone(),
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