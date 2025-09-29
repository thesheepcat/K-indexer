use anyhow::Result;
use async_trait::async_trait;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use tracing::{info, warn};

use crate::database_trait::{
    DatabaseError, DatabaseInterface, DatabaseResult, PaginatedResult, QueryOptions,
};
use crate::models::{
    ContentRecord, KBroadcastRecord, KPostRecord, KReplyRecord, KVoteRecord,
    NotificationContentRecord, PaginationMetadata,
};

pub struct PostgresDbManager {
    pub pool: PgPool,
}

impl PostgresDbManager {
    pub async fn new(connection_string: &str, max_connections: u32) -> Result<Self, sqlx::Error> {
        loop {
            match PgPoolOptions::new()
                .max_connections(max_connections)
                .acquire_timeout(std::time::Duration::from_secs(30))
                .connect(connection_string)
                .await
            {
                Ok(pool) => {
                    // Test the pool connection
                    match sqlx::query("SELECT 1").fetch_one(&pool).await {
                        Ok(_) => {
                            info!("Database connection pool created and tested successfully");
                            return Ok(Self { pool });
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

    fn create_compound_pagination_metadata<T>(
        &self,
        items: &[T],
        _limit: u32,
        has_more: bool,
    ) -> PaginationMetadata
    where
        T: HasCompoundCursor,
    {
        let next_cursor = if has_more && !items.is_empty() {
            let last_item = items.last().unwrap();
            Some(Self::create_compound_cursor(
                last_item.get_timestamp(),
                last_item.get_id(),
            ))
        } else {
            None
        };

        let prev_cursor = if !items.is_empty() {
            let first_item = items.first().unwrap();
            Some(Self::create_compound_cursor(
                first_item.get_timestamp(),
                first_item.get_id(),
            ))
        } else {
            None
        };

        PaginationMetadata {
            has_more,
            next_cursor,
            prev_cursor,
        }
    }

    fn decode_hex_to_bytes(hex_str: &str) -> DatabaseResult<Vec<u8>> {
        hex::decode(hex_str)
            .map_err(|e| DatabaseError::InvalidInput(format!("Invalid hex string: {}", e)))
    }

    fn encode_bytes_to_hex(bytes: &[u8]) -> String {
        hex::encode(bytes)
    }

    fn parse_compound_cursor(cursor: &str) -> DatabaseResult<(u64, i64)> {
        if cursor.contains('_') {
            let parts: Vec<&str> = cursor.split('_').collect();
            if parts.len() == 2 {
                let timestamp = parts[0].parse::<u64>().map_err(|_| {
                    DatabaseError::InvalidInput("Invalid timestamp in cursor".to_string())
                })?;
                let id = parts[1]
                    .parse::<i64>()
                    .map_err(|_| DatabaseError::InvalidInput("Invalid ID in cursor".to_string()))?;
                return Ok((timestamp, id));
            }
        }
        // Fallback: treat as simple timestamp cursor for backward compatibility
        let timestamp = cursor
            .parse::<u64>()
            .map_err(|_| DatabaseError::InvalidInput("Invalid cursor format".to_string()))?;
        Ok((timestamp, i64::MAX)) // Use MAX to include all records with same timestamp
    }

    fn create_compound_cursor(timestamp: u64, id: i64) -> String {
        format!("{}_{}", timestamp, id)
    }
}

trait HasCompoundCursor {
    fn get_timestamp(&self) -> u64;
    fn get_id(&self) -> i64;
}

impl HasCompoundCursor for KPostRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
    }

    fn get_id(&self) -> i64 {
        self.id
    }
}

impl HasCompoundCursor for KReplyRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
    }

    fn get_id(&self) -> i64 {
        self.id
    }
}

impl HasCompoundCursor for KBroadcastRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
    }

    fn get_id(&self) -> i64 {
        self.id
    }
}

impl HasCompoundCursor for KVoteRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
    }

    fn get_id(&self) -> i64 {
        self.id
    }
}

impl HasCompoundCursor for ContentRecord {
    fn get_timestamp(&self) -> u64 {
        match self {
            ContentRecord::Post(post) => post.block_time,
            ContentRecord::Reply(reply) => reply.block_time,
            ContentRecord::Vote(vote) => vote.block_time,
        }
    }

    fn get_id(&self) -> i64 {
        match self {
            ContentRecord::Post(post) => post.id,
            ContentRecord::Reply(reply) => reply.id,
            ContentRecord::Vote(vote) => vote.id,
        }
    }
}

#[async_trait]
#[allow(unused_variables)]
impl DatabaseInterface for PostgresDbManager {
    // Broadcast operations

    async fn get_all_broadcasts_with_block_status(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<(KBroadcastRecord, bool)>> {
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1;

        let mut query = String::from(
            r#"
            SELECT
                b.id, b.transaction_id, b.block_time, b.sender_pubkey, b.sender_signature,
                b.base64_encoded_nickname, b.base64_encoded_profile_image, b.base64_encoded_message,
                CASE
                    WHEN kb.blocked_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_blocked
            FROM k_broadcasts b
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = b.sender_pubkey
            WHERE 1=1
            "#,
        );

        let mut bind_count = 1; // Start with 1 since we already have requester_pubkey

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (b.block_time < ${} OR (b.block_time = ${} AND b.id < ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (b.block_time > ${} OR (b.block_time = ${} AND b.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if options.sort_descending {
            query.push_str(" ORDER BY b.block_time DESC, b.id DESC");
        } else {
            query.push_str(" ORDER BY b.block_time ASC, b.id ASC");
        }

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query).bind(&requester_pubkey_bytes);

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder.bind(before_timestamp as i64).bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder.bind(after_timestamp as i64).bind(after_id);
            }
        }

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder.fetch_all(&self.pool).await.map_err(|e| {
            DatabaseError::QueryError(format!(
                "Failed to fetch all broadcasts with block status: {}",
                e
            ))
        })?;

        let mut broadcasts_with_block_status = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let is_blocked: bool = row.get("is_blocked");

            let broadcast_record = KBroadcastRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_nickname: row.get("base64_encoded_nickname"),
                base64_encoded_profile_image: row.get("base64_encoded_profile_image"),
                base64_encoded_message: row.get("base64_encoded_message"),
            };

            broadcasts_with_block_status.push((broadcast_record, is_blocked));
        }

        let has_more = broadcasts_with_block_status.len() > limit as usize;
        if has_more {
            broadcasts_with_block_status.pop();
        }

        // Extract just the broadcast records for pagination metadata calculation
        let broadcast_records: Vec<KBroadcastRecord> = broadcasts_with_block_status
            .iter()
            .map(|(record, _)| record.clone())
            .collect();
        let pagination =
            self.create_compound_pagination_metadata(&broadcast_records, limit as u32, has_more);

        Ok(PaginatedResult {
            items: broadcasts_with_block_status,
            pagination,
        })
    }

