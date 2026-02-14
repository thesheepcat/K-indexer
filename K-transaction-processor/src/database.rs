use crate::config::AppConfig;
use anyhow::Result;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use tracing::{error, info, warn};

pub type DbPool = PgPool;

// Schema version management
const SCHEMA_VERSION: i32 = 2;

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
                info!(
                    "✓ Transactions table found - proceeding with K-transaction-processor schema setup"
                );
                return Ok(());
            } else {
                warn!(
                    "⚠️  Transactions table not found - K-transaction-processor requires the main Kaspa indexer to be running first"
                );
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

    /// Set or verify network type in k_vars table
    pub async fn set_and_verify_network(&self, network: &str) -> Result<()> {
        info!("Setting and verifying network type: {}", network);

        // Check if network is already set in k_vars
        let existing_network = sqlx::query("SELECT value FROM k_vars WHERE key = 'network'")
            .fetch_optional(&self.pool)
            .await?;

        match existing_network {
            Some(row) => {
                let stored_network: String = row.get("value");
                if stored_network != network {
                    return Err(anyhow::anyhow!(
                        "Network mismatch! Database is configured for '{}' but K-transaction-processor is set to '{}'. \
                        This could lead to data corruption. Please use the correct network parameter or initialize a new database.",
                        stored_network,
                        network
                    ));
                } else {
                    info!("✓ Network type verified: {}", network);
                }
            }
            None => {
                // Insert network type for the first time
                info!("Setting network type in database: {}", network);
                sqlx::query("INSERT INTO k_vars (key, value) VALUES ('network', $1)")
                    .bind(network)
                    .execute(&self.pool)
                    .await?;
                info!("✓ Network type set to: {}", network);
            }
        }

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

                        // v0 -> v1: Add all indexes, constraints, and extensions
                        if current_version == 0 {
                            info!("Applying migration v0 -> v1 (indexes, constraints, extensions)");
                            execute_ddl(MIGRATION_V0_TO_V1_SQL, &self.pool).await?;
                            current_version = 1;
                            info!("Migration v0 -> v1 completed successfully");
                        }

                        // v1 -> v2: Add hashtags table and indexes
                        if current_version == 1 {
                            info!("Applying migration v1 -> v2 (hashtags support)");
                            execute_ddl(MIGRATION_V1_TO_V2_SQL, &self.pool).await?;
                            current_version = 2;
                            info!("Migration v1 -> v2 completed successfully");
                        }

                        info!(
                            "Schema upgrade completed successfully (final version: {})",
                            current_version
                        );
                    } else {
                        return Err(anyhow::anyhow!(
                            "Found outdated schema v{}. Set flag '--upgrade-db' to upgrade",
                            version
                        ));
                    }
                } else if version > SCHEMA_VERSION {
                    return Err(anyhow::anyhow!(
                        "Found newer & unsupported schema version {}. Current supported version is {}",
                        version,
                        SCHEMA_VERSION
                    ));
                } else {
                    info!("Schema version {} is up to date", version);
                }
            }
            None => {
                info!(
                    "No existing schema found, creating fresh schema v{}",
                    SCHEMA_VERSION
                );
                execute_ddl(SCHEMA_UP_SQL, &self.pool).await?;

                // Create the notification function and trigger separately to avoid parsing issues
                self.create_notification_system().await?;

                info!("Fresh schema creation completed successfully");
            }
        }

        // Verify schema setup
        verify_schema_setup(&self.pool).await?;

        info!("Schema creation/upgrade process completed");
        Ok(())
    }

    /// Create the notification function and trigger separately to avoid DDL parsing issues
    async fn create_notification_system(&self) -> Result<()> {
        info!("Creating notification function and trigger");

        // Create the function using dollar quoting
        sqlx::query(
            r#"
            CREATE OR REPLACE FUNCTION notify_transaction() RETURNS TRIGGER AS $$
            BEGIN
                IF substr(encode(NEW.payload, 'hex'), 1, 8) = '6b3a313a' THEN
                    PERFORM pg_notify('transaction_channel', encode(NEW.transaction_id, 'hex'));
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql
        "#,
        )
        .execute(&self.pool)
        .await?;

        // Create the trigger
        sqlx::query(
            r#"
            CREATE TRIGGER transaction_notify_trigger
            AFTER INSERT ON transactions
            FOR EACH ROW EXECUTE FUNCTION notify_transaction()
        "#,
        )
        .execute(&self.pool)
        .await?;

        info!("Notification system created successfully");
        Ok(())
    }
}

// Embedded SQL migration files
const SCHEMA_UP_SQL: &str = include_str!("migrations/schema/up.sql");
const SCHEMA_DOWN_SQL: &str = include_str!("migrations/schema/down.sql");
const MIGRATION_V0_TO_V1_SQL: &str = include_str!("migrations/schema/v0_to_v1.sql");
const MIGRATION_V1_TO_V2_SQL: &str = include_str!("migrations/schema/v1_to_v2.sql");

