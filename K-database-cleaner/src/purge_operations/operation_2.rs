use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::info;

/// Purge Operation 2: Remove all content from blocked users
/// This includes posts, quotes, replies, and votes from k_contents and k_votes tables,
/// along with related data from k_mentions table
pub async fn execute(pool: &PgPool, user_pubkey: &[u8]) -> Result<()> {
    info!("Starting purge operation 2: Removing blocked users' content");

    // Single transaction with CTE to delete all blocked users' content and mentions atomically
    let mut tx = pool.begin().await?;

    let result = sqlx::query(
        r#"
        WITH blocked_users AS (
            SELECT blocked_user_pubkey
            FROM k_blocks
            WHERE sender_pubkey = $1
        ),
        deleted_mentions_contents AS (
            DELETE FROM k_mentions
            WHERE content_id IN (
                SELECT transaction_id
                FROM k_contents
                WHERE sender_pubkey IN (SELECT blocked_user_pubkey FROM blocked_users)
            )
            RETURNING id
        ),
        deleted_contents AS (
            DELETE FROM k_contents
            WHERE sender_pubkey IN (SELECT blocked_user_pubkey FROM blocked_users)
            RETURNING id
        ),
        deleted_mentions_votes AS (
            DELETE FROM k_mentions
            WHERE content_id IN (
                SELECT transaction_id
                FROM k_votes
                WHERE sender_pubkey IN (SELECT blocked_user_pubkey FROM blocked_users)
            )
            RETURNING id
        ),
        deleted_votes AS (
            DELETE FROM k_votes
            WHERE sender_pubkey IN (SELECT blocked_user_pubkey FROM blocked_users)
            RETURNING id
        )
        SELECT
            (SELECT COUNT(*) FROM deleted_mentions_contents) as mentions_contents_count,
            (SELECT COUNT(*) FROM deleted_contents) as contents_count,
            (SELECT COUNT(*) FROM deleted_mentions_votes) as mentions_votes_count,
            (SELECT COUNT(*) FROM deleted_votes) as votes_count
        "#,
    )
    .bind(user_pubkey)
    .fetch_one(&mut *tx)
    .await?;

    let k_mentions_contents_deleted: i64 = result.get("mentions_contents_count");
    let k_contents_deleted: i64 = result.get("contents_count");
    let k_mentions_votes_deleted: i64 = result.get("mentions_votes_count");
    let k_votes_deleted: i64 = result.get("votes_count");

    tx.commit().await?;

    info!(
        "✓ Purge operation 2: Deleted {} mentions related to blocked users' content",
        k_mentions_contents_deleted
    );
    info!(
        "✓ Purge operation 2: Deleted {} records from k_contents table",
        k_contents_deleted
    );
    info!(
        "✓ Purge operation 2: Deleted {} mentions related to blocked users' votes",
        k_mentions_votes_deleted
    );
    info!(
        "✓ Purge operation 2: Deleted {} records from k_votes table",
        k_votes_deleted
    );
    info!(
        "✓ Purge operation 2 completed: Total {} records deleted ({} from k_contents, {} from k_votes, {} mentions)",
        k_contents_deleted
            + k_votes_deleted
            + k_mentions_contents_deleted
            + k_mentions_votes_deleted,
        k_contents_deleted,
        k_votes_deleted,
        k_mentions_contents_deleted + k_mentions_votes_deleted
    );

    Ok(())
}