    async fn get_broadcast_by_user_with_block_status(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
    ) -> DatabaseResult<Option<(KBroadcastRecord, bool)>> {
        let user_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;

        // First, always check if user is blocked
        let blocking_query = r#"
            SELECT EXISTS (
                SELECT 1 FROM k_blocks kb
                WHERE kb.sender_pubkey = $2 AND kb.blocked_user_pubkey = $1
            ) as is_blocked
        "#;

        let blocking_row = sqlx::query(blocking_query)
            .bind(&user_pubkey_bytes)
            .bind(&requester_pubkey_bytes)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                DatabaseError::QueryError(format!("Failed to check blocking status: {}", e))
            })?;

        let is_blocked: bool = blocking_row.get("is_blocked");

        // Then check for broadcast data
        let query = r#"
            SELECT
                b.id, b.transaction_id, b.block_time, b.sender_pubkey, b.sender_signature,
                b.base64_encoded_nickname, b.base64_encoded_profile_image, b.base64_encoded_message
            FROM k_broadcasts b
            WHERE b.sender_pubkey = $1
            ORDER BY b.block_time DESC
            LIMIT 1
        "#;

        let row_opt = sqlx::query(query)
            .bind(&user_pubkey_bytes)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                DatabaseError::QueryError(format!("Failed to fetch broadcast by user: {}", e))
            })?;

        if let Some(row) = row_opt {
            // User has broadcast data
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");

            let broadcast_record = KBroadcastRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_nickname: row.get("base64_encoded_nickname"),
                base64_encoded_profile_image: row.get("base64_encoded_profile_image"),
                base64_encoded_message: row.get("base64_encoded_message"),
            };

            Ok(Some((broadcast_record, is_blocked)))
        } else {
            // No broadcast data found, but we have blocking status
            // Create a minimal broadcast record with empty fields and the blocking status
            let broadcast_record = KBroadcastRecord {
                id: 0, // Dummy ID
                transaction_id: String::new(),
                block_time: 0,
                sender_pubkey: user_public_key.to_string(),
                sender_signature: String::new(),
                base64_encoded_nickname: String::new(),
                base64_encoded_profile_image: None,
                base64_encoded_message: String::new(),
            };

            Ok(Some((broadcast_record, is_blocked)))
        }
    }

    async fn get_blocked_users_by_requester(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KBroadcastRecord>> {
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1;

        let mut query = String::from(
            r#"
            SELECT kb.id, kb.transaction_id, kb.block_time, kb.blocked_user_pubkey as sender_pubkey, kb.sender_signature,
                   COALESCE(b.base64_encoded_nickname, '') as base64_encoded_nickname,
                   b.base64_encoded_profile_image,
                   COALESCE(b.base64_encoded_message, '') as base64_encoded_message
            FROM k_blocks kb
            LEFT JOIN k_broadcasts b ON b.sender_pubkey = kb.blocked_user_pubkey
            WHERE kb.sender_pubkey = $1
            "#,
        );

        let mut bind_count = 1;

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (kb.block_time < ${} OR (kb.block_time = ${} AND kb.id < ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (kb.block_time > ${} OR (kb.block_time = ${} AND kb.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if options.sort_descending {
            query.push_str(" ORDER BY kb.block_time DESC, kb.id DESC");
        } else {
            query.push_str(" ORDER BY kb.block_time ASC, kb.id ASC");
        }

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query);
        query_builder = query_builder.bind(&requester_pubkey_bytes);

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder.bind(before_timestamp as i64).bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder.bind(after_timestamp as i64).bind(after_id);
            }
        }

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder.fetch_all(&self.pool).await.map_err(|e| {
            DatabaseError::QueryError(format!("Failed to fetch blocked users by requester: {}", e))
        })?;

        let mut broadcasts = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");

            broadcasts.push(KBroadcastRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_nickname: row.get("base64_encoded_nickname"),
                base64_encoded_profile_image: row.get("base64_encoded_profile_image"),
                base64_encoded_message: row.get("base64_encoded_message"),
            });
        }

        let has_more = broadcasts.len() > limit as usize;
        if has_more {
            broadcasts.pop();
        }

        let pagination =
            self.create_compound_pagination_metadata(&broadcasts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: broadcasts,
            pagination,
        })
    }

    // Optimized single-query method for get-posts-watching API with blocking awareness
    async fn get_all_posts_with_metadata_and_block_status(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<(KPostRecord, bool)>> {
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        let mut bind_count = 1;
        let mut cursor_conditions = String::new();

        // Add cursor logic to the all_posts CTE
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (p.block_time < ${} OR (p.block_time = ${} AND p.id < ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (p.block_time > ${} OR (p.block_time = ${} AND p.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY p.block_time DESC, p.id DESC"
        } else {
            " ORDER BY p.block_time ASC, p.id ASC"
        };

        let final_order_clause = if options.sort_descending {
            " ORDER BY ps.block_time DESC, ps.id DESC"
        } else {
            " ORDER BY ps.block_time ASC, ps.id ASC"
        };

        let query = format!(
            r#"
            WITH all_posts AS (
                SELECT p.id, p.transaction_id, p.block_time, p.sender_pubkey,
                       p.sender_signature, p.base64_encoded_message
                FROM k_posts p
                WHERE 1=1{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ), post_stats AS (
                SELECT lp.id, lp.transaction_id, lp.block_time, lp.sender_pubkey,
                       lp.sender_signature, lp.base64_encoded_message,
                       COALESCE(r.replies_count, 0) as replies_count,
                       COALESCE(v.up_votes_count, 0) as up_votes_count,
                       COALESCE(v.down_votes_count, 0) as down_votes_count,
                       COALESCE(v.user_upvoted, false) as is_upvoted,
                       COALESCE(v.user_downvoted, false) as is_downvoted
                FROM all_posts lp
                LEFT JOIN (
                    SELECT post_id, COUNT(*) as replies_count
                    FROM k_replies r
                    WHERE EXISTS (SELECT 1 FROM all_posts lp WHERE lp.transaction_id = r.post_id)
                    GROUP BY post_id
                ) r ON lp.transaction_id = r.post_id
                LEFT JOIN (
                    SELECT post_id,
                           COUNT(*) FILTER (WHERE vote = 'upvote') as up_votes_count,
                           COUNT(*) FILTER (WHERE vote = 'downvote') as down_votes_count,
                           bool_or(vote = 'upvote' AND sender_pubkey = $1) as user_upvoted,
                           bool_or(vote = 'downvote' AND sender_pubkey = $1) as user_downvoted
                    FROM k_votes v
                    WHERE EXISTS (SELECT 1 FROM all_posts lp WHERE lp.transaction_id = v.post_id)
                    GROUP BY post_id
                ) v ON lp.transaction_id = v.post_id
            )
            SELECT ps.id, ps.transaction_id, ps.block_time, ps.sender_pubkey,
                   ps.sender_signature, ps.base64_encoded_message,
                   COALESCE(ARRAY(SELECT encode(m.mentioned_pubkey, 'hex') FROM k_mentions m
                                  WHERE m.content_id = ps.transaction_id AND m.content_type = 'post'), '{{}}') as mentioned_pubkeys,
                   ps.replies_count, ps.up_votes_count, ps.down_votes_count,
                   ps.is_upvoted, ps.is_downvoted,
                   COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                   b.base64_encoded_profile_image as user_profile_image,
                   CASE
                       WHEN kb.blocked_user_pubkey IS NOT NULL THEN true
                       ELSE false
                   END as is_blocked
            FROM post_stats ps
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = ps.sender_pubkey
                ORDER BY b.block_time DESC
                LIMIT 1
            ) b ON true
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = ps.sender_pubkey
            WHERE 1=1
            {final_order_clause}
            "#,
            cursor_conditions = cursor_conditions,
            order_clause = order_clause,
            final_order_clause = final_order_clause,
            limit_param = bind_count + 1
        );

        // Build query with parameter binding following get-mentions pattern
        let mut query_builder = sqlx::query(&query).bind(&requester_pubkey_bytes);

        // Add cursor parameters if present
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder.bind(before_timestamp as i64).bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder.bind(after_timestamp as i64).bind(after_id);
            }
        }

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let has_more = rows.len() > limit as usize;
        let actual_items = if has_more {
            rows.into_iter().take(limit as usize).collect::<Vec<_>>()
        } else {
            rows.into_iter().collect::<Vec<_>>()
        };

        let mut posts_with_block_status = Vec::new();
        for row in actual_items {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");
            let is_blocked: bool = row.get("is_blocked");

            let post_record = KPostRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
                replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                is_upvoted: Some(row.get("is_upvoted")),
                is_downvoted: Some(row.get("is_downvoted")),
                user_nickname: Some(row.get("user_nickname")),
                user_profile_image: row.get("user_profile_image"),
            };

            posts_with_block_status.push((post_record, is_blocked));
        }

        // Extract just the posts for pagination metadata calculation
        let posts: Vec<KPostRecord> = posts_with_block_status
            .iter()
            .map(|(post, _)| post.clone())
            .collect();
        let pagination = self.create_compound_pagination_metadata(&posts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: posts_with_block_status,
            pagination,
        })
    }

    async fn get_contents_mentioning_user_with_metadata_and_block_status(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<(ContentRecord, bool)>> {
        let mentioned_user_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        let mut bind_count = 1;
        let mut post_cursor_conditions = String::new();
        let mut reply_cursor_conditions = String::new();

        // Add cursor logic for posts
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                post_cursor_conditions.push_str(&format!(
                    " AND (p.block_time < ${} OR (p.block_time = ${} AND p.id < ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
                reply_cursor_conditions.push_str(&format!(
                    " AND (r.block_time < ${} OR (r.block_time = ${} AND r.id < ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                post_cursor_conditions.push_str(&format!(
                    " AND (p.block_time > ${} OR (p.block_time = ${} AND p.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
                reply_cursor_conditions.push_str(&format!(
                    " AND (r.block_time > ${} OR (r.block_time = ${} AND r.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let post_order_clause = if options.sort_descending {
            " ORDER BY p.block_time DESC, p.id DESC"
        } else {
            " ORDER BY p.block_time ASC, p.id ASC"
        };

        let reply_order_clause = if options.sort_descending {
            " ORDER BY r.block_time DESC, r.id DESC"
        } else {
            " ORDER BY r.block_time ASC, r.id ASC"
        };

        let final_order_clause = if options.sort_descending {
            " ORDER BY block_time DESC, id DESC"
        } else {
            " ORDER BY block_time ASC, id ASC"
        };

        let cs_final_order_clause = if options.sort_descending {
            " ORDER BY cs.block_time DESC, cs.id DESC"
        } else {
            " ORDER BY cs.block_time ASC, cs.id ASC"
        };

        let query = format!(
            r#"
            WITH mentioned_posts AS (
                -- Get posts that mention the specific user with efficient filtering and LIMIT
                SELECT 'post' as content_type, p.id, p.transaction_id, p.block_time, p.sender_pubkey,
                       p.sender_signature, p.base64_encoded_message, NULL::bytea as post_id
                FROM k_posts p
                WHERE EXISTS (
                    SELECT 1
                    FROM k_mentions m
                    WHERE m.content_type = 'post'
                      AND m.mentioned_pubkey = $1
                      AND m.content_id = p.transaction_id
                ){post_cursor_conditions}
                {post_order_clause}
                LIMIT ${limit_param}
            ),
            mentioned_replies AS (
                -- Get replies that mention the specific user with efficient filtering and LIMIT
                SELECT 'reply' as content_type, r.id, r.transaction_id, r.block_time, r.sender_pubkey,
                       r.sender_signature, r.base64_encoded_message, r.post_id
                FROM k_replies r
                WHERE EXISTS (
                    SELECT 1
                    FROM k_mentions m
                    WHERE m.content_type = 'reply'
                      AND m.mentioned_pubkey = $1
                      AND m.content_id = r.transaction_id
                ){reply_cursor_conditions}
                {reply_order_clause}
                LIMIT ${limit_param}
            ),
            mentioned_content AS (
                -- Combine limited posts and replies
                SELECT * FROM mentioned_posts
                UNION ALL
                SELECT * FROM mentioned_replies
                {final_order_clause}
                LIMIT ${limit_param}
            ),
            content_stats AS (
                -- Pre-aggregate all metadata in one pass
                SELECT
                    mc.content_type, mc.id, mc.transaction_id, mc.block_time, mc.sender_pubkey,
                    mc.sender_signature, mc.base64_encoded_message, mc.post_id,

                    -- Replies count (only applicable for posts, not replies)
                    CASE WHEN mc.content_type = 'post' THEN COALESCE(r.replies_count, 0) ELSE 0 END as replies_count,

                    -- Vote statistics
                    COALESCE(v.up_votes_count, 0) as up_votes_count,
                    COALESCE(v.down_votes_count, 0) as down_votes_count,
                    COALESCE(v.user_upvoted, false) as is_upvoted,
                    COALESCE(v.user_downvoted, false) as is_downvoted

                FROM mentioned_content mc

                -- Optimized replies aggregation (only for posts)
                LEFT JOIN (
                    SELECT post_id, COUNT(*) as replies_count
                    FROM k_replies r
                    WHERE EXISTS (SELECT 1 FROM mentioned_content mc WHERE mc.content_type = 'post' AND mc.transaction_id = r.post_id)
                    GROUP BY post_id
                ) r ON mc.content_type = 'post' AND mc.transaction_id = r.post_id

                -- Optimized vote aggregation with user vote in single query
                LEFT JOIN (
                    SELECT
                        post_id,
                        COUNT(*) FILTER (WHERE vote = 'upvote') as up_votes_count,
                        COUNT(*) FILTER (WHERE vote = 'downvote') as down_votes_count,
                        bool_or(vote = 'upvote' AND sender_pubkey = ${requester_param}) as user_upvoted,
                        bool_or(vote = 'downvote' AND sender_pubkey = ${requester_param}) as user_downvoted
                    FROM k_votes v
                    WHERE EXISTS (SELECT 1 FROM mentioned_content mc WHERE mc.transaction_id = v.post_id)
                    GROUP BY post_id
                ) v ON mc.transaction_id = v.post_id
            )
            SELECT
                cs.content_type, cs.id, cs.transaction_id, cs.block_time, cs.sender_pubkey,
                cs.sender_signature, cs.base64_encoded_message, cs.post_id,

                -- Get mentioned pubkeys efficiently (use appropriate content_type)
                COALESCE(
                    ARRAY(
                        SELECT encode(m.mentioned_pubkey, 'hex')
                        FROM k_mentions m
                        WHERE m.content_id = cs.transaction_id AND m.content_type = cs.content_type
                    ),
                    '{{}}'::text[]
                ) as mentioned_pubkeys,

                cs.replies_count,
                cs.up_votes_count,
                cs.down_votes_count,
                cs.is_upvoted,
                cs.is_downvoted,

                -- User profile lookup with efficient filtering
                COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                b.base64_encoded_profile_image as user_profile_image,

                -- Blocking status check
                CASE
                    WHEN kb.blocked_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_blocked

            FROM content_stats cs
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = cs.sender_pubkey
                ORDER BY b.block_time DESC
                LIMIT 1
            ) b ON true
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = ${requester_param} AND kb.blocked_user_pubkey = cs.sender_pubkey
            WHERE 1=1
            {cs_final_order_clause}
            "#,
            post_cursor_conditions = post_cursor_conditions,
            reply_cursor_conditions = reply_cursor_conditions,
            post_order_clause = post_order_clause,
            reply_order_clause = reply_order_clause,
            final_order_clause = final_order_clause,
            cs_final_order_clause = cs_final_order_clause,
            limit_param = bind_count + 1,
            requester_param = bind_count + 2
        );

        // Build query with parameter binding
        let mut query_builder = sqlx::query(&query).bind(&mentioned_user_pubkey_bytes);

        // Add cursor parameters if present
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder.bind(before_timestamp as i64).bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder.bind(after_timestamp as i64).bind(after_id);
            }
        }

        query_builder = query_builder
            .bind(offset_limit)
            .bind(&requester_pubkey_bytes);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let has_more = rows.len() > limit as usize;
        let actual_items = if has_more {
            rows.into_iter().take(limit as usize).collect::<Vec<_>>()
        } else {
            rows.into_iter().collect::<Vec<_>>()
        };

        let mut content_records_with_block_status = Vec::new();
        for row in actual_items {
            let content_type: &str = row.get("content_type");
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");
            let is_blocked: bool = row.get("is_blocked");

            let content_record = match content_type {
                "post" => {
                    let post_record = KPostRecord {
                        id: row.get::<i64, _>("id"),
                        transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                        block_time: row.get::<i64, _>("block_time") as u64,
                        sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                        sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                        base64_encoded_message: row.get("base64_encoded_message"),
                        mentioned_pubkeys: mentioned_pubkeys_array,
                        replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                        up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                        down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                        is_upvoted: Some(row.get("is_upvoted")),
                        is_downvoted: Some(row.get("is_downvoted")),
                        user_nickname: Some(row.get("user_nickname")),
                        user_profile_image: row.get("user_profile_image"),
                    };
                    ContentRecord::Post(post_record)
                }
                "reply" => {
                    let post_id: Option<Vec<u8>> = row.get("post_id");
                    let post_id_hex = match post_id {
                        Some(bytes) => Self::encode_bytes_to_hex(&bytes),
                        None => {
                            return Err(DatabaseError::QueryError(
                                "Missing post_id for reply".to_string(),
                            ));
                        }
                    };

                    let reply_record = KReplyRecord {
                        id: row.get::<i64, _>("id"),
                        transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                        block_time: row.get::<i64, _>("block_time") as u64,
                        sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                        sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                        post_id: post_id_hex,
                        base64_encoded_message: row.get("base64_encoded_message"),
                        mentioned_pubkeys: mentioned_pubkeys_array,
                        replies_count: Some(0), // Replies don't have replies
                        up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                        down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                        is_upvoted: Some(row.get("is_upvoted")),
                        is_downvoted: Some(row.get("is_downvoted")),
                        user_nickname: Some(row.get("user_nickname")),
                        user_profile_image: row.get("user_profile_image"),
                    };
                    ContentRecord::Reply(reply_record)
                }
                _ => {
                    return Err(DatabaseError::QueryError(format!(
                        "Unknown content type: {}",
                        content_type
                    )));
                }
            };

            content_records_with_block_status.push((content_record, is_blocked));
        }

        // Extract just the content records for pagination metadata calculation
        let content_records: Vec<ContentRecord> = content_records_with_block_status
            .iter()
            .map(|(record, _)| record.clone())
            .collect();
        let pagination =
            self.create_compound_pagination_metadata(&content_records, limit as u32, has_more);

        Ok(PaginatedResult {
            items: content_records_with_block_status,
            pagination,
        })
    }

    async fn get_content_by_id_with_metadata_and_block_status(
        &self,
        content_id: &str,
        requester_pubkey: &str,
    ) -> DatabaseResult<Option<(ContentRecord, bool)>> {
        let content_id_bytes = Self::decode_hex_to_bytes(content_id)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;

        let query = r#"
            SELECT
                content.content_type,
                content.id,
                content.transaction_id,
                content.block_time,
                content.sender_pubkey,
                content.sender_signature,
                content.post_id, -- NULL for posts, actual value for replies
                content.base64_encoded_message,
                content.mentioned_pubkeys,
                content.replies_count,
                content.up_votes_count,
                content.down_votes_count,
                content.is_upvoted,
                content.is_downvoted,
                content.user_nickname,
                content.user_profile_image,
                CASE
                    WHEN kb.blocked_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_blocked
            FROM (
                -- Posts subquery with all metadata (executes first)
                SELECT
                    'post' as content_type,
                    p.id,
                    p.transaction_id,
                    p.block_time,
                    p.sender_pubkey,
                    p.sender_signature,
                    NULL::bytea as post_id, -- NULL for posts
                    p.base64_encoded_message,
                    COALESCE(
                        ARRAY(
                            SELECT m.mentioned_pubkey
                            FROM k_mentions m
                            WHERE m.content_id = p.transaction_id AND m.content_type = 'post'
                        ),
                        ARRAY[]::bytea[]
                    ) as mentioned_pubkeys,
                    COALESCE(reply_counts.replies_count, 0) as replies_count,
                    COALESCE(vote_counts.up_votes_count, 0) as up_votes_count,
                    COALESCE(vote_counts.down_votes_count, 0) as down_votes_count,
                    COALESCE(user_vote.is_upvoted, false) as is_upvoted,
                    COALESCE(user_vote.is_downvoted, false) as is_downvoted,
                    user_profile.base64_encoded_nickname as user_nickname,
                    user_profile.base64_encoded_profile_image as user_profile_image
                FROM k_posts p
                LEFT JOIN (
                    SELECT post_id, COUNT(*) as replies_count
                    FROM k_replies
                    GROUP BY post_id
                ) reply_counts ON p.transaction_id = reply_counts.post_id
                LEFT JOIN (
                    SELECT
                        post_id,
                        COUNT(*) FILTER (WHERE vote = 'upvote') as up_votes_count,
                        COUNT(*) FILTER (WHERE vote = 'downvote') as down_votes_count
                    FROM k_votes
                    GROUP BY post_id
                ) vote_counts ON p.transaction_id = vote_counts.post_id
                LEFT JOIN (
                    SELECT
                        post_id,
                        sender_pubkey,
                        bool_or(vote = 'upvote') as is_upvoted,
                        bool_or(vote = 'downvote') as is_downvoted
                    FROM k_votes
                    WHERE sender_pubkey = $2
                    GROUP BY post_id, sender_pubkey
                ) user_vote ON p.transaction_id = user_vote.post_id
                LEFT JOIN (
                    SELECT DISTINCT ON (sender_pubkey)
                        sender_pubkey,
                        base64_encoded_nickname,
                        base64_encoded_profile_image
                    FROM k_broadcasts
                    ORDER BY sender_pubkey, block_time DESC
                ) user_profile ON p.sender_pubkey = user_profile.sender_pubkey
                WHERE p.transaction_id = $1

                UNION ALL

                -- Replies subquery with all metadata (only executes if NOT found in posts)
                SELECT
                    'reply' as content_type,
                    r.id,
                    r.transaction_id,
                    r.block_time,
                    r.sender_pubkey,
                    r.sender_signature,
                    r.post_id,
                    r.base64_encoded_message,
                    COALESCE(
                        ARRAY(
                            SELECT m.mentioned_pubkey
                            FROM k_mentions m
                            WHERE m.content_id = r.transaction_id AND m.content_type = 'reply'
                        ),
                        ARRAY[]::bytea[]
                    ) as mentioned_pubkeys,
                    0 as replies_count, -- replies don't have nested replies
                    COALESCE(vote_counts.up_votes_count, 0) as up_votes_count,
                    COALESCE(vote_counts.down_votes_count, 0) as down_votes_count,
                    COALESCE(user_vote.is_upvoted, false) as is_upvoted,
                    COALESCE(user_vote.is_downvoted, false) as is_downvoted,
                    user_profile.base64_encoded_nickname as user_nickname,
                    user_profile.base64_encoded_profile_image as user_profile_image
                FROM k_replies r
                LEFT JOIN (
                    SELECT
                        post_id,
                        COUNT(*) FILTER (WHERE vote = 'upvote') as up_votes_count,
                        COUNT(*) FILTER (WHERE vote = 'downvote') as down_votes_count
                    FROM k_votes
                    GROUP BY post_id
                ) vote_counts ON r.transaction_id = vote_counts.post_id
                LEFT JOIN (
                    SELECT
                        post_id,
                        sender_pubkey,
                        bool_or(vote = 'upvote') as is_upvoted,
                        bool_or(vote = 'downvote') as is_downvoted
                    FROM k_votes
                    WHERE sender_pubkey = $2
                    GROUP BY post_id, sender_pubkey
                ) user_vote ON r.transaction_id = user_vote.post_id
                LEFT JOIN (
                    SELECT DISTINCT ON (sender_pubkey)
                        sender_pubkey,
                        base64_encoded_nickname,
                        base64_encoded_profile_image
                    FROM k_broadcasts
                    ORDER BY sender_pubkey, block_time DESC
                ) user_profile ON r.sender_pubkey = user_profile.sender_pubkey
                WHERE r.transaction_id = $1
                AND NOT EXISTS (SELECT 1 FROM k_posts WHERE transaction_id = $1)
            ) content
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = $2 AND kb.blocked_user_pubkey = content.sender_pubkey
            LIMIT 1
        "#;

        let row = match sqlx::query(query)
            .bind(&content_id_bytes)
            .bind(&requester_pubkey_bytes)
            .fetch_optional(&self.pool)
            .await
        {
            Ok(Some(row)) => row,
            Ok(None) => return Ok(None),
            Err(e) => return Err(DatabaseError::QueryError(e.to_string())),
        };

        let content_type: &str = row.get("content_type");
        let is_blocked: bool = row.get("is_blocked");

        let content_record = match content_type {
            "post" => {
                let mentioned_pubkeys_bytes: Vec<Vec<u8>> = row.get("mentioned_pubkeys");
                let mentioned_pubkeys: Vec<String> = mentioned_pubkeys_bytes
                    .into_iter()
                    .map(|bytes| hex::encode(bytes))
                    .collect();

                let post_record = KPostRecord {
                    id: row.get("id"),
                    transaction_id: hex::encode(row.get::<Vec<u8>, _>("transaction_id")),
                    block_time: row.get::<i64, _>("block_time") as u64,
                    sender_pubkey: hex::encode(row.get::<Vec<u8>, _>("sender_pubkey")),
                    sender_signature: hex::encode(row.get::<Vec<u8>, _>("sender_signature")),
                    base64_encoded_message: row.get("base64_encoded_message"),
                    mentioned_pubkeys,
                    replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                    up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                    down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                    is_upvoted: Some(row.get("is_upvoted")),
                    is_downvoted: Some(row.get("is_downvoted")),
                    user_nickname: row.get("user_nickname"),
                    user_profile_image: row.get("user_profile_image"),
                };

                ContentRecord::Post(post_record)
            }
            "reply" => {
                let mentioned_pubkeys_bytes: Vec<Vec<u8>> = row.get("mentioned_pubkeys");
                let mentioned_pubkeys: Vec<String> = mentioned_pubkeys_bytes
                    .into_iter()
                    .map(|bytes| hex::encode(bytes))
                    .collect();

                let reply_record = KReplyRecord {
                    id: row.get("id"),
                    transaction_id: hex::encode(row.get::<Vec<u8>, _>("transaction_id")),
                    block_time: row.get::<i64, _>("block_time") as u64,
                    sender_pubkey: hex::encode(row.get::<Vec<u8>, _>("sender_pubkey")),
                    sender_signature: hex::encode(row.get::<Vec<u8>, _>("sender_signature")),
                    post_id: hex::encode(row.get::<Vec<u8>, _>("post_id")),
                    base64_encoded_message: row.get("base64_encoded_message"),
                    mentioned_pubkeys,
                    replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                    up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                    down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                    is_upvoted: Some(row.get("is_upvoted")),
                    is_downvoted: Some(row.get("is_downvoted")),
                    user_nickname: row.get("user_nickname"),
                    user_profile_image: row.get("user_profile_image"),
                };

                ContentRecord::Reply(reply_record)
            }
            _ => {
                return Err(DatabaseError::QueryError(format!(
                    "Unknown content type: {}",
                    content_type
                )));
            }
        };

        Ok(Some((content_record, is_blocked)))
    }

    async fn get_replies_by_post_id_with_metadata_and_block_status(
        &self,
        post_id: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<(KReplyRecord, bool)>> {
        let post_id_bytes = Self::decode_hex_to_bytes(post_id)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        let mut bind_count = 1;
        let mut cursor_conditions = String::new();

        // Add cursor logic to the limited_replies CTE
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (r.block_time < ${} OR (r.block_time = ${} AND r.id < ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (r.block_time > ${} OR (r.block_time = ${} AND r.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY r.block_time DESC, r.id DESC"
        } else {
            " ORDER BY r.block_time ASC, r.id ASC"
        };

        let final_order_clause = if options.sort_descending {
            " ORDER BY rs.block_time DESC, rs.id DESC"
        } else {
            " ORDER BY rs.block_time ASC, rs.id ASC"
        };

        let query = format!(
            r#"
            WITH limited_replies AS (
                -- Get limited replies for specific post first to reduce data volume
                SELECT r.id, r.transaction_id, r.block_time, r.sender_pubkey,
                       r.sender_signature, r.post_id, r.base64_encoded_message
                FROM k_replies r
                WHERE r.post_id = $1{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ),
            reply_stats AS (
                -- Pre-aggregate metadata only for limited replies
                SELECT
                    lr.id, lr.transaction_id, lr.block_time, lr.sender_pubkey,
                    lr.sender_signature, lr.post_id, lr.base64_encoded_message,

                    -- Replies count (nested replies to this reply)
                    COALESCE(r.replies_count, 0) as replies_count,

                    -- Vote statistics
                    COALESCE(v.up_votes_count, 0) as up_votes_count,
                    COALESCE(v.down_votes_count, 0) as down_votes_count,
                    COALESCE(v.user_upvoted, false) as is_upvoted,
                    COALESCE(v.user_downvoted, false) as is_downvoted

                FROM limited_replies lr

                -- Optimized replies aggregation with EXISTS filter (nested replies)
                LEFT JOIN (
                    SELECT post_id, COUNT(*) as replies_count
                    FROM k_replies r
                    WHERE EXISTS (SELECT 1 FROM limited_replies lr WHERE lr.transaction_id = r.post_id)
                    GROUP BY post_id
                ) r ON lr.transaction_id = r.post_id

                -- Optimized vote aggregation with EXISTS filter and combined user vote
                LEFT JOIN (
                    SELECT
                        post_id,
                        COUNT(*) FILTER (WHERE vote = 'upvote') as up_votes_count,
                        COUNT(*) FILTER (WHERE vote = 'downvote') as down_votes_count,
                        bool_or(vote = 'upvote' AND sender_pubkey = ${requester_param}) as user_upvoted,
                        bool_or(vote = 'downvote' AND sender_pubkey = ${requester_param}) as user_downvoted
                    FROM k_votes v
                    WHERE EXISTS (SELECT 1 FROM limited_replies lr WHERE lr.transaction_id = v.post_id)
                    GROUP BY post_id
                ) v ON lr.transaction_id = v.post_id
            )
            SELECT
                rs.id, rs.transaction_id, rs.block_time, rs.sender_pubkey,
                rs.sender_signature, rs.post_id, rs.base64_encoded_message,

                -- Get mentioned pubkeys efficiently with subquery
                COALESCE(
                    ARRAY(
                        SELECT encode(m.mentioned_pubkey, 'hex')
                        FROM k_mentions m
                        WHERE m.content_id = rs.transaction_id AND m.content_type = 'reply'
                    ),
                    '{{}}'::text[]
                ) as mentioned_pubkeys,

                rs.replies_count,
                rs.up_votes_count,
                rs.down_votes_count,
                rs.is_upvoted,
                rs.is_downvoted,

                -- User profile lookup with LATERAL join
                COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                b.base64_encoded_profile_image as user_profile_image,

                -- Blocking status check
                CASE
                    WHEN kb.blocked_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_blocked

            FROM reply_stats rs
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = rs.sender_pubkey
                ORDER BY b.block_time DESC
                LIMIT 1
            ) b ON true
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = ${requester_param} AND kb.blocked_user_pubkey = rs.sender_pubkey
            WHERE 1=1
            {final_order_clause}
            "#,
            cursor_conditions = cursor_conditions,
            order_clause = order_clause,
            final_order_clause = final_order_clause,
            limit_param = bind_count + 1,
            requester_param = bind_count + 2
        );

        // Build query with parameter binding
        let mut query_builder = sqlx::query(&query).bind(&post_id_bytes);

        // Add cursor parameters if present
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder.bind(before_timestamp as i64).bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder.bind(after_timestamp as i64).bind(after_id);
            }
        }

        query_builder = query_builder
            .bind(offset_limit)
            .bind(&requester_pubkey_bytes);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let has_more = rows.len() > limit as usize;
        let actual_items = if has_more {
            rows.into_iter().take(limit as usize).collect::<Vec<_>>()
        } else {
            rows.into_iter().collect::<Vec<_>>()
        };

        let mut replies_with_block_status = Vec::new();
        for row in actual_items {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let post_id: Vec<u8> = row.get("post_id");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");
            let is_blocked: bool = row.get("is_blocked");

            let reply_record = KReplyRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                post_id: Self::encode_bytes_to_hex(&post_id),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
                replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                is_upvoted: Some(row.get("is_upvoted")),
                is_downvoted: Some(row.get("is_downvoted")),
                user_nickname: Some(row.get("user_nickname")),
                user_profile_image: row.get("user_profile_image"),
            };

            replies_with_block_status.push((reply_record, is_blocked));
        }

        // Extract just the replies for pagination metadata calculation
        let replies: Vec<KReplyRecord> = replies_with_block_status
            .iter()
            .map(|(reply, _)| reply.clone())
            .collect();
        let pagination = self.create_compound_pagination_metadata(&replies, limit as u32, has_more);

        Ok(PaginatedResult {
            items: replies_with_block_status,
            pagination,
        })
    }

    async fn get_replies_by_user_with_metadata_and_block_status(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<(KReplyRecord, bool)>> {
        let user_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        let mut bind_count = 1;
        let mut cursor_conditions = String::new();

        // Add cursor logic to the limited_replies CTE
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (r.block_time < ${} OR (r.block_time = ${} AND r.id < ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (r.block_time > ${} OR (r.block_time = ${} AND r.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY r.block_time DESC, r.id DESC"
        } else {
            " ORDER BY r.block_time ASC, r.id ASC"
        };

        let final_order_clause = if options.sort_descending {
            " ORDER BY rs.block_time DESC, rs.id DESC"
        } else {
            " ORDER BY rs.block_time ASC, rs.id ASC"
        };

        let query = format!(
            r#"
            WITH limited_replies AS (
                -- Get limited replies for specific user first to reduce data volume
                SELECT r.id, r.transaction_id, r.block_time, r.sender_pubkey,
                       r.sender_signature, r.post_id, r.base64_encoded_message
                FROM k_replies r
                WHERE r.sender_pubkey = $1{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ),
            reply_stats AS (
                -- Pre-aggregate metadata only for limited replies
                SELECT
                    lr.id, lr.transaction_id, lr.block_time, lr.sender_pubkey,
                    lr.sender_signature, lr.post_id, lr.base64_encoded_message,

                    -- Replies count (nested replies to this reply)
                    COALESCE(r.replies_count, 0) as replies_count,

                    -- Vote statistics
                    COALESCE(v.up_votes_count, 0) as up_votes_count,
                    COALESCE(v.down_votes_count, 0) as down_votes_count,
                    COALESCE(v.user_upvoted, false) as is_upvoted,
                    COALESCE(v.user_downvoted, false) as is_downvoted

                FROM limited_replies lr

                -- Optimized replies aggregation with EXISTS filter (nested replies)
                LEFT JOIN (
                    SELECT post_id, COUNT(*) as replies_count
                    FROM k_replies r
                    WHERE EXISTS (SELECT 1 FROM limited_replies lr WHERE lr.transaction_id = r.post_id)
                    GROUP BY post_id
                ) r ON lr.transaction_id = r.post_id

                -- Optimized vote aggregation with EXISTS filter and combined user vote
                LEFT JOIN (
                    SELECT
                        post_id,
                        COUNT(*) FILTER (WHERE vote = 'upvote') as up_votes_count,
                        COUNT(*) FILTER (WHERE vote = 'downvote') as down_votes_count,
                        bool_or(vote = 'upvote' AND sender_pubkey = ${requester_param}) as user_upvoted,
                        bool_or(vote = 'downvote' AND sender_pubkey = ${requester_param}) as user_downvoted
                    FROM k_votes v
                    WHERE EXISTS (SELECT 1 FROM limited_replies lr WHERE lr.transaction_id = v.post_id)
                    GROUP BY post_id
                ) v ON lr.transaction_id = v.post_id
            )
            SELECT
                rs.id, rs.transaction_id, rs.block_time, rs.sender_pubkey,
                rs.sender_signature, rs.post_id, rs.base64_encoded_message,

                -- Get mentioned pubkeys efficiently with subquery
                COALESCE(
                    ARRAY(
                        SELECT encode(m.mentioned_pubkey, 'hex')
                        FROM k_mentions m
                        WHERE m.content_id = rs.transaction_id AND m.content_type = 'reply'
                    ),
                    '{{}}'::text[]
                ) as mentioned_pubkeys,

                rs.replies_count,
                rs.up_votes_count,
                rs.down_votes_count,
                rs.is_upvoted,
                rs.is_downvoted,

                -- User profile lookup with LATERAL join
                COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                b.base64_encoded_profile_image as user_profile_image,

                -- Blocking status check
                CASE
                    WHEN kb.blocked_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_blocked

            FROM reply_stats rs
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = rs.sender_pubkey
                ORDER BY b.block_time DESC
                LIMIT 1
            ) b ON true
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = ${requester_param} AND kb.blocked_user_pubkey = rs.sender_pubkey
            WHERE 1=1
            {final_order_clause}
            "#,
            cursor_conditions = cursor_conditions,
            order_clause = order_clause,
            final_order_clause = final_order_clause,
            limit_param = bind_count + 1,
            requester_param = bind_count + 2
        );

        // Build query with parameter binding
        let mut query_builder = sqlx::query(&query).bind(&user_pubkey_bytes);

        // Add cursor parameters if present
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder.bind(before_timestamp as i64).bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder.bind(after_timestamp as i64).bind(after_id);
            }
        }

        query_builder = query_builder
            .bind(offset_limit)
            .bind(&requester_pubkey_bytes);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let has_more = rows.len() > limit as usize;
        let actual_items = if has_more {
            rows.into_iter().take(limit as usize).collect::<Vec<_>>()
        } else {
            rows.into_iter().collect::<Vec<_>>()
        };

        let mut replies_with_block_status = Vec::new();
        for row in actual_items {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let post_id: Vec<u8> = row.get("post_id");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");
            let is_blocked: bool = row.get("is_blocked");

            let reply_record = KReplyRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                post_id: Self::encode_bytes_to_hex(&post_id),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
                replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                is_upvoted: Some(row.get("is_upvoted")),
                is_downvoted: Some(row.get("is_downvoted")),
                user_nickname: Some(row.get("user_nickname")),
                user_profile_image: row.get("user_profile_image"),
            };

            replies_with_block_status.push((reply_record, is_blocked));
        }

        // Extract just the replies for pagination metadata calculation
        let replies: Vec<KReplyRecord> = replies_with_block_status
            .iter()
            .map(|(reply, _)| reply.clone())
            .collect();
        let pagination = self.create_compound_pagination_metadata(&replies, limit as u32, has_more);

        Ok(PaginatedResult {
            items: replies_with_block_status,
            pagination,
        })
    }

    async fn get_posts_by_user_with_metadata_and_block_status(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<(KPostRecord, bool)>> {
        let user_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        let mut bind_count = 1;
        let mut cursor_conditions = String::new();

        // Add cursor logic to the all_posts CTE
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (p.block_time < ${} OR (p.block_time = ${} AND p.id < ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (p.block_time > ${} OR (p.block_time = ${} AND p.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY p.block_time DESC, p.id DESC"
        } else {
            " ORDER BY p.block_time ASC, p.id ASC"
        };

        let final_order_clause = if options.sort_descending {
            " ORDER BY ps.block_time DESC, ps.id DESC"
        } else {
            " ORDER BY ps.block_time ASC, ps.id ASC"
        };

        let query = format!(
            r#"
            WITH all_posts AS (
                -- Get limited posts for specific user first to reduce data volume
                SELECT p.id, p.transaction_id, p.block_time, p.sender_pubkey,
                       p.sender_signature, p.base64_encoded_message
                FROM k_posts p
                WHERE p.sender_pubkey = $1{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ),
            post_stats AS (
                -- Pre-aggregate metadata only for limited posts
                SELECT
                    lp.id, lp.transaction_id, lp.block_time, lp.sender_pubkey,
                    lp.sender_signature, lp.base64_encoded_message,

                    -- Replies count (optimized with EXISTS)
                    COALESCE(r.replies_count, 0) as replies_count,

                    -- Vote statistics (optimized with EXISTS)
                    COALESCE(v.up_votes_count, 0) as up_votes_count,
                    COALESCE(v.down_votes_count, 0) as down_votes_count,
                    COALESCE(v.user_upvoted, false) as is_upvoted,
                    COALESCE(v.user_downvoted, false) as is_downvoted

                FROM all_posts lp

                -- Optimized replies aggregation with EXISTS filter
                LEFT JOIN (
                    SELECT post_id, COUNT(*) as replies_count
                    FROM k_replies r
                    WHERE EXISTS (SELECT 1 FROM all_posts lp WHERE lp.transaction_id = r.post_id)
                    GROUP BY post_id
                ) r ON lp.transaction_id = r.post_id

                -- Optimized vote aggregation with EXISTS filter and combined user vote
                LEFT JOIN (
                    SELECT
                        post_id,
                        COUNT(*) FILTER (WHERE vote = 'upvote') as up_votes_count,
                        COUNT(*) FILTER (WHERE vote = 'downvote') as down_votes_count,
                        bool_or(vote = 'upvote' AND sender_pubkey = ${requester_param}) as user_upvoted,
                        bool_or(vote = 'downvote' AND sender_pubkey = ${requester_param}) as user_downvoted
                    FROM k_votes v
                    WHERE EXISTS (SELECT 1 FROM all_posts lp WHERE lp.transaction_id = v.post_id)
                    GROUP BY post_id
                ) v ON lp.transaction_id = v.post_id
            )
            SELECT
                ps.id, ps.transaction_id, ps.block_time, ps.sender_pubkey,
                ps.sender_signature, ps.base64_encoded_message,

                -- Get mentioned pubkeys efficiently with subquery
                COALESCE(
                    ARRAY(
                        SELECT encode(m.mentioned_pubkey, 'hex')
                        FROM k_mentions m
                        WHERE m.content_id = ps.transaction_id AND m.content_type = 'post'
                    ),
                    '{{}}'::text[]
                ) as mentioned_pubkeys,

                ps.replies_count,
                ps.up_votes_count,
                ps.down_votes_count,
                ps.is_upvoted,
                ps.is_downvoted,

                -- User profile lookup with LATERAL join
                COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                b.base64_encoded_profile_image as user_profile_image,

                -- Blocking status check
                CASE
                    WHEN kb.blocked_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_blocked

            FROM post_stats ps
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = ps.sender_pubkey
                ORDER BY b.block_time DESC
                LIMIT 1
            ) b ON true
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = ${requester_param} AND kb.blocked_user_pubkey = ps.sender_pubkey
            WHERE 1=1
            {final_order_clause}
            "#,
            cursor_conditions = cursor_conditions,
            order_clause = order_clause,
            final_order_clause = final_order_clause,
            limit_param = bind_count + 1,
            requester_param = bind_count + 2
        );

        // Build query with parameter binding
        let mut query_builder = sqlx::query(&query).bind(&user_pubkey_bytes);

        // Add cursor parameters if present
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder.bind(before_timestamp as i64).bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder.bind(after_timestamp as i64).bind(after_id);
            }
        }

        query_builder = query_builder
            .bind(offset_limit)
            .bind(&requester_pubkey_bytes);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let has_more = rows.len() > limit as usize;
        let actual_items = if has_more {
            rows.into_iter().take(limit as usize).collect::<Vec<_>>()
        } else {
            rows.into_iter().collect::<Vec<_>>()
        };

        let mut posts_with_block_status = Vec::new();
        for row in actual_items {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");
            let is_blocked: bool = row.get("is_blocked");

            let post_record = KPostRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
                replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                is_upvoted: Some(row.get("is_upvoted")),
                is_downvoted: Some(row.get("is_downvoted")),
                user_nickname: Some(row.get("user_nickname")),
                user_profile_image: row.get("user_profile_image"),
            };

            posts_with_block_status.push((post_record, is_blocked));
        }

        // Extract just the posts for pagination metadata calculation
        let posts: Vec<KPostRecord> = posts_with_block_status
            .iter()
            .map(|(post, _)| post.clone())
            .collect();
        let pagination = self.create_compound_pagination_metadata(&posts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: posts_with_block_status,
            pagination,
        })
    }

    async fn get_notification_count(
        &self,
        requester_pubkey: &str,
        after: Option<String>,
    ) -> DatabaseResult<u64> {
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;

        let count_result = if let Some(cursor_str) = after {
            // If after cursor is provided, count notifications since that cursor (excluding blocked users)
            if let Ok((cursor_timestamp, cursor_id)) = Self::parse_compound_cursor(&cursor_str) {
                sqlx::query_scalar::<_, i64>(
                    r#"
                    SELECT COUNT(*)
                    FROM (
                        SELECT 1
                        FROM k_mentions km
                        WHERE km.mentioned_pubkey = $1
                          AND km.sender_pubkey IS NOT NULL
                          AND km.sender_pubkey != $1
                          AND (km.block_time > $2 OR (km.block_time = $2 AND km.id > $3))
                          AND NOT EXISTS (
                              SELECT 1 FROM k_blocks kb
                              WHERE kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = km.sender_pubkey
                          )
                        ORDER BY km.block_time DESC, km.id DESC
                        LIMIT 31
                    ) recent_notifications
                    "#,
                )
                .bind(&requester_pubkey_bytes)
                .bind(cursor_timestamp as i64)
                .bind(cursor_id)
                .fetch_one(&self.pool)
                .await
            } else {
                return Err(DatabaseError::InvalidInput(
                    "Invalid cursor format".to_string(),
                ));
            }
        } else {
            // If no cursor is provided, count all notifications (excluding blocked users)
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)
                FROM (
                    SELECT 1
                    FROM k_mentions km
                    WHERE km.mentioned_pubkey = $1
                      AND km.sender_pubkey IS NOT NULL
                      AND km.sender_pubkey != $1
                      AND NOT EXISTS (
                          SELECT 1 FROM k_blocks kb
                          WHERE kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = km.sender_pubkey
                      )
                    ORDER BY km.block_time DESC, km.id DESC
                    LIMIT 31
                ) recent_notifications
                "#,
            )
            .bind(&requester_pubkey_bytes)
            .fetch_one(&self.pool)
            .await
        };

        match count_result {
            Ok(count) => Ok(count as u64),
            Err(e) => Err(DatabaseError::QueryError(format!(
                "Failed to count notifications: {}",
                e
            ))),
        }
    }

    async fn get_notifications_with_content_details(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<NotificationContentRecord>> {
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        // Build cursor conditions for optimized k_mentions filtering
        let mut cursor_conditions = String::new();
        let mut bind_count = 1;

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (km.block_time < ${} OR (km.block_time = ${} AND km.id < ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }
        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (km.block_time > ${} OR (km.block_time = ${} AND km.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let mentions_order_clause = if options.sort_descending {
            "ORDER BY km.block_time DESC, km.id DESC"
        } else {
            "ORDER BY km.block_time ASC, km.id ASC"
        };

        let final_order_clause = if options.sort_descending {
            "ORDER BY block_time DESC, mention_id DESC"
        } else {
            "ORDER BY block_time ASC, mention_id ASC"
        };
        let final_limit = format!("LIMIT ${}", bind_count + 1);

        // Optimized query: filter k_mentions first, then join with content details
        let query = format!(
            r#"
            WITH filtered_mentions AS (
                -- Step 1: Get recent mentions from non-blocked users (leverages comprehensive index)
                SELECT km.id as mention_id, km.content_id, km.content_type, km.block_time, km.sender_pubkey
                FROM k_mentions km
                WHERE km.mentioned_pubkey = $1
                  AND km.sender_pubkey IS NOT NULL
                  AND km.sender_pubkey != $1
                  AND NOT EXISTS (
                      SELECT 1 FROM k_blocks kb
                      WHERE kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = km.sender_pubkey
                  )
                {cursor_conditions}
                {mentions_order_clause}
                {final_limit}
            ),
            notifications_with_content AS (
                -- Step 2: Get content details only for filtered mentions
                SELECT
                    CASE fm.content_type
                        WHEN 'post' THEN p.id
                        WHEN 'reply' THEN r.id
                        WHEN 'vote' THEN v.id
                    END as id,
                    fm.content_id as transaction_id,
                    fm.block_time,
                    fm.sender_pubkey,
                    CASE fm.content_type
                        WHEN 'post' THEN p.base64_encoded_message
                        WHEN 'reply' THEN r.base64_encoded_message
                        WHEN 'vote' THEN ''
                    END as base64_encoded_message,
                    fm.mention_id,
                    COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                    b.base64_encoded_profile_image as user_profile_image,
                    fm.content_type,
                    -- Vote-specific fields
                    CASE WHEN fm.content_type = 'vote' THEN v.vote ELSE NULL END as vote_type,
                    CASE WHEN fm.content_type = 'vote' THEN v.block_time ELSE NULL END as vote_block_time,
                    CASE WHEN fm.content_type = 'vote' THEN encode(v.post_id, 'hex') ELSE NULL END as content_id,
                    CASE WHEN fm.content_type = 'vote' THEN COALESCE(vp.base64_encoded_message, vr.base64_encoded_message, '') ELSE NULL END as voted_content
                FROM filtered_mentions fm
                LEFT JOIN k_posts p ON fm.content_type = 'post' AND fm.content_id = p.transaction_id
                LEFT JOIN k_replies r ON fm.content_type = 'reply' AND fm.content_id = r.transaction_id
                LEFT JOIN k_votes v ON fm.content_type = 'vote' AND fm.content_id = v.transaction_id
                -- Get user profile for sender
                LEFT JOIN LATERAL (
                    SELECT base64_encoded_nickname, base64_encoded_profile_image
                    FROM k_broadcasts b
                    WHERE b.sender_pubkey = fm.sender_pubkey
                    ORDER BY b.block_time DESC LIMIT 1
                ) b ON true
                -- For votes, get the content being voted on
                LEFT JOIN k_posts vp ON fm.content_type = 'vote' AND v.post_id = vp.transaction_id
                LEFT JOIN k_replies vr ON fm.content_type = 'vote' AND v.post_id = vr.transaction_id
                {final_order_clause}
            )
            SELECT * FROM notifications_with_content
            "#,
            cursor_conditions = cursor_conditions,
            mentions_order_clause = mentions_order_clause,
            final_order_clause = final_order_clause,
            final_limit = final_limit
        );

        // Build query with parameter binding
        let mut query_builder = sqlx::query(&query).bind(&requester_pubkey_bytes);

        // Add cursor parameters if present
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder.bind(before_timestamp as i64).bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder.bind(after_timestamp as i64).bind(after_id);
            }
        }

        query_builder = query_builder
            .bind(offset_limit)
            .bind(&requester_pubkey_bytes);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let has_more = rows.len() > limit as usize;
        let actual_items = if has_more {
            rows.into_iter().take(limit as usize).collect::<Vec<_>>()
        } else {
            rows.into_iter().collect::<Vec<_>>()
        };

        let mut notifications = Vec::new();
        for row in actual_items {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let content_type: String = row.get("content_type");
            let mention_id: i64 = row.get("mention_id");
            let block_time: i64 = row.get("block_time");

            if content_type == "post" {
                let post_record = KPostRecord {
                    id: row.get::<i64, _>("id"),
                    transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                    block_time: row.get::<i64, _>("block_time") as u64,
                    sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                    sender_signature: String::new(),
                    base64_encoded_message: row.get("base64_encoded_message"),
                    mentioned_pubkeys: Vec::new(),
                    up_votes_count: None,
                    down_votes_count: None,
                    is_upvoted: None,
                    is_downvoted: None,
                    replies_count: None,
                    user_nickname: Some(row.get("user_nickname")),
                    user_profile_image: row.get("user_profile_image"),
                };

                notifications.push(NotificationContentRecord {
                    content: ContentRecord::Post(post_record),
                    mention_id,
                    mention_block_time: block_time as u64,
                });
            } else if content_type == "reply" {
                let reply_record = KReplyRecord {
                    id: row.get::<i64, _>("id"),
                    transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                    block_time: row.get::<i64, _>("block_time") as u64,
                    sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                    sender_signature: String::new(),
                    post_id: String::new(),
                    base64_encoded_message: row.get("base64_encoded_message"),
                    mentioned_pubkeys: Vec::new(),
                    replies_count: None,
                    up_votes_count: None,
                    down_votes_count: None,
                    is_upvoted: None,
                    is_downvoted: None,
                    user_nickname: Some(row.get("user_nickname")),
                    user_profile_image: row.get("user_profile_image"),
                };

                notifications.push(NotificationContentRecord {
                    content: ContentRecord::Reply(reply_record),
                    mention_id,
                    mention_block_time: block_time as u64,
                });
            } else if content_type == "vote" {
                let vote_record = KVoteRecord {
                    id: row.get::<i64, _>("id"),
                    transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                    block_time: row.get::<i64, _>("block_time") as u64,
                    sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                    sender_signature: String::new(),
                    post_id: row
                        .get::<Option<String>, _>("content_id")
                        .unwrap_or_default(),
                    vote: row
                        .get::<Option<String>, _>("vote_type")
                        .unwrap_or_default(),
                    mention_block_time: Some(row.get::<i64, _>("block_time") as u64), // Now k_mentions.block_time
                    voted_content: row.get::<Option<String>, _>("voted_content"),
                    user_nickname: Some(row.get("user_nickname")),
                    user_profile_image: row.get("user_profile_image"),
                };

                notifications.push(NotificationContentRecord {
                    content: ContentRecord::Vote(vote_record),
                    mention_id,
                    mention_block_time: block_time as u64,
                });
            }
        }

        // Generate pagination info
        let mut pagination = PaginationMetadata {
            has_more,
            next_cursor: None,
            prev_cursor: None,
        };

        if !notifications.is_empty() {
            let first_item = &notifications[0];
            let last_item = &notifications[notifications.len() - 1];

            // Use mention data for cursor generation
            pagination.prev_cursor = Some(Self::create_compound_cursor(
                first_item.mention_block_time,
                first_item.mention_id,
            ));

            pagination.next_cursor = Some(Self::create_compound_cursor(
                last_item.mention_block_time,
                last_item.mention_id,
            ));
        }

        Ok(PaginatedResult {
            items: notifications,
            pagination,
        })
    }
}
