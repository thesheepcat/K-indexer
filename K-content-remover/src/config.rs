use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about = "K-content-remover - Remove all content created by a specific user", long_about = None)]
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
        short = 't',
        long = "target-user",
        help = "Public key (hex string) of the user whose content should be removed"
    )]
    pub target_user_pubkey: String,

    #[arg(
        long = "dry-run",
        help = "Preview what would be deleted without actually deleting anything"
    )]
    pub dry_run: bool,

    #[arg(
        short = 'y',
        long = "yes",
        help = "Skip confirmation prompt and proceed with deletion"
    )]
    pub skip_confirmation: bool,
}

pub struct AppConfig {
    pub database: DatabaseConfig,
    pub target_user_pubkey: String,
    pub dry_run: bool,
    pub skip_confirmation: bool,
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
            target_user_pubkey: args.target_user_pubkey.clone(),
            dry_run: args.dry_run,
            skip_confirmation: args.skip_confirmation,
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
