use crate::config::AppConfig;
use anyhow::Result;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tracing::{error, info, warn};

pub type DbPool = PgPool;

// Schema version management
const SCHEMA_VERSION: i32 = 1;

/// K-transaction-processor Database Client
/// Similar to KaspaDbClient in Simply Kaspa Indexer
pub struct KDbClient {
    pool: DbPool,
}

impl KDbClient {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    /// Verify that transactions table exists (required for trigger)
    /// Loops with warning and 10-second wait if not found
    async fn verify_transactions_table_exists(&self) -> Result<()> {
        loop {
            let table_exists = sqlx::query(
                "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'transactions')"
            )
            .fetch_one(&self.pool)
            .await?
            .get::<bool, _>(0);

            if table_exists {
                info!("✓ Transactions table found - proceeding with K-transaction-processor schema setup");
                return Ok(());
            } else {
                warn!("⚠️  Transactions table not found - K-transaction-processor requires the main Kaspa indexer to be running first");
                warn!("   Waiting 10 seconds before checking again...");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        }
    }

    /// Drop existing schema (equivalent to KaspaDbClient::drop_schema)
    pub async fn drop_schema(&self) -> Result<()> {
        info!("Dropping existing schema");
        execute_ddl(SCHEMA_DOWN_SQL, &self.pool).await?;
        info!("Schema dropped successfully");
        Ok(())
    }


    /// Create or upgrade schema (equivalent to KaspaDbClient::create_schema)
    pub async fn create_schema(&self, upgrade_db: bool) -> Result<()> {
        info!("Starting schema creation/upgrade process");

        // Verify transactions table exists (required for trigger)
        self.verify_transactions_table_exists().await?;

        // Check current schema version
        let current_version = get_schema_version(&self.pool).await?;

        match current_version {
            Some(version) => {
                info!("Found existing schema version: {}", version);

                if version < SCHEMA_VERSION {
                    if upgrade_db {
                        warn!("Upgrading schema from v{} to v{}", version, SCHEMA_VERSION);

                        // Perform sequential upgrades
                        let mut current_version = version;

                        // v0 -> v1: Add all indexes
                        if current_version == 0 {
                            info!("Applying migration v0 -> v1 (adding indexes)");
                            execute_ddl(MIGRATION_V0_TO_V1_SQL, &self.pool).await?;
                            current_version = 1;
                            info!("Migration v0 -> v1 completed successfully");
                        }

                        info!("Schema upgrade completed successfully (final version: {})", current_version);
                    } else {
                        return Err(anyhow::anyhow!(
                            "Found outdated schema v{}. Set flag '--upgrade-db' to upgrade",
                            version
                        ));
                    }
                } else if version > SCHEMA_VERSION {
                    return Err(anyhow::anyhow!(
                        "Found newer & unsupported schema version {}. Current supported version is {}",
                        version, SCHEMA_VERSION
                    ));
                } else {
                    info!("Schema version {} is up to date", version);
                }
            }
            None => {
                info!("No existing schema found, creating fresh schema v{}", SCHEMA_VERSION);
                execute_ddl(SCHEMA_UP_SQL, &self.pool).await?;
                info!("Fresh schema creation completed successfully");
            }
        }

        // Verify schema setup
        verify_schema_setup(&self.pool).await?;

        info!("Schema creation/upgrade process completed");
        Ok(())
    }
}

// Embedded SQL migration files
const SCHEMA_UP_SQL: &str = include_str!("../migrations/schema/up.sql");
const SCHEMA_DOWN_SQL: &str = include_str!("../migrations/schema/down.sql");
const SCHEMA_V0_SQL: &str = include_str!("../migrations/schema/v0.sql");
const MIGRATION_V0_TO_V1_SQL: &str = include_str!("../migrations/schema/v0_to_v1.sql");

pub async fn create_pool(config: &AppConfig) -> Result<DbPool> {
    let connection_string = config.connection_string();

    let pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections as u32)
        .connect(&connection_string)
        .await?;

    Ok(pool)
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub transaction_id: String,
    pub payload: Option<String>,
    pub block_time: Option<i64>,
}

