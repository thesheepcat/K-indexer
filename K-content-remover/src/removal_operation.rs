use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::info;

#[derive(Debug)]
pub struct RemovalStats {
    pub mentions_deleted: i64,
    pub contents_deleted: i64,
    pub votes_deleted: i64,
    pub broadcasts_deleted: i64,
    pub blocks_deleted: i64,
    pub follows_deleted: i64,
}

impl RemovalStats {
    pub fn total(&self) -> i64 {
        self.mentions_deleted
            + self.contents_deleted
            + self.votes_deleted
            + self.broadcasts_deleted
            + self.blocks_deleted
            + self.follows_deleted
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// Preview what would be deleted without actually deleting
pub async fn preview_removal(pool: &PgPool, target_user_pubkey: &[u8]) -> Result<RemovalStats> {
    info!(
        "Previewing content removal for user: {}",
        hex::encode(target_user_pubkey)
    );

    // Count mentions by sender_pubkey
    let mentions_count: i64 = sqlx::query(
        r#"
        SELECT COUNT(*)
        FROM k_mentions
        WHERE sender_pubkey = $1
        "#,
    )
    .bind(target_user_pubkey)
    .fetch_one(pool)
    .await?
    .get(0);

    // Count user's content
    let contents_count: i64 = sqlx::query(
        r#"
        SELECT COUNT(*)
        FROM k_contents
        WHERE sender_pubkey = $1
        "#,
    )
    .bind(target_user_pubkey)
    .fetch_one(pool)
    .await?
    .get(0);

    // Count user's votes
    let votes_count: i64 = sqlx::query(
        r#"
        SELECT COUNT(*)
        FROM k_votes
        WHERE sender_pubkey = $1
        "#,
    )
    .bind(target_user_pubkey)
    .fetch_one(pool)
    .await?
    .get(0);

    // Count user's broadcasts
    let broadcasts_count: i64 = sqlx::query(
        r#"
        SELECT COUNT(*)
        FROM k_broadcasts
        WHERE sender_pubkey = $1
        "#,
    )
    .bind(target_user_pubkey)
    .fetch_one(pool)
    .await?
    .get(0);

    // Count user's blocks
    let blocks_count: i64 = sqlx::query(
        r#"
        SELECT COUNT(*)
        FROM k_blocks
        WHERE sender_pubkey = $1
        "#,
    )
    .bind(target_user_pubkey)
    .fetch_one(pool)
    .await?
    .get(0);

    // Count user's follows
    let follows_count: i64 = sqlx::query(
        r#"
        SELECT COUNT(*)
        FROM k_follows
        WHERE sender_pubkey = $1
        "#,
    )
    .bind(target_user_pubkey)
    .fetch_one(pool)
    .await?
    .get(0);

    let stats = RemovalStats {
        mentions_deleted: mentions_count,
        contents_deleted: contents_count,
        votes_deleted: votes_count,
        broadcasts_deleted: broadcasts_count,
        blocks_deleted: blocks_count,
        follows_deleted: follows_count,
    };

    info!("Preview results:");
    info!("  - k_mentions:   {} records", stats.mentions_deleted);
    info!("  - k_contents:   {} records", stats.contents_deleted);
    info!("  - k_votes:      {} records", stats.votes_deleted);
    info!("  - k_broadcasts: {} records", stats.broadcasts_deleted);
    info!("  - k_blocks:     {} records", stats.blocks_deleted);
    info!("  - k_follows:    {} records", stats.follows_deleted);
    info!("  Total records to be deleted: {}", stats.total());

    Ok(stats)
}

/// Execute the removal operation - deletes all content created by the target user
/// Deletes records ONLY where sender_pubkey matches the target user
pub async fn execute_removal(pool: &PgPool, target_user_pubkey: &[u8]) -> Result<RemovalStats> {
    info!(
        "Starting content removal for user: {}",
        hex::encode(target_user_pubkey)
    );

    // Single transaction with CTEs to delete all user's content atomically
    // Deletion is based ONLY on sender_pubkey matching
    let mut tx = pool.begin().await?;

    let result = sqlx::query(
        r#"
        WITH deleted_mentions AS (
            DELETE FROM k_mentions
            WHERE sender_pubkey = $1
            RETURNING id
        ),
        deleted_contents AS (
            DELETE FROM k_contents
            WHERE sender_pubkey = $1
            RETURNING id
        ),
        deleted_votes AS (
            DELETE FROM k_votes
            WHERE sender_pubkey = $1
            RETURNING id
        ),
        deleted_broadcasts AS (
            DELETE FROM k_broadcasts
            WHERE sender_pubkey = $1
            RETURNING id
        ),
        deleted_blocks AS (
            DELETE FROM k_blocks
            WHERE sender_pubkey = $1
            RETURNING id
        ),
        deleted_follows AS (
            DELETE FROM k_follows
            WHERE sender_pubkey = $1
            RETURNING id
        )
        SELECT
            (SELECT COUNT(*) FROM deleted_mentions) as mentions_count,
            (SELECT COUNT(*) FROM deleted_contents) as contents_count,
            (SELECT COUNT(*) FROM deleted_votes) as votes_count,
            (SELECT COUNT(*) FROM deleted_broadcasts) as broadcasts_count,
            (SELECT COUNT(*) FROM deleted_blocks) as blocks_count,
            (SELECT COUNT(*) FROM deleted_follows) as follows_count
        "#,
    )
    .bind(target_user_pubkey)
    .fetch_one(&mut *tx)
    .await?;

    let stats = RemovalStats {
        mentions_deleted: result.get("mentions_count"),
        contents_deleted: result.get("contents_count"),
        votes_deleted: result.get("votes_count"),
        broadcasts_deleted: result.get("broadcasts_count"),
        blocks_deleted: result.get("blocks_count"),
        follows_deleted: result.get("follows_count"),
    };

    tx.commit().await?;

    info!("✓ Content removal completed successfully:");
    info!(
        "  - Deleted {} records from k_mentions",
        stats.mentions_deleted
    );
    info!(
        "  - Deleted {} records from k_contents",
        stats.contents_deleted
    );
    info!("  - Deleted {} records from k_votes", stats.votes_deleted);
    info!(
        "  - Deleted {} records from k_broadcasts",
        stats.broadcasts_deleted
    );
    info!("  - Deleted {} records from k_blocks", stats.blocks_deleted);
    info!(
        "  - Deleted {} records from k_follows",
        stats.follows_deleted
    );
    info!("  Total records deleted: {}", stats.total());

    Ok(stats)
}
