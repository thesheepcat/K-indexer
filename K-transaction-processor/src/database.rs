use crate::config::AppConfig;
use crate::migrations::{
    MIGRATION_001_ADD_TRANSACTION_TRIGGER, MIGRATION_002_CREATE_K_PROTOCOL_TABLES,
};
use anyhow::Result;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tracing::{error, info};

pub type DbPool = PgPool;

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

pub async fn verify_and_setup_database(pool: &DbPool) -> Result<()> {
    info!("Starting database verification and setup");

    if !check_trigger_exists(pool).await? {
        info!("Transaction trigger not found, running migration_001_add_transaction_trigger.sql");
        run_migration_001(pool).await?;
        verify_trigger_setup(pool).await?;
    } else {
        info!("Transaction trigger already exists, skipping migration 001");
    }

    if !check_k_tables_exist(pool).await? {
        info!("K protocol tables not found, running migration_002_create_k_protocol_tables.sql");
        run_migration_002(pool).await?;
        verify_tables_setup(pool).await?;
    } else {
        info!("K protocol tables already exist, skipping migration 002");
    }

    info!("Database verification and setup completed");
    Ok(())
}

async fn check_trigger_exists(pool: &DbPool) -> Result<bool> {
    let row = sqlx::query(
        "SELECT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname = 'transaction_notify_trigger')",
    )
    .fetch_one(pool)
    .await?;

    let exists: bool = row.get(0);
    Ok(exists)
}

async fn check_k_tables_exist(pool: &DbPool) -> Result<bool> {
    let tables = vec!["k_posts", "k_replies", "k_broadcasts", "k_votes"];

    for table in &tables {
        let row = sqlx::query(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
        )
        .bind(table)
        .fetch_one(pool)
        .await?;

        let exists: bool = row.get(0);
        if !exists {
            return Ok(false);
        }
    }

    Ok(true)
}

async fn run_migration_001(pool: &DbPool) -> Result<()> {
    execute_migration(pool, MIGRATION_001_ADD_TRANSACTION_TRIGGER).await
}

async fn run_migration_002(pool: &DbPool) -> Result<()> {
    execute_migration(pool, MIGRATION_002_CREATE_K_PROTOCOL_TABLES).await
}

async fn execute_migration(pool: &DbPool, migration_sql: &str) -> Result<()> {
    let mut tx = pool.begin().await?;

    // Parse SQL statements more carefully to handle multi-line statements
    let mut statements = Vec::new();
    let mut current_statement = String::new();

    for line in migration_sql.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }

        // Add line to current statement
        if !current_statement.is_empty() {
            current_statement.push(' ');
        }
        current_statement.push_str(trimmed);

        // Check if statement ends with semicolon
        if trimmed.ends_with(';') {
            // Remove the semicolon and add to statements list
            current_statement.pop();
            let stmt = current_statement.trim();
            if !stmt.is_empty() {
                statements.push(stmt.to_string());
            }
            current_statement.clear();
        }
    }

    // Handle any remaining statement without semicolon
    if !current_statement.trim().is_empty() {
        statements.push(current_statement.trim().to_string());
    }

    for (i, statement) in statements.iter().enumerate() {
        if !statement.is_empty() {
            tracing::debug!(
                "Executing statement {}: {}",
                i + 1,
                &statement[..std::cmp::min(100, statement.len())]
            );

            match sqlx::query(statement).execute(&mut *tx).await {
                Ok(_) => {
                    tracing::debug!("Statement {} executed successfully", i + 1);
                }
                Err(e) => {
                    tracing::error!("Failed to execute statement {}: {}", i + 1, e);
                    tracing::error!("Statement was: {}", statement);
                    return Err(e.into());
                }
            }
        }
    }

    tx.commit().await?;
    Ok(())
}

async fn verify_trigger_setup(pool: &DbPool) -> Result<()> {
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
        info!("✓ Transaction trigger setup verification PASSED");
        info!("  - notify_transaction() function created successfully");
        info!("  - transaction_notify_trigger trigger created successfully");
    } else {
        error!("✗ Transaction trigger setup verification FAILED");
        if !function_exists {
            error!("  - notify_transaction() function NOT found");
        }
        if !trigger_exists {
            error!("  - transaction_notify_trigger trigger NOT found");
        }
        return Err(anyhow::anyhow!(
            "Transaction trigger setup verification failed"
        ));
    }

    Ok(())
}

async fn verify_tables_setup(pool: &DbPool) -> Result<()> {
    let tables = vec!["k_posts", "k_replies", "k_broadcasts", "k_votes"];
    let mut all_verified = true;

    info!("Verifying K protocol tables setup");

    for table in &tables {
        let table_exists = sqlx::query(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
        )
        .bind(table)
        .fetch_one(pool)
        .await?
        .get::<bool, _>(0);

        if table_exists {
            info!("  ✓ Table '{}' created successfully", table);
        } else {
            error!("  ✗ Table '{}' NOT found", table);
            all_verified = false;
        }
    }

    let index_count = sqlx::query("SELECT COUNT(*) FROM pg_indexes WHERE indexname LIKE 'idx_k_%'")
        .fetch_one(pool)
        .await?
        .get::<i64, _>(0);

    info!("  ✓ {} K protocol indexes created", index_count);

    if all_verified {
        info!("✓ K protocol tables setup verification PASSED");
    } else {
        error!("✗ K protocol tables setup verification FAILED");
        return Err(anyhow::anyhow!(
            "K protocol tables setup verification failed"
        ));
    }

    Ok(())
}
