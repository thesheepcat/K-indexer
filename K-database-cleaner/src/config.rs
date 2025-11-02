use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about = "K-database-cleaner", long_about = None)]
pub struct Args {
    #[arg(short = 'H', long, default_value = "localhost", help = "Database host")]
    pub db_host: String,

    #[arg(short = 'P', long, default_value = "5432", help = "Database port")]
    pub db_port: u16,

    #[arg(short = 'd', long, default_value = "kaspa", help = "Database name")]
    pub db_name: String,

    #[arg(
        short = 'U',
        long,
        default_value = "postgres",
        help = "Database username"
    )]
    pub db_user: String,

    #[arg(
        short = 'p',
        long,
        default_value = "postgres",
        help = "Database password"
    )]
    pub db_password: String,

    #[arg(
        short = 'm',
        long,
        default_value = "2",
        help = "Maximum database connections"
    )]
    pub db_max_connections: usize,

    #[arg(
        short = 'u',
        long = "user",
        help = "Public key of the user to whom this indexer is dedicated"
    )]
    pub user_pubkey: String,

    #[arg(
        short = 't',
        long = "purge-interval",
        default_value = "600",
        help = "Interval in seconds between purge operations"
    )]
    pub purge_interval: u64,

    #[arg(
        short = 'r',
        long = "data-retention",
        default_value = "72",
        help = "Data retention time in hours for non-followed users' content"
    )]
    pub data_retention_hours: u64,
}

pub struct AppConfig {
    pub database: DatabaseConfig,
    pub user_pubkey: String,
    pub purge_interval: u64,
    pub data_retention_hours: u64,
}

pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    pub max_connections: usize,
}

impl AppConfig {
    pub fn from_args(args: &Args) -> Self {
        Self {
            database: DatabaseConfig {
                host: args.db_host.clone(),
                port: args.db_port,
                database: args.db_name.clone(),
                user: args.db_user.clone(),
                password: args.db_password.clone(),
                max_connections: args.db_max_connections,
            },
            user_pubkey: args.user_pubkey.clone(),
            purge_interval: args.purge_interval,
            data_retention_hours: args.data_retention_hours,
        }
    }

    pub fn connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.database.user,
            self.database.password,
            self.database.host,
            self.database.port,
            self.database.database
        )
    }
}
