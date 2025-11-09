use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::info;

/// Purge Operation 3: Remove old posts and quotes from non-followed users
/// This removes posts and quotes older than the specified data retention period
/// from users who are not followed by the main user, including related data from k_mentions
pub async fn execute(pool: &PgPool, user_pubkey: &[u8], data_retention_hours: u64) -> Result<()> {
    info!(
        "Starting purge operation 3: Removing old posts/quotes from non-followed users (retention: {} hours)",
        data_retention_hours
    );

    // Calculate the cutoff timestamp (current time - retention period)
    // block_time is in milliseconds since epoch
    let cutoff_timestamp_ms = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64)
        - (data_retention_hours as i64 * 3600 * 1000);

    // Single transaction with CTE to delete old posts/quotes and related mentions atomically
    let mut tx = pool.begin().await?;

    let result = sqlx::query(
        r#"
        WITH old_content AS (
            SELECT transaction_id
            FROM k_contents
            WHERE content_type IN ('post', 'quote')
              AND block_time < $1
              AND sender_pubkey != $2
              AND sender_pubkey NOT IN (
                  SELECT followed_user_pubkey
                  FROM k_follows
                  WHERE sender_pubkey = $2
              )
        ),
        deleted_mentions AS (
            DELETE FROM k_mentions
            WHERE content_id IN (SELECT transaction_id FROM old_content)
            RETURNING id
        ),
        deleted_contents AS (
            DELETE FROM k_contents
            WHERE transaction_id IN (SELECT transaction_id FROM old_content)
            RETURNING id
        )
        SELECT
            (SELECT COUNT(*) FROM deleted_mentions) as mentions_count,
            (SELECT COUNT(*) FROM deleted_contents) as contents_count
        "#,
    )
    .bind(cutoff_timestamp_ms)
    .bind(user_pubkey)
    .fetch_one(&mut *tx)
    .await?;

    let k_mentions_deleted: i64 = result.get("mentions_count");
    let k_contents_deleted: i64 = result.get("contents_count");

    tx.commit().await?;

    info!(
        "✓ Purge operation 3: Deleted {} mentions related to old posts/quotes",
        k_mentions_deleted
    );
    info!(
        "✓ Purge operation 3: Deleted {} old posts/quotes from k_contents table",
        k_contents_deleted
    );
    info!(
        "✓ Purge operation 3 completed: Total {} records deleted ({} from k_contents, {} mentions)",
        k_contents_deleted + k_mentions_deleted,
        k_contents_deleted,
        k_mentions_deleted
    );

    Ok(())
}