pub async fn fetch_transaction(
    pool: &DbPool,
    transaction_id_hex: &str,
) -> Result<Option<Transaction>> {
    // Convert hex string back to bytea for database query
    let transaction_id_bytes = hex::decode(transaction_id_hex)?;

    let row = sqlx::query(
        r#"
        SELECT 
            transaction_id,
            payload,
            block_time
        FROM transactions 
        WHERE transaction_id = $1
        "#,
    )
    .bind(&transaction_id_bytes)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        let transaction_id: Vec<u8> = row.get("transaction_id");
        let payload: Option<Vec<u8>> = row.get("payload");

        Ok(Some(Transaction {
            transaction_id: hex::encode(&transaction_id),
            payload: payload.map(|p| hex::encode(&p)),
            block_time: row.get("block_time"),
        }))
    } else {
        Ok(None)
    }
}


async fn get_schema_version(pool: &DbPool) -> Result<Option<i32>> {
    // Check if k_vars table exists
    let table_exists = sqlx::query(
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'k_vars')"
    )
    .fetch_one(pool)
    .await?
    .get::<bool, _>(0);

    if !table_exists {
        return Ok(None);
    }

    // Get schema version from k_vars table
    let result = sqlx::query(
        "SELECT value FROM k_vars WHERE key = 'schema_version'"
    )
    .fetch_optional(pool)
    .await?;

    match result {
        Some(row) => {
            let version_str: String = row.get("value");
            let version = version_str.parse::<i32>()
                .map_err(|_| anyhow::anyhow!("Invalid schema version format: {}", version_str))?;
            Ok(Some(version))
        }
        None => Ok(None),
    }
}

async fn execute_ddl(ddl: &str, pool: &DbPool) -> Result<()> {
    // Split DDL into individual statements and execute each one
    // This matches the Simply Kaspa Indexer implementation pattern
    for statement in ddl.split(";").filter(|stmt| !stmt.trim().is_empty()) {
        let trimmed_statement = statement.trim();

        // Skip empty statements and comments
        if trimmed_statement.is_empty() || trimmed_statement.starts_with("--") {
            continue;
        }

        // Execute the statement
        match sqlx::query(trimmed_statement).execute(pool).await {
            Ok(_) => {
                tracing::debug!("DDL statement executed successfully: {}",
                    &trimmed_statement[..std::cmp::min(100, trimmed_statement.len())]);
            }
            Err(e) => {
                tracing::error!("Failed to execute DDL statement: {}", e);
                tracing::error!("Statement was: {}", trimmed_statement);
                return Err(e.into());
            }
        }
    }
    Ok(())
}

async fn verify_schema_setup(pool: &DbPool) -> Result<()> {
    info!("Verifying schema setup");

    // Check k_vars table and schema version
    let version = get_schema_version(pool).await?;
    match version {
        Some(v) if v == SCHEMA_VERSION => {
            info!("  ✓ k_vars table and schema version {} verified", v);
        }
        Some(v) => {
            error!("  ✗ Incorrect schema version: expected {}, found {}", SCHEMA_VERSION, v);
            return Err(anyhow::anyhow!("Schema version mismatch"));
        }
        None => {
            error!("  ✗ k_vars table or schema_version not found");
            return Err(anyhow::anyhow!("Schema version not found"));
        }
    }

    // Check K protocol tables
    let tables = vec!["k_posts", "k_replies", "k_broadcasts", "k_votes", "k_mentions"];
    let mut all_verified = true;

    for table in &tables {
        let table_exists = sqlx::query(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
        )
        .bind(table)
        .fetch_one(pool)
        .await?
        .get::<bool, _>(0);

        if table_exists {
            info!("  ✓ Table '{}' verified", table);
        } else {
            error!("  ✗ Table '{}' NOT found", table);
            all_verified = false;
        }
    }

    // Check transaction trigger
    let function_exists = sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_proc WHERE proname = 'notify_transaction')")
        .fetch_one(pool)
        .await?
        .get::<bool, _>(0);

    let trigger_exists = sqlx::query(
        "SELECT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname = 'transaction_notify_trigger')",
    )
    .fetch_one(pool)
    .await?
    .get::<bool, _>(0);

    if function_exists && trigger_exists {
        info!("  ✓ Transaction notification system verified");
    } else {
        error!("  ✗ Transaction notification system verification failed");
        all_verified = false;
    }

    // Check indexes
    let index_count = sqlx::query("SELECT COUNT(*) FROM pg_indexes WHERE indexname LIKE 'idx_k_%'")
        .fetch_one(pool)
        .await?
        .get::<i64, _>(0);

    info!("  ✓ {} K protocol indexes verified", index_count);

    if all_verified {
        info!("✓ Schema setup verification PASSED");
    } else {
        error!("✗ Schema setup verification FAILED");
        return Err(anyhow::anyhow!("Schema setup verification failed"));
    }

    Ok(())
}
