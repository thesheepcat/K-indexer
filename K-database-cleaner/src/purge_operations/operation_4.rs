use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::info;

/// Purge Operation 4: Remove orphaned replies
/// This removes all replies that reference content that no longer exists in the database,
/// including related data from k_mentions
pub async fn execute(pool: &PgPool) -> Result<()> {
    info!("Starting purge operation 4: Removing orphaned replies");

    // Single transaction with CTE to delete orphaned replies and related mentions atomically
    let mut tx = pool.begin().await?;

    let result = sqlx::query(
        r#"
        WITH orphaned_replies AS (
            SELECT transaction_id
            FROM k_contents
            WHERE content_type = 'reply'
              AND referenced_content_id IS NOT NULL
              AND referenced_content_id NOT IN (
                  SELECT transaction_id FROM k_contents
              )
        ),
        deleted_mentions AS (
            DELETE FROM k_mentions
            WHERE content_id IN (SELECT transaction_id FROM orphaned_replies)
            RETURNING id
        ),
        deleted_contents AS (
            DELETE FROM k_contents
            WHERE transaction_id IN (SELECT transaction_id FROM orphaned_replies)
            RETURNING id
        )
        SELECT
            (SELECT COUNT(*) FROM deleted_mentions) as mentions_count,
            (SELECT COUNT(*) FROM deleted_contents) as contents_count
        "#,
    )
    .fetch_one(&mut *tx)
    .await?;

    let k_mentions_deleted: i64 = result.get("mentions_count");
    let k_contents_deleted: i64 = result.get("contents_count");

    tx.commit().await?;

    info!(
        "✓ Purge operation 4: Deleted {} mentions related to orphaned replies",
        k_mentions_deleted
    );
    info!(
        "✓ Purge operation 4: Deleted {} orphaned replies from k_contents table",
        k_contents_deleted
    );
    info!(
        "✓ Purge operation 4 completed: Total {} records deleted ({} from k_contents, {} mentions)",
        k_contents_deleted + k_mentions_deleted,
        k_contents_deleted,
        k_mentions_deleted
    );

    Ok(())
}
