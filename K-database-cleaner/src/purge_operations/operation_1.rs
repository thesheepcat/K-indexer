use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::info;

/// Purge Operation 1: Remove all records where sender_pubkey is not the user's pubkey
/// from k_blocks and k_follows tables
pub async fn execute(pool: &PgPool, user_pubkey: &[u8]) -> Result<()> {
    info!("Starting purge operation 1: Cleaning k_blocks and k_follows tables");

    // Single transaction with CTE to delete from both tables and count results
    let mut tx = pool.begin().await?;

    let result = sqlx::query(
        r#"
        WITH deleted_blocks AS (
            DELETE FROM k_blocks
            WHERE sender_pubkey != $1
            RETURNING id
        ),
        deleted_follows AS (
            DELETE FROM k_follows
            WHERE sender_pubkey != $1
            RETURNING id
        )
        SELECT
            (SELECT COUNT(*) FROM deleted_blocks) as blocks_count,
            (SELECT COUNT(*) FROM deleted_follows) as follows_count
        "#,
    )
    .bind(user_pubkey)
    .fetch_one(&mut *tx)
    .await?;

    let k_blocks_deleted: i64 = result.get("blocks_count");
    let k_follows_deleted: i64 = result.get("follows_count");

    tx.commit().await?;

    info!(
        "✓ Purge operation 1: Deleted {} records from k_blocks table",
        k_blocks_deleted
    );
    info!(
        "✓ Purge operation 1: Deleted {} records from k_follows table",
        k_follows_deleted
    );
    info!(
        "✓ Purge operation 1 completed: Total {} records deleted",
        k_blocks_deleted + k_follows_deleted
    );

    Ok(())
}