pub async fn create_pool(config: &AppConfig) -> Result<DbPool> {
    let connection_string = config.connection_string();

    loop {
        match PgPoolOptions::new()
            .max_connections(config.database.max_connections as u32)
            .connect(&connection_string)
            .await
        {
            Ok(pool) => {
                // Test the pool connection
                match sqlx::query("SELECT 1").fetch_one(&pool).await {
                    Ok(_) => {
                        info!("Database connection pool created and tested successfully");
                        return Ok(pool);
                    }
                    Err(e) => {
                        warn!(
                            "Database connection pool created but test query failed: {}",
                            e
                        );
                        warn!("Retrying in 10 seconds...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create database connection pool: {}", e);
                warn!("Retrying in 10 seconds...");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        }
    }
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
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'k_vars')",
    )
    .fetch_one(pool)
    .await?
    .get::<bool, _>(0);

    if !table_exists {
        return Ok(None);
    }

    // Get schema version from k_vars table
    let result = sqlx::query("SELECT value FROM k_vars WHERE key = 'schema_version'")
        .fetch_optional(pool)
        .await?;

    match result {
        Some(row) => {
            let version_str: String = row.get("value");
            let version = version_str
                .parse::<i32>()
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
        sqlx::query(statement).execute(pool).await?;
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
            error!(
                "  ✗ Incorrect schema version: expected {}, found {}",
                SCHEMA_VERSION, v
            );
            return Err(anyhow::anyhow!("Schema version mismatch"));
        }
        None => {
            error!("  ✗ k_vars table or schema_version not found");
            return Err(anyhow::anyhow!("Schema version not found"));
        }
    }

    // Check K protocol tables
    let tables = vec![
        "k_contents",
        "k_broadcasts",
        "k_votes",
        "k_mentions",
        "k_blocks",
        "k_follows",
        "k_hashtags",
    ];
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
    let function_exists =
        sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_proc WHERE proname = 'notify_transaction')")
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

    // Explicit verification of all 37 expected K protocol indexes
    let expected_indexes = vec![
        // k_broadcasts indexes
        "idx_k_broadcasts_transaction_id",
        "idx_k_broadcasts_sender_pubkey",
        "idx_k_broadcasts_block_time",
        // k_votes indexes
        "idx_k_votes_transaction_id",
        "idx_k_votes_sender_pubkey",
        "idx_k_votes_sender_signature_unique",
        "idx_k_votes_post_id",
        "idx_k_votes_vote",
        "idx_k_votes_block_time",
        "idx_k_votes_post_id_sender",
        // k_mentions indexes
        "idx_k_mentions_comprehensive",
        "idx_k_mentions_content_id",
        "idx_k_mentions_mentioned_pubkey",
        // k_blocks indexes
        "idx_k_blocks_sender_signature_unique",
        "idx_k_blocks_sender_blocked_user_unique",
        "idx_k_blocks_sender_pubkey",
        "idx_k_blocks_blocked_user_pubkey",
        "idx_k_blocks_block_time",
        // k_contents indexes
        "idx_k_contents_transaction_id",
        "idx_k_contents_sender_signature_unique",
        "idx_k_contents_sender_pubkey",
        "idx_k_contents_block_time",
        "idx_k_contents_replies",
        "idx_k_contents_reposts",
        "idx_k_contents_quotes",
        "idx_k_contents_feed_optimized",
        "idx_k_contents_content_type",
        "idx_k_contents_sender_content_type",
        // k_follows indexes
        "idx_k_follows_sender_signature_unique",
        "idx_k_follows_sender_followed_user_unique",
        "idx_k_follows_followed_user_pubkey",
        "idx_k_follows_sender_pubkey",
        "idx_k_follows_block_time",
        // k_hashtags indexes
        "idx_k_hashtags_by_hashtag_time",
        "idx_k_hashtags_pattern",
        "idx_k_hashtags_trending",
        "idx_k_hashtags_by_hashtag_sender",
    ];

    let mut missing_indexes = Vec::new();

    for index_name in &expected_indexes {
        let index_exists =
            sqlx::query("SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE indexname = $1)")
                .bind(index_name)
                .fetch_one(pool)
                .await?
                .get::<bool, _>(0);

        if index_exists {
            info!("  ✓ Index '{}' verified", index_name);
        } else {
            error!("  ✗ Index '{}' NOT found", index_name);
            missing_indexes.push(index_name);
            all_verified = false;
        }
    }

    // Verify total count matches expected (37 indexes)
    let index_count = sqlx::query("SELECT COUNT(*) FROM pg_indexes WHERE indexname LIKE 'idx_k_%'")
        .fetch_one(pool)
        .await?
        .get::<i64, _>(0);

    if index_count == 37 {
        info!(
            "  ✓ Expected 37 K protocol indexes verified (found {})",
            index_count
        );
    } else {
        error!("  ✗ Expected 37 K protocol indexes, found {}", index_count);
        all_verified = false;
    }

    if !missing_indexes.is_empty() {
        error!("  ✗ Missing indexes: {:?}", missing_indexes);
    }

    // Verify k_contents table (v4+)
    if version.unwrap_or(0) >= 4 {
        info!("Verifying k_contents table");

        let k_contents_count: i64 = sqlx::query("SELECT COUNT(*) FROM k_contents")
            .fetch_one(pool)
            .await?
            .get(0);

        info!("  k_contents records: {}", k_contents_count);
        info!("  ✓ k_contents is the unified content table (k_posts and k_replies removed in v6)");
    }

    if all_verified {
        info!("✓ Schema setup verification PASSED");
    } else {
        error!("✗ Schema setup verification FAILED");
        return Err(anyhow::anyhow!("Schema setup verification failed"));
    }

    Ok(())
}
