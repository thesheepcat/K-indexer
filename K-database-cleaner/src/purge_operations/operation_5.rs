use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::info;

/// Purge Operation 5: Remove orphaned votes
/// This removes all votes that reference posts that no longer exist in the database,
/// including related data from k_mentions
pub async fn execute(pool: &PgPool) -> Result<()> {
    info!("Starting purge operation 5: Removing orphaned votes");

    // Single transaction with CTE to delete orphaned votes and related mentions atomically
    let mut tx = pool.begin().await?;

    let result = sqlx::query(
        r#"
        WITH orphaned_votes AS (
            SELECT transaction_id
            FROM k_votes
            WHERE post_id NOT IN (
                SELECT transaction_id FROM k_contents
            )
        ),
        deleted_mentions AS (
            DELETE FROM k_mentions
            WHERE content_id IN (SELECT transaction_id FROM orphaned_votes)
            RETURNING id
        ),
        deleted_votes AS (
            DELETE FROM k_votes
            WHERE transaction_id IN (SELECT transaction_id FROM orphaned_votes)
            RETURNING id
        )
        SELECT
            (SELECT COUNT(*) FROM deleted_mentions) as mentions_count,
            (SELECT COUNT(*) FROM deleted_votes) as votes_count
        "#,
    )
    .fetch_one(&mut *tx)
    .await?;

    let k_mentions_deleted: i64 = result.get("mentions_count");
    let k_votes_deleted: i64 = result.get("votes_count");

    tx.commit().await?;

    info!(
        "✓ Purge operation 5: Deleted {} mentions related to orphaned votes",
        k_mentions_deleted
    );
    info!(
        "✓ Purge operation 5: Deleted {} orphaned votes from k_votes table",
        k_votes_deleted
    );
    info!(
        "✓ Purge operation 5 completed: Total {} records deleted ({} from k_votes, {} mentions)",
        k_votes_deleted + k_mentions_deleted,
        k_votes_deleted,
        k_mentions_deleted
    );

    Ok(())
}
