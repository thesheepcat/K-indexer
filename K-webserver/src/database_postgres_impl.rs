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

    /// Get network type from k_vars table (internal implementation)
    async fn get_network_from_db(&self) -> Result<String, sqlx::Error> {
        let result = sqlx::query("SELECT value FROM k_vars WHERE key = 'network'")
            .fetch_optional(&self.pool)
            .await?;

        Ok(result
            .map(|row| row.get("value"))
            .unwrap_or_else(|| "unknown".to_string()))
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

    async fn get_all_users(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<(KBroadcastRecord, bool, bool)>> {
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
                END as is_blocked,
                CASE
                    WHEN kf.followed_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_followed
            FROM k_broadcasts b
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = b.sender_pubkey
            LEFT JOIN k_follows kf ON kf.sender_pubkey = $1 AND kf.followed_user_pubkey = b.sender_pubkey
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
            let is_followed: bool = row.get("is_followed");

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

            broadcasts_with_block_status.push((broadcast_record, is_blocked, is_followed));
        }

        let has_more = broadcasts_with_block_status.len() > limit as usize;
        if has_more {
            broadcasts_with_block_status.pop();
        }

        // Extract just the broadcast records for pagination metadata calculation
        let broadcast_records: Vec<KBroadcastRecord> = broadcasts_with_block_status
            .iter()
            .map(|(record, _, _)| record.clone())
            .collect();
        let pagination =
            self.create_compound_pagination_metadata(&broadcast_records, limit as u32, has_more);

        Ok(PaginatedResult {
            items: broadcasts_with_block_status,
            pagination,
        })
    }

    async fn get_most_active_users(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
        from_time_millis: u64,
        to_time_millis: u64,
    ) -> DatabaseResult<PaginatedResult<(KBroadcastRecord, bool, bool, i64)>> {
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1;

        let mut query = String::from(
            r#"
            WITH user_content_counts AS (
                SELECT sender_pubkey, COUNT(*) as content_count
                FROM k_contents
                WHERE block_time >= $2 AND block_time <= $3
                GROUP BY sender_pubkey
            )
            SELECT
                b.id, b.transaction_id, b.block_time, b.sender_pubkey, b.sender_signature,
                b.base64_encoded_nickname, b.base64_encoded_profile_image, b.base64_encoded_message,
                ucc.content_count,
                CASE
                    WHEN kb.blocked_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_blocked,
                CASE
                    WHEN kf.followed_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_followed
            FROM k_broadcasts b
            INNER JOIN user_content_counts ucc ON ucc.sender_pubkey = b.sender_pubkey
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = b.sender_pubkey
            LEFT JOIN k_follows kf ON kf.sender_pubkey = $1 AND kf.followed_user_pubkey = b.sender_pubkey
            WHERE 1=1
            "#,
        );

        // $1 = requester_pubkey, $2 = from_time_millis, $3 = to_time_millis
        let mut bind_count = 3;

        if let Some(before_cursor) = &options.before {
            if let Ok((before_count, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (ucc.content_count < ${} OR (ucc.content_count = ${} AND b.id < ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_count, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (ucc.content_count > ${} OR (ucc.content_count = ${} AND b.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        query.push_str(" ORDER BY ucc.content_count DESC, b.id DESC");

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query)
            .bind(&requester_pubkey_bytes)
            .bind(from_time_millis as i64)
            .bind(to_time_millis as i64);

        if let Some(before_cursor) = &options.before {
            if let Ok((before_count, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder.bind(before_count as i64).bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_count, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder.bind(after_count as i64).bind(after_id);
            }
        }

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder.fetch_all(&self.pool).await.map_err(|e| {
            DatabaseError::QueryError(format!("Failed to fetch most active users: {}", e))
        })?;

        let mut results: Vec<(KBroadcastRecord, bool, bool, i64)> = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let is_blocked: bool = row.get("is_blocked");
            let is_followed: bool = row.get("is_followed");
            let content_count: i64 = row.get("content_count");

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

            results.push((broadcast_record, is_blocked, is_followed, content_count));
        }

        let has_more = results.len() > limit as usize;
        if has_more {
            results.pop();
        }

        // Build pagination metadata using content_count as the cursor "timestamp" component
        let pagination = if results.is_empty() {
            PaginationMetadata {
                has_more,
                next_cursor: None,
                prev_cursor: None,
            }
        } else {
            let first = &results[0];
            let last = results.last().unwrap();

            let next_cursor = if has_more {
                Some(Self::create_compound_cursor(last.3 as u64, last.0.id))
            } else {
                None
            };

            let prev_cursor = Some(Self::create_compound_cursor(first.3 as u64, first.0.id));

            PaginationMetadata {
                has_more,
                next_cursor,
                prev_cursor,
            }
        };

        Ok(PaginatedResult {
            items: results,
            pagination,
        })
    }

    async fn search_users(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
        searched_user_pubkey: Option<String>,
        searched_user_nickname: Option<String>,
    ) -> DatabaseResult<PaginatedResult<(KBroadcastRecord, bool, bool)>> {
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
                END as is_blocked,
                CASE
                    WHEN kf.followed_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_followed
            FROM k_broadcasts b
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = b.sender_pubkey
            LEFT JOIN k_follows kf ON kf.sender_pubkey = $1 AND kf.followed_user_pubkey = b.sender_pubkey
            WHERE 1=1
            "#,
        );

        let mut bind_count = 1; // Start with 1 since we already have requester_pubkey
        let mut search_user_pubkey_bytes: Option<Vec<u8>> = None;

        // Add search filter for user pubkey (matches both 02 and 03 prefix variants)
        if let Some(ref pubkey) = searched_user_pubkey {
            search_user_pubkey_bytes = Some(Self::decode_hex_to_bytes(pubkey)?);
            bind_count += 1;
            query.push_str(&format!(
                " AND encode(b.sender_pubkey, 'hex') LIKE ${}",
                bind_count
            ));
        }

        // Add search filter for nickname (decode Base64 and search plain text)
        if let Some(_) = searched_user_nickname.as_ref() {
            bind_count += 1;
            query.push_str(&format!(
                " AND convert_from(decode(b.base64_encoded_nickname, 'base64'), 'UTF8') ILIKE ${}",
                bind_count
            ));
        }

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

        // Bind search user pubkey pattern if provided (matches both 02 and 03 prefix)
        if let Some(ref pubkey_bytes) = search_user_pubkey_bytes {
            let hex_pattern = format!("%{}", hex::encode(pubkey_bytes));
            query_builder = query_builder.bind(hex_pattern);
        }

        // Bind nickname search pattern if provided
        if let Some(ref nickname) = searched_user_nickname {
            let search_pattern = format!("%{}%", nickname);
            query_builder = query_builder.bind(search_pattern);
        }

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
            .map_err(|e| DatabaseError::QueryError(format!("Failed to search users: {}", e)))?;

        let mut broadcasts_with_block_status = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let is_blocked: bool = row.get("is_blocked");
            let is_followed: bool = row.get("is_followed");

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

            broadcasts_with_block_status.push((broadcast_record, is_blocked, is_followed));
        }

        let has_more = broadcasts_with_block_status.len() > limit as usize;
        if has_more {
            broadcasts_with_block_status.pop();
        }

        // Extract just the broadcast records for pagination metadata calculation
        let broadcast_records: Vec<KBroadcastRecord> = broadcasts_with_block_status
            .iter()
            .map(|(record, _, _)| record.clone())
            .collect();
        let pagination =
            self.create_compound_pagination_metadata(&broadcast_records, limit as u32, has_more);

        Ok(PaginatedResult {
            items: broadcasts_with_block_status,
            pagination,
        })
    }

    async fn get_user_details(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
    ) -> DatabaseResult<Option<(KBroadcastRecord, bool, bool, i64, i64, i64)>> {
        let user_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;

        // Single query to get broadcast data + block/follow status + follower counts
        let query = r#"
            SELECT
                b.id,
                b.transaction_id,
                b.block_time,
                b.sender_pubkey,
                b.sender_signature,
                b.base64_encoded_nickname,
                b.base64_encoded_profile_image,
                b.base64_encoded_message,
                EXISTS (
                    SELECT 1 FROM k_blocks kb
                    WHERE kb.sender_pubkey = $2 AND kb.blocked_user_pubkey = $1
                ) as is_blocked,
                EXISTS (
                    SELECT 1 FROM k_follows kf
                    WHERE kf.sender_pubkey = $2 AND kf.followed_user_pubkey = $1
                ) as is_followed,
                (SELECT COUNT(*) FROM k_follows WHERE followed_user_pubkey = $1) as followers_count,
                (SELECT COUNT(*) FROM k_follows WHERE sender_pubkey = $1) as following_count,
                (SELECT COUNT(*) FROM k_blocks WHERE sender_pubkey = $1) as blocked_count
            FROM k_broadcasts b
            WHERE b.sender_pubkey = $1
            LIMIT 1
        "#;

        let row_opt = sqlx::query(query)
            .bind(&user_pubkey_bytes)
            .bind(&requester_pubkey_bytes)
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
            let is_blocked: bool = row.get("is_blocked");
            let is_followed: bool = row.get("is_followed");
            let followers_count: i64 = row.get("followers_count");
            let following_count: i64 = row.get("following_count");
            let blocked_count: i64 = row.get("blocked_count");

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

            Ok(Some((
                broadcast_record,
                is_blocked,
                is_followed,
                followers_count,
                following_count,
                blocked_count,
            )))
        } else {
            // No broadcast data found, need separate query for block/follow status and counts
            let status_query = r#"
                SELECT
                    EXISTS (
                        SELECT 1 FROM k_blocks kb
                        WHERE kb.sender_pubkey = $2 AND kb.blocked_user_pubkey = $1
                    ) as is_blocked,
                    EXISTS (
                        SELECT 1 FROM k_follows kf
                        WHERE kf.sender_pubkey = $2 AND kf.followed_user_pubkey = $1
                    ) as is_followed,
                    (SELECT COUNT(*) FROM k_follows WHERE followed_user_pubkey = $1) as followers_count,
                    (SELECT COUNT(*) FROM k_follows WHERE sender_pubkey = $1) as following_count,
                    (SELECT COUNT(*) FROM k_blocks WHERE sender_pubkey = $1) as blocked_count
            "#;

            let status_row = sqlx::query(status_query)
                .bind(&user_pubkey_bytes)
                .bind(&requester_pubkey_bytes)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| {
                    DatabaseError::QueryError(format!("Failed to check block/follow status: {}", e))
                })?;

            let is_blocked: bool = status_row.get("is_blocked");
            let is_followed: bool = status_row.get("is_followed");
            let followers_count: i64 = status_row.get("followers_count");
            let following_count: i64 = status_row.get("following_count");
            let blocked_count: i64 = status_row.get("blocked_count");

            // Create a minimal broadcast record with empty fields and the status
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

            Ok(Some((
                broadcast_record,
                is_blocked,
                is_followed,
                followers_count,
                following_count,
                blocked_count,
            )))
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

    async fn get_followed_users_by_requester(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KBroadcastRecord>> {
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1;

        let mut query = String::from(
            r#"
            SELECT kf.id, kf.transaction_id, kf.block_time, kf.followed_user_pubkey as sender_pubkey, kf.sender_signature,
                   COALESCE(b.base64_encoded_nickname, '') as base64_encoded_nickname,
                   b.base64_encoded_profile_image,
                   COALESCE(b.base64_encoded_message, '') as base64_encoded_message
            FROM k_follows kf
            LEFT JOIN k_broadcasts b ON b.sender_pubkey = kf.followed_user_pubkey
            WHERE kf.sender_pubkey = $1
            "#,
        );

        let mut bind_count = 1;

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (kf.block_time < ${} OR (kf.block_time = ${} AND kf.id < ${}))",
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
                    " AND (kf.block_time > ${} OR (kf.block_time = ${} AND kf.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if options.sort_descending {
            query.push_str(" ORDER BY kf.block_time DESC, kf.id DESC");
        } else {
            query.push_str(" ORDER BY kf.block_time ASC, kf.id ASC");
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
            DatabaseError::QueryError(format!(
                "Failed to fetch followed users by requester: {}",
                e
            ))
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

    async fn get_users_following(
        &self,
        requester_pubkey: &str,
        user_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<(KBroadcastRecord, bool)>> {
        let user_pubkey_bytes = Self::decode_hex_to_bytes(user_pubkey)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1;

        let mut query = String::from(
            r#"
            SELECT kf.id, kf.transaction_id, kf.block_time, kf.followed_user_pubkey as sender_pubkey, kf.sender_signature,
                   COALESCE(b.base64_encoded_nickname, '') as base64_encoded_nickname,
                   b.base64_encoded_profile_image,
                   COALESCE(b.base64_encoded_message, '') as base64_encoded_message,
                   CASE WHEN kf2.id IS NOT NULL THEN true ELSE false END as is_followed_by_requester
            FROM k_follows kf
            LEFT JOIN k_broadcasts b ON b.sender_pubkey = kf.followed_user_pubkey
            LEFT JOIN k_follows kf2 ON kf2.sender_pubkey = $2 AND kf2.followed_user_pubkey = kf.followed_user_pubkey
            WHERE kf.sender_pubkey = $1
            "#,
        );

        let mut bind_count = 2;

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (kf.block_time < ${} OR (kf.block_time = ${} AND kf.id < ${}))",
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
                    " AND (kf.block_time > ${} OR (kf.block_time = ${} AND kf.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if options.sort_descending {
            query.push_str(" ORDER BY kf.block_time DESC, kf.id DESC");
        } else {
            query.push_str(" ORDER BY kf.block_time ASC, kf.id ASC");
        }

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query);
        query_builder = query_builder
            .bind(&user_pubkey_bytes)
            .bind(&requester_pubkey_bytes);

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
            DatabaseError::QueryError(format!("Failed to fetch users following: {}", e))
        })?;

        let mut broadcasts_with_follow_status = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let is_followed_by_requester: bool = row.get("is_followed_by_requester");

            let broadcast = KBroadcastRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_nickname: row.get("base64_encoded_nickname"),
                base64_encoded_profile_image: row.get("base64_encoded_profile_image"),
                base64_encoded_message: row.get("base64_encoded_message"),
            };

            broadcasts_with_follow_status.push((broadcast, is_followed_by_requester));
        }

        let has_more = broadcasts_with_follow_status.len() > limit as usize;
        if has_more {
            broadcasts_with_follow_status.pop();
        }

        // Extract just the broadcasts for pagination metadata
        let broadcasts: Vec<_> = broadcasts_with_follow_status
            .iter()
            .map(|(b, _)| b.clone())
            .collect();
        let pagination =
            self.create_compound_pagination_metadata(&broadcasts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: broadcasts_with_follow_status,
            pagination,
        })
    }

    async fn get_users_followers(
        &self,
        requester_pubkey: &str,
        user_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<(KBroadcastRecord, bool)>> {
        let user_pubkey_bytes = Self::decode_hex_to_bytes(user_pubkey)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1;

        let mut query = String::from(
            r#"
            SELECT kf.id, kf.transaction_id, kf.block_time, kf.sender_pubkey, kf.sender_signature,
                   COALESCE(b.base64_encoded_nickname, '') as base64_encoded_nickname,
                   b.base64_encoded_profile_image,
                   COALESCE(b.base64_encoded_message, '') as base64_encoded_message,
                   CASE WHEN kf2.id IS NOT NULL THEN true ELSE false END as is_followed_by_requester
            FROM k_follows kf
            LEFT JOIN k_broadcasts b ON b.sender_pubkey = kf.sender_pubkey
            LEFT JOIN k_follows kf2 ON kf2.sender_pubkey = $2 AND kf2.followed_user_pubkey = kf.sender_pubkey
            WHERE kf.followed_user_pubkey = $1
            "#,
        );

        let mut bind_count = 2;

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (kf.block_time < ${} OR (kf.block_time = ${} AND kf.id < ${}))",
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
                    " AND (kf.block_time > ${} OR (kf.block_time = ${} AND kf.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        if options.sort_descending {
            query.push_str(" ORDER BY kf.block_time DESC, kf.id DESC");
        } else {
            query.push_str(" ORDER BY kf.block_time ASC, kf.id ASC");
        }

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query);
        query_builder = query_builder
            .bind(&user_pubkey_bytes)
            .bind(&requester_pubkey_bytes);

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
            DatabaseError::QueryError(format!("Failed to fetch users followers: {}", e))
        })?;

        let mut broadcasts_with_follow_status = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let is_followed_by_requester: bool = row.get("is_followed_by_requester");

            let broadcast = KBroadcastRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_nickname: row.get("base64_encoded_nickname"),
                base64_encoded_profile_image: row.get("base64_encoded_profile_image"),
                base64_encoded_message: row.get("base64_encoded_message"),
            };

            broadcasts_with_follow_status.push((broadcast, is_followed_by_requester));
        }

        let has_more = broadcasts_with_follow_status.len() > limit as usize;
        if has_more {
            broadcasts_with_follow_status.pop();
        }

        // Extract just the broadcasts for pagination metadata
        let broadcasts: Vec<_> = broadcasts_with_follow_status
            .iter()
            .map(|(b, _)| b.clone())
            .collect();
        let pagination =
            self.create_compound_pagination_metadata(&broadcasts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: broadcasts_with_follow_status,
            pagination,
        })
    }

    // Optimized single-query method for get-posts-watching API with blocking awareness

    async fn get_all_posts(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>> {
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
                    " AND (c.block_time < ${} OR (c.block_time = ${} AND c.id < ${}))",
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
                    " AND (c.block_time > ${} OR (c.block_time = ${} AND c.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY c.block_time DESC, c.id DESC"
        } else {
            " ORDER BY c.block_time ASC, c.id ASC"
        };

        let final_order_clause = if options.sort_descending {
            " ORDER BY ps.block_time DESC, ps.id DESC"
        } else {
            " ORDER BY ps.block_time ASC, ps.id ASC"
        };

        let query = format!(
            r#"
            WITH all_posts AS (
                SELECT c.id, c.transaction_id, c.block_time, c.sender_pubkey,
                       c.sender_signature, c.base64_encoded_message, c.content_type,
                       c.referenced_content_id
                FROM k_contents c
                LEFT JOIN k_blocks kb ON kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = c.sender_pubkey
                WHERE c.content_type IN ('post', 'quote')
                  AND kb.blocked_user_pubkey IS NULL{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ), post_stats AS (
                SELECT lp.id, lp.transaction_id, lp.block_time, lp.sender_pubkey,
                       lp.sender_signature, lp.base64_encoded_message, lp.content_type,
                       lp.referenced_content_id,
                       COALESCE(r.replies_count, 0) as replies_count,
                       COALESCE(q.quotes_count, 0) as quotes_count,
                       COALESCE(v.up_votes_count, 0) as up_votes_count,
                       COALESCE(v.down_votes_count, 0) as down_votes_count,
                       COALESCE(v.user_upvoted, false) as is_upvoted,
                       COALESCE(v.user_downvoted, false) as is_downvoted
                FROM all_posts lp
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as replies_count
                    FROM k_contents r
                    WHERE r.content_type = 'reply'
                      AND EXISTS (SELECT 1 FROM all_posts lp WHERE lp.transaction_id = r.referenced_content_id)
                    GROUP BY referenced_content_id
                ) r ON lp.transaction_id = r.referenced_content_id
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as quotes_count
                    FROM k_contents qt
                    WHERE qt.content_type = 'quote'
                      AND EXISTS (SELECT 1 FROM all_posts lp WHERE lp.transaction_id = qt.referenced_content_id)
                    GROUP BY referenced_content_id
                ) q ON lp.transaction_id = q.referenced_content_id
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
                                  WHERE m.content_id = ps.transaction_id AND m.content_type IN ('post', 'quote')), '{{}}') as mentioned_pubkeys,
                   ps.replies_count, ps.quotes_count, ps.up_votes_count, ps.down_votes_count,
                   ps.is_upvoted, ps.is_downvoted,
                   COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                   b.base64_encoded_profile_image as user_profile_image,
                   encode(ps.referenced_content_id, 'hex') as referenced_content_id,
                   ref_c.base64_encoded_message as referenced_message,
                   encode(ref_c.sender_pubkey, 'hex') as referenced_sender_pubkey,
                   COALESCE(ref_b.base64_encoded_nickname, '') as referenced_nickname,
                   ref_b.base64_encoded_profile_image as referenced_profile_image
            FROM post_stats ps
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = ps.sender_pubkey
                LIMIT 1
            ) b ON true
            LEFT JOIN LATERAL (
                SELECT base64_encoded_message, sender_pubkey
                FROM k_contents
                WHERE transaction_id = ps.referenced_content_id
                  AND ps.content_type IN ('reply', 'quote')
                LIMIT 1
            ) ref_c ON true
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts
                WHERE sender_pubkey = ref_c.sender_pubkey
                LIMIT 1
            ) ref_b ON ref_c.sender_pubkey IS NOT NULL
            WHERE 1=1
            {final_order_clause}
            "#,
            cursor_conditions = cursor_conditions,
            order_clause = order_clause,
            final_order_clause = final_order_clause,
            limit_param = bind_count + 1
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

        let mut posts = Vec::new();
        for row in actual_items {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            let post_record = KPostRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
                content_type: None,
                replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                quotes_count: Some(row.get::<i64, _>("quotes_count") as u64),
                up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                is_upvoted: Some(row.get("is_upvoted")),
                is_downvoted: Some(row.get("is_downvoted")),
                user_nickname: Some(row.get("user_nickname")),
                user_profile_image: row.get("user_profile_image"),
                referenced_content_id: row.get("referenced_content_id"),
                referenced_message: row.get("referenced_message"),
                referenced_sender_pubkey: row.get("referenced_sender_pubkey"),
                referenced_nickname: row.get("referenced_nickname"),
                referenced_profile_image: row.get("referenced_profile_image"),
            };

            posts.push(post_record);
        }

        let pagination = self.create_compound_pagination_metadata(&posts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: posts,
            pagination,
        })
    }

    async fn get_content_following(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>> {
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        let mut bind_count = 1;
        let mut cursor_conditions = String::new();

        // Add cursor logic
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (c.block_time < ${} OR (c.block_time = ${} AND c.id < ${}))",
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
                    " AND (c.block_time > ${} OR (c.block_time = ${} AND c.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY c.block_time DESC, c.id DESC"
        } else {
            " ORDER BY c.block_time ASC, c.id ASC"
        };

        let final_order_clause = if options.sort_descending {
            " ORDER BY ps.block_time DESC, ps.id DESC"
        } else {
            " ORDER BY ps.block_time ASC, ps.id ASC"
        };

        let query = format!(
            r#"
            WITH followed_content AS (
                SELECT c.id, c.transaction_id, c.block_time, c.sender_pubkey,
                       c.sender_signature, c.base64_encoded_message, c.content_type,
                       c.referenced_content_id
                FROM k_contents c
                INNER JOIN k_follows kf ON kf.followed_user_pubkey = c.sender_pubkey
                LEFT JOIN k_blocks kb ON kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = c.sender_pubkey
                WHERE kf.sender_pubkey = $1
                  AND c.content_type IN ('post', 'reply', 'quote')
                  AND kb.blocked_user_pubkey IS NULL{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ), content_stats AS (
                SELECT fc.id, fc.transaction_id, fc.block_time, fc.sender_pubkey,
                       fc.sender_signature, fc.base64_encoded_message, fc.content_type,
                       fc.referenced_content_id,
                       COALESCE(r.replies_count, 0) as replies_count,
                       COALESCE(q.quotes_count, 0) as quotes_count,
                       COALESCE(v.up_votes_count, 0) as up_votes_count,
                       COALESCE(v.down_votes_count, 0) as down_votes_count,
                       COALESCE(v.user_upvoted, false) as is_upvoted,
                       COALESCE(v.user_downvoted, false) as is_downvoted
                FROM followed_content fc
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as replies_count
                    FROM k_contents r
                    WHERE r.content_type = 'reply'
                    GROUP BY referenced_content_id
                ) r ON fc.transaction_id = r.referenced_content_id
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as quotes_count
                    FROM k_contents qt
                    WHERE qt.content_type = 'quote'
                    GROUP BY referenced_content_id
                ) q ON fc.transaction_id = q.referenced_content_id
                LEFT JOIN (
                    SELECT post_id,
                           COUNT(*) FILTER (WHERE vote = 'upvote') as up_votes_count,
                           COUNT(*) FILTER (WHERE vote = 'downvote') as down_votes_count,
                           bool_or(vote = 'upvote' AND sender_pubkey = $1) as user_upvoted,
                           bool_or(vote = 'downvote' AND sender_pubkey = $1) as user_downvoted
                    FROM k_votes v
                    GROUP BY post_id
                ) v ON fc.transaction_id = v.post_id
            )
            SELECT ps.id, ps.transaction_id, ps.block_time, ps.sender_pubkey,
                   ps.sender_signature, ps.base64_encoded_message, ps.content_type,
                   COALESCE(ARRAY(SELECT encode(m.mentioned_pubkey, 'hex') FROM k_mentions m
                                  WHERE m.content_id = ps.transaction_id AND m.content_type = ps.content_type), '{{}}') as mentioned_pubkeys,
                   ps.replies_count, ps.quotes_count, ps.up_votes_count, ps.down_votes_count,
                   ps.is_upvoted, ps.is_downvoted,
                   COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                   b.base64_encoded_profile_image as user_profile_image,
                   encode(ps.referenced_content_id, 'hex') as referenced_content_id,
                   ref_c.base64_encoded_message as referenced_message,
                   encode(ref_c.sender_pubkey, 'hex') as referenced_sender_pubkey,
                   COALESCE(ref_b.base64_encoded_nickname, '') as referenced_nickname,
                   ref_b.base64_encoded_profile_image as referenced_profile_image
            FROM content_stats ps
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = ps.sender_pubkey
                LIMIT 1
            ) b ON true
            LEFT JOIN LATERAL (
                SELECT base64_encoded_message, sender_pubkey
                FROM k_contents
                WHERE transaction_id = ps.referenced_content_id
                  AND ps.content_type = 'quote'
                LIMIT 1
            ) ref_c ON true
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts
                WHERE sender_pubkey = ref_c.sender_pubkey
                LIMIT 1
            ) ref_b ON ref_c.sender_pubkey IS NOT NULL
            WHERE 1=1
            {final_order_clause}
            "#,
            cursor_conditions = cursor_conditions,
            order_clause = order_clause,
            final_order_clause = final_order_clause,
            limit_param = bind_count + 1
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

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder.fetch_all(&self.pool).await.map_err(|e| {
            DatabaseError::QueryError(format!("Failed to fetch followed content: {}", e))
        })?;

        // Process results and build pagination
        let mut items = Vec::new();
        let mut has_more = false;

        for (index, row) in rows.iter().enumerate() {
            if index >= limit as usize {
                has_more = true;
                break;
            }

            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_raw: Vec<String> = row.get("mentioned_pubkeys");

            let referenced_content_id: Option<String> = row.try_get("referenced_content_id").ok();
            let referenced_message: Option<String> = row.try_get("referenced_message").ok();
            let referenced_sender_pubkey: Option<String> =
                row.try_get("referenced_sender_pubkey").ok();
            let referenced_nickname: Option<String> = row.try_get("referenced_nickname").ok();
            let referenced_profile_image: Option<String> =
                row.try_get("referenced_profile_image").ok();

            let record = KPostRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_raw,
                content_type: row.try_get("content_type").ok(),
                replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                quotes_count: Some(row.get::<i64, _>("quotes_count") as u64),
                is_upvoted: Some(row.get("is_upvoted")),
                is_downvoted: Some(row.get("is_downvoted")),
                user_nickname: Some(row.get("user_nickname")),
                user_profile_image: row.get("user_profile_image"),
                referenced_content_id,
                referenced_message,
                referenced_sender_pubkey,
                referenced_nickname,
                referenced_profile_image,
            };

            items.push(record);
        }

        // Build pagination metadata
        let pagination = if items.is_empty() {
            PaginationMetadata {
                has_more: false,
                next_cursor: None,
                prev_cursor: None,
            }
        } else {
            let first_item = items.first().unwrap();
            let last_item = items.last().unwrap();

            let next_cursor = if has_more {
                Some(Self::create_compound_cursor(
                    last_item.block_time,
                    last_item.id,
                ))
            } else {
                None
            };

            let prev_cursor = Some(Self::create_compound_cursor(
                first_item.block_time,
                first_item.id,
            ));

            PaginationMetadata {
                has_more,
                next_cursor,
                prev_cursor,
            }
        };

        Ok(PaginatedResult { items, pagination })
    }

    async fn get_contents_mentioning_user(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<ContentRecord>> {
        let mentioned_user_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        let mut bind_count = 1;
        let mut cursor_conditions = String::new();

        // Add cursor logic for unified content table
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (c.block_time < ${} OR (c.block_time = ${} AND c.id < ${}))",
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
                    " AND (c.block_time > ${} OR (c.block_time = ${} AND c.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY c.block_time DESC, c.id DESC"
        } else {
            " ORDER BY c.block_time ASC, c.id ASC"
        };

        let cs_final_order_clause = if options.sort_descending {
            " ORDER BY cs.block_time DESC, cs.id DESC"
        } else {
            " ORDER BY cs.block_time ASC, cs.id ASC"
        };

        let query = format!(
            r#"
            WITH mentioned_content AS (
                -- Get content (posts, quotes, and replies) that mention the specific user
                SELECT c.content_type, c.id, c.transaction_id, c.block_time, c.sender_pubkey,
                       c.sender_signature, c.base64_encoded_message, c.referenced_content_id
                FROM k_contents c
                LEFT JOIN k_blocks kb ON kb.sender_pubkey = ${requester_param} AND kb.blocked_user_pubkey = c.sender_pubkey
                WHERE EXISTS (
                    SELECT 1
                    FROM k_mentions m
                    WHERE m.mentioned_pubkey = $1
                      AND m.content_id = c.transaction_id
                      AND m.content_type = c.content_type
                )
                  AND kb.blocked_user_pubkey IS NULL{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ),
            content_stats AS (
                -- Pre-aggregate all metadata in one pass
                SELECT
                    mc.content_type, mc.id, mc.transaction_id, mc.block_time, mc.sender_pubkey,
                    mc.sender_signature, mc.base64_encoded_message, mc.referenced_content_id,

                    -- Replies count (only applicable for posts and quotes, not replies)
                    CASE WHEN mc.content_type IN ('post', 'quote') THEN COALESCE(r.replies_count, 0) ELSE 0 END as replies_count,

                    -- Quotes count (only applicable for posts and quotes, not replies)
                    CASE WHEN mc.content_type IN ('post', 'quote') THEN COALESCE(q.quotes_count, 0) ELSE 0 END as quotes_count,

                    -- Vote statistics
                    COALESCE(v.up_votes_count, 0) as up_votes_count,
                    COALESCE(v.down_votes_count, 0) as down_votes_count,
                    COALESCE(v.user_upvoted, false) as is_upvoted,
                    COALESCE(v.user_downvoted, false) as is_downvoted

                FROM mentioned_content mc

                -- Optimized replies aggregation (only for posts and quotes)
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as replies_count
                    FROM k_contents r
                    WHERE r.content_type = 'reply'
                      AND EXISTS (SELECT 1 FROM mentioned_content mc WHERE mc.content_type IN ('post', 'quote') AND mc.transaction_id = r.referenced_content_id)
                    GROUP BY referenced_content_id
                ) r ON mc.content_type IN ('post', 'quote') AND mc.transaction_id = r.referenced_content_id

                -- Optimized quotes aggregation (only for posts and quotes)
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as quotes_count
                    FROM k_contents qt
                    WHERE qt.content_type = 'quote'
                      AND EXISTS (SELECT 1 FROM mentioned_content mc WHERE mc.content_type IN ('post', 'quote') AND mc.transaction_id = qt.referenced_content_id)
                    GROUP BY referenced_content_id
                ) q ON mc.content_type IN ('post', 'quote') AND mc.transaction_id = q.referenced_content_id

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
                cs.sender_signature, cs.base64_encoded_message, cs.referenced_content_id,

                -- Get mentioned pubkeys efficiently
                COALESCE(
                    ARRAY(
                        SELECT encode(m.mentioned_pubkey, 'hex')
                        FROM k_mentions m
                        WHERE m.content_id = cs.transaction_id AND m.content_type = cs.content_type
                    ),
                    '{{}}'::text[]
                ) as mentioned_pubkeys,

                cs.replies_count,
                cs.quotes_count,
                cs.up_votes_count,
                cs.down_votes_count,
                cs.is_upvoted,
                cs.is_downvoted,

                -- User profile lookup with efficient filtering
                COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                b.base64_encoded_profile_image as user_profile_image,

                -- Quote reference data
                encode(ref_c.transaction_id, 'hex') as ref_transaction_id,
                ref_c.base64_encoded_message as ref_message,
                encode(ref_c.sender_pubkey, 'hex') as ref_sender_pubkey,
                COALESCE(ref_b.base64_encoded_nickname, '') as ref_nickname,
                ref_b.base64_encoded_profile_image as ref_profile_image

            FROM content_stats cs
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = cs.sender_pubkey
                LIMIT 1
            ) b ON true
            LEFT JOIN LATERAL (
                SELECT transaction_id, base64_encoded_message, sender_pubkey
                FROM k_contents
                WHERE transaction_id = cs.referenced_content_id
                  AND cs.content_type = 'quote'
                LIMIT 1
            ) ref_c ON true
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts
                WHERE sender_pubkey = ref_c.sender_pubkey
                LIMIT 1
            ) ref_b ON ref_c.sender_pubkey IS NOT NULL
            WHERE 1=1
            {cs_final_order_clause}
            "#,
            cursor_conditions = cursor_conditions,
            order_clause = order_clause,
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

        let mut content_records = Vec::new();
        for row in actual_items {
            let content_type: &str = row.get("content_type");
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            let content_record = match content_type {
                "post" | "quote" => {
                    let post_record = KPostRecord {
                        id: row.get::<i64, _>("id"),
                        transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                        block_time: row.get::<i64, _>("block_time") as u64,
                        sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                        sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                        base64_encoded_message: row.get("base64_encoded_message"),
                        mentioned_pubkeys: mentioned_pubkeys_array,
                        content_type: None,
                        replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                        quotes_count: Some(row.get::<i64, _>("quotes_count") as u64),
                        up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                        down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                        is_upvoted: Some(row.get("is_upvoted")),
                        is_downvoted: Some(row.get("is_downvoted")),
                        user_nickname: Some(row.get("user_nickname")),
                        user_profile_image: row.get("user_profile_image"),
                        referenced_content_id: row.get("ref_transaction_id"),
                        referenced_message: row.get("ref_message"),
                        referenced_sender_pubkey: row.get("ref_sender_pubkey"),
                        referenced_nickname: row.get("ref_nickname"),
                        referenced_profile_image: row.get("ref_profile_image"),
                    };
                    ContentRecord::Post(post_record)
                }
                "reply" => {
                    let referenced_content_id: Option<Vec<u8>> = row.get("referenced_content_id");
                    let post_id_hex = match referenced_content_id {
                        Some(bytes) => Self::encode_bytes_to_hex(&bytes),
                        None => {
                            return Err(DatabaseError::QueryError(
                                "Missing referenced_content_id for reply".to_string(),
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
                        content_type: None,
                        replies_count: Some(0), // Replies don't have replies
                        quotes_count: None,
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

            content_records.push(content_record);
        }

        let pagination =
            self.create_compound_pagination_metadata(&content_records, limit as u32, has_more);

        Ok(PaginatedResult {
            items: content_records,
            pagination,
        })
    }

    async fn get_content_by_id(
        &self,
        content_id: &str,
        requester_pubkey: &str,
    ) -> DatabaseResult<Option<(ContentRecord, bool)>> {
        let content_id_bytes = Self::decode_hex_to_bytes(content_id)?;
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;

        let query = r#"
            SELECT
                c.content_type,
                c.id,
                c.transaction_id,
                c.block_time,
                c.sender_pubkey,
                c.sender_signature,
                c.referenced_content_id,
                c.base64_encoded_message,
                COALESCE(
                    ARRAY(
                        SELECT m.mentioned_pubkey
                        FROM k_mentions m
                        WHERE m.content_id = c.transaction_id AND m.content_type = c.content_type
                    ),
                    ARRAY[]::bytea[]
                ) as mentioned_pubkeys,
                COALESCE(reply_counts.replies_count, 0) as replies_count,
                COALESCE(quote_counts.quotes_count, 0) as quotes_count,
                COALESCE(vote_counts.up_votes_count, 0) as up_votes_count,
                COALESCE(vote_counts.down_votes_count, 0) as down_votes_count,
                COALESCE(user_vote.is_upvoted, false) as is_upvoted,
                COALESCE(user_vote.is_downvoted, false) as is_downvoted,
                user_profile.base64_encoded_nickname as user_nickname,
                user_profile.base64_encoded_profile_image as user_profile_image,
                encode(c.referenced_content_id, 'hex') as ref_content_id,
                ref_c.base64_encoded_message as referenced_message,
                encode(ref_c.sender_pubkey, 'hex') as referenced_sender_pubkey,
                ref_b.base64_encoded_nickname as referenced_nickname,
                ref_b.base64_encoded_profile_image as referenced_profile_image,
                CASE
                    WHEN kb.blocked_user_pubkey IS NOT NULL THEN true
                    ELSE false
                END as is_blocked
            FROM k_contents c
            LEFT JOIN (
                SELECT referenced_content_id, COUNT(*) as replies_count
                FROM k_contents
                WHERE content_type = 'reply'
                GROUP BY referenced_content_id
            ) reply_counts ON c.transaction_id = reply_counts.referenced_content_id
            LEFT JOIN (
                SELECT referenced_content_id, COUNT(*) as quotes_count
                FROM k_contents
                WHERE content_type = 'quote'
                GROUP BY referenced_content_id
            ) quote_counts ON c.transaction_id = quote_counts.referenced_content_id
            LEFT JOIN (
                SELECT
                    post_id,
                    COUNT(*) FILTER (WHERE vote = 'upvote') as up_votes_count,
                    COUNT(*) FILTER (WHERE vote = 'downvote') as down_votes_count
                FROM k_votes
                GROUP BY post_id
            ) vote_counts ON c.transaction_id = vote_counts.post_id
            LEFT JOIN (
                SELECT
                    post_id,
                    sender_pubkey,
                    bool_or(vote = 'upvote') as is_upvoted,
                    bool_or(vote = 'downvote') as is_downvoted
                FROM k_votes
                WHERE sender_pubkey = $2
                GROUP BY post_id, sender_pubkey
            ) user_vote ON c.transaction_id = user_vote.post_id
            LEFT JOIN (
                SELECT DISTINCT ON (sender_pubkey)
                    sender_pubkey,
                    base64_encoded_nickname,
                    base64_encoded_profile_image
                FROM k_broadcasts
                ORDER BY sender_pubkey
            ) user_profile ON c.sender_pubkey = user_profile.sender_pubkey
            LEFT JOIN LATERAL (
                SELECT base64_encoded_message, sender_pubkey
                FROM k_contents
                WHERE transaction_id = c.referenced_content_id
                  AND c.content_type = 'quote'
                LIMIT 1
            ) ref_c ON true
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts
                WHERE sender_pubkey = ref_c.sender_pubkey
                LIMIT 1
            ) ref_b ON ref_c.sender_pubkey IS NOT NULL
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = $2 AND kb.blocked_user_pubkey = c.sender_pubkey
            WHERE c.transaction_id = $1
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
            "post" | "quote" => {
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
                    content_type: None,
                    replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                    quotes_count: Some(row.get::<i64, _>("quotes_count") as u64),
                    up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                    down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                    is_upvoted: Some(row.get("is_upvoted")),
                    is_downvoted: Some(row.get("is_downvoted")),
                    user_nickname: row.get("user_nickname"),
                    user_profile_image: row.get("user_profile_image"),
                    referenced_content_id: row.get("ref_content_id"),
                    referenced_message: row.get("referenced_message"),
                    referenced_sender_pubkey: row.get("referenced_sender_pubkey"),
                    referenced_nickname: row.get("referenced_nickname"),
                    referenced_profile_image: row.get("referenced_profile_image"),
                };

                ContentRecord::Post(post_record)
            }
            "reply" => {
                let mentioned_pubkeys_bytes: Vec<Vec<u8>> = row.get("mentioned_pubkeys");
                let mentioned_pubkeys: Vec<String> = mentioned_pubkeys_bytes
                    .into_iter()
                    .map(|bytes| hex::encode(bytes))
                    .collect();

                let referenced_content_id: Option<Vec<u8>> = row.get("referenced_content_id");
                let post_id = match referenced_content_id {
                    Some(bytes) => hex::encode(bytes),
                    None => {
                        return Err(DatabaseError::QueryError(
                            "Missing referenced_content_id for reply".to_string(),
                        ));
                    }
                };

                let reply_record = KReplyRecord {
                    id: row.get("id"),
                    transaction_id: hex::encode(row.get::<Vec<u8>, _>("transaction_id")),
                    block_time: row.get::<i64, _>("block_time") as u64,
                    sender_pubkey: hex::encode(row.get::<Vec<u8>, _>("sender_pubkey")),
                    sender_signature: hex::encode(row.get::<Vec<u8>, _>("sender_signature")),
                    post_id,
                    base64_encoded_message: row.get("base64_encoded_message"),
                    mentioned_pubkeys,
                    content_type: None,
                    replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                    quotes_count: Some(row.get::<i64, _>("quotes_count") as u64),
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

    async fn get_replies_by_post_id(
        &self,
        post_id: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KReplyRecord>> {
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
                    " AND (c.block_time < ${} OR (c.block_time = ${} AND c.id < ${}))",
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
                    " AND (c.block_time > ${} OR (c.block_time = ${} AND c.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY c.block_time DESC, c.id DESC"
        } else {
            " ORDER BY c.block_time ASC, c.id ASC"
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
                SELECT c.id, c.transaction_id, c.block_time, c.sender_pubkey,
                       c.sender_signature, c.referenced_content_id, c.base64_encoded_message
                FROM k_contents c
                LEFT JOIN k_blocks kb ON kb.sender_pubkey = ${requester_param} AND kb.blocked_user_pubkey = c.sender_pubkey
                WHERE c.content_type = 'reply'
                  AND c.referenced_content_id = $1
                  AND kb.blocked_user_pubkey IS NULL{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ),
            reply_stats AS (
                -- Pre-aggregate metadata only for limited replies
                SELECT
                    lr.id, lr.transaction_id, lr.block_time, lr.sender_pubkey,
                    lr.sender_signature, lr.referenced_content_id, lr.base64_encoded_message,

                    -- Replies count (nested replies to this reply)
                    COALESCE(r.replies_count, 0) as replies_count,

                    -- Quotes count
                    COALESCE(q.quotes_count, 0) as quotes_count,

                    -- Vote statistics
                    COALESCE(v.up_votes_count, 0) as up_votes_count,
                    COALESCE(v.down_votes_count, 0) as down_votes_count,
                    COALESCE(v.user_upvoted, false) as is_upvoted,
                    COALESCE(v.user_downvoted, false) as is_downvoted

                FROM limited_replies lr

                -- Optimized replies aggregation with EXISTS filter (nested replies)
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as replies_count
                    FROM k_contents r
                    WHERE r.content_type = 'reply'
                      AND EXISTS (SELECT 1 FROM limited_replies lr WHERE lr.transaction_id = r.referenced_content_id)
                    GROUP BY referenced_content_id
                ) r ON lr.transaction_id = r.referenced_content_id

                -- Optimized quotes aggregation with EXISTS filter
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as quotes_count
                    FROM k_contents qt
                    WHERE qt.content_type = 'quote'
                      AND EXISTS (SELECT 1 FROM limited_replies lr WHERE lr.transaction_id = qt.referenced_content_id)
                    GROUP BY referenced_content_id
                ) q ON lr.transaction_id = q.referenced_content_id

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
                rs.sender_signature, rs.referenced_content_id, rs.base64_encoded_message,

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
                rs.quotes_count,
                rs.up_votes_count,
                rs.down_votes_count,
                rs.is_upvoted,
                rs.is_downvoted,

                -- User profile lookup with LATERAL join
                COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                b.base64_encoded_profile_image as user_profile_image

            FROM reply_stats rs
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = rs.sender_pubkey
                LIMIT 1
            ) b ON true
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

        let mut replies = Vec::new();
        for row in actual_items {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let referenced_content_id: Vec<u8> = row.get("referenced_content_id");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            let reply_record = KReplyRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                post_id: Self::encode_bytes_to_hex(&referenced_content_id),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
                content_type: None,
                replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                quotes_count: Some(row.get::<i64, _>("quotes_count") as u64),
                up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                is_upvoted: Some(row.get("is_upvoted")),
                is_downvoted: Some(row.get("is_downvoted")),
                user_nickname: Some(row.get("user_nickname")),
                user_profile_image: row.get("user_profile_image"),
            };

            replies.push(reply_record);
        }

        let pagination = self.create_compound_pagination_metadata(&replies, limit as u32, has_more);

        Ok(PaginatedResult {
            items: replies,
            pagination,
        })
    }

    async fn get_replies_by_user(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KReplyRecord>> {
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
                    " AND (c.block_time < ${} OR (c.block_time = ${} AND c.id < ${}))",
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
                    " AND (c.block_time > ${} OR (c.block_time = ${} AND c.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY c.block_time DESC, c.id DESC"
        } else {
            " ORDER BY c.block_time ASC, c.id ASC"
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
                SELECT c.id, c.transaction_id, c.block_time, c.sender_pubkey,
                       c.sender_signature, c.referenced_content_id, c.base64_encoded_message
                FROM k_contents c
                LEFT JOIN k_blocks kb ON kb.sender_pubkey = ${requester_param} AND kb.blocked_user_pubkey = c.sender_pubkey
                WHERE c.content_type = 'reply'
                  AND c.sender_pubkey = $1
                  AND kb.blocked_user_pubkey IS NULL{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ),
            reply_stats AS (
                -- Pre-aggregate metadata only for limited replies
                SELECT
                    lr.id, lr.transaction_id, lr.block_time, lr.sender_pubkey,
                    lr.sender_signature, lr.referenced_content_id, lr.base64_encoded_message,

                    -- Replies count (nested replies to this reply)
                    COALESCE(r.replies_count, 0) as replies_count,

                    -- Quotes count
                    COALESCE(q.quotes_count, 0) as quotes_count,

                    -- Vote statistics
                    COALESCE(v.up_votes_count, 0) as up_votes_count,
                    COALESCE(v.down_votes_count, 0) as down_votes_count,
                    COALESCE(v.user_upvoted, false) as is_upvoted,
                    COALESCE(v.user_downvoted, false) as is_downvoted

                FROM limited_replies lr

                -- Optimized replies aggregation with EXISTS filter (nested replies)
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as replies_count
                    FROM k_contents r
                    WHERE r.content_type = 'reply'
                      AND EXISTS (SELECT 1 FROM limited_replies lr WHERE lr.transaction_id = r.referenced_content_id)
                    GROUP BY referenced_content_id
                ) r ON lr.transaction_id = r.referenced_content_id

                -- Optimized quotes aggregation with EXISTS filter
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as quotes_count
                    FROM k_contents qt
                    WHERE qt.content_type = 'quote'
                      AND EXISTS (SELECT 1 FROM limited_replies lr WHERE lr.transaction_id = qt.referenced_content_id)
                    GROUP BY referenced_content_id
                ) q ON lr.transaction_id = q.referenced_content_id

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
                rs.sender_signature, rs.referenced_content_id, rs.base64_encoded_message,

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
                rs.quotes_count,
                rs.up_votes_count,
                rs.down_votes_count,
                rs.is_upvoted,
                rs.is_downvoted,

                -- User profile lookup with LATERAL join
                COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                b.base64_encoded_profile_image as user_profile_image

            FROM reply_stats rs
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = rs.sender_pubkey
                LIMIT 1
            ) b ON true
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

        let mut replies = Vec::new();
        for row in actual_items {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let referenced_content_id: Vec<u8> = row.get("referenced_content_id");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            let reply_record = KReplyRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                post_id: Self::encode_bytes_to_hex(&referenced_content_id),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
                content_type: None,
                replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                quotes_count: Some(row.get::<i64, _>("quotes_count") as u64),
                up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                is_upvoted: Some(row.get("is_upvoted")),
                is_downvoted: Some(row.get("is_downvoted")),
                user_nickname: Some(row.get("user_nickname")),
                user_profile_image: row.get("user_profile_image"),
            };

            replies.push(reply_record);
        }

        let pagination = self.create_compound_pagination_metadata(&replies, limit as u32, has_more);

        Ok(PaginatedResult {
            items: replies,
            pagination,
        })
    }

    async fn get_posts_by_user(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>> {
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
                    " AND (c.block_time < ${} OR (c.block_time = ${} AND c.id < ${}))",
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
                    " AND (c.block_time > ${} OR (c.block_time = ${} AND c.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY c.block_time DESC, c.id DESC"
        } else {
            " ORDER BY c.block_time ASC, c.id ASC"
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
                SELECT c.id, c.transaction_id, c.block_time, c.sender_pubkey,
                       c.sender_signature, c.base64_encoded_message, c.content_type,
                       c.referenced_content_id
                FROM k_contents c
                WHERE c.content_type IN ('post', 'quote') AND c.sender_pubkey = $1{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ),
            post_stats AS (
                -- Pre-aggregate metadata only for limited posts
                SELECT
                    lp.id, lp.transaction_id, lp.block_time, lp.sender_pubkey,
                    lp.sender_signature, lp.base64_encoded_message, lp.content_type,
                    lp.referenced_content_id,

                    -- Replies count (optimized with EXISTS)
                    COALESCE(r.replies_count, 0) as replies_count,

                    -- Quotes count (optimized with EXISTS)
                    COALESCE(q.quotes_count, 0) as quotes_count,

                    -- Vote statistics (optimized with EXISTS)
                    COALESCE(v.up_votes_count, 0) as up_votes_count,
                    COALESCE(v.down_votes_count, 0) as down_votes_count,
                    COALESCE(v.user_upvoted, false) as is_upvoted,
                    COALESCE(v.user_downvoted, false) as is_downvoted

                FROM all_posts lp

                -- Optimized replies aggregation with EXISTS filter
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as replies_count
                    FROM k_contents r
                    WHERE r.content_type = 'reply'
                      AND EXISTS (SELECT 1 FROM all_posts lp WHERE lp.transaction_id = r.referenced_content_id)
                    GROUP BY referenced_content_id
                ) r ON lp.transaction_id = r.referenced_content_id

                -- Optimized quotes aggregation with EXISTS filter
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as quotes_count
                    FROM k_contents qt
                    WHERE qt.content_type = 'quote'
                      AND EXISTS (SELECT 1 FROM all_posts lp WHERE lp.transaction_id = qt.referenced_content_id)
                    GROUP BY referenced_content_id
                ) q ON lp.transaction_id = q.referenced_content_id

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
                        WHERE m.content_id = ps.transaction_id AND m.content_type IN ('post', 'quote')
                    ),
                    '{{}}'::text[]
                ) as mentioned_pubkeys,

                ps.replies_count,
                ps.quotes_count,
                ps.up_votes_count,
                ps.down_votes_count,
                ps.is_upvoted,
                ps.is_downvoted,

                -- User profile lookup with LATERAL join
                COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                b.base64_encoded_profile_image as user_profile_image,

                -- Quote reference data
                encode(ps.referenced_content_id, 'hex') as referenced_content_id,
                ref_c.base64_encoded_message as referenced_message,
                encode(ref_c.sender_pubkey, 'hex') as referenced_sender_pubkey,
                COALESCE(ref_b.base64_encoded_nickname, '') as referenced_nickname,
                ref_b.base64_encoded_profile_image as referenced_profile_image

            FROM post_stats ps
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = ps.sender_pubkey
                LIMIT 1
            ) b ON true
            LEFT JOIN LATERAL (
                SELECT base64_encoded_message, sender_pubkey
                FROM k_contents
                WHERE transaction_id = ps.referenced_content_id
                  AND ps.content_type IN ('reply', 'quote')
                LIMIT 1
            ) ref_c ON true
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts
                WHERE sender_pubkey = ref_c.sender_pubkey
                LIMIT 1
            ) ref_b ON ref_c.sender_pubkey IS NOT NULL
            LEFT JOIN k_blocks kb ON kb.sender_pubkey = ${requester_param} AND kb.blocked_user_pubkey = ps.sender_pubkey
            WHERE kb.blocked_user_pubkey IS NULL
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

        let mut posts = Vec::new();
        for row in actual_items {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            let post_record = KPostRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
                content_type: None,
                replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                quotes_count: Some(row.get::<i64, _>("quotes_count") as u64),
                up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                is_upvoted: Some(row.get("is_upvoted")),
                is_downvoted: Some(row.get("is_downvoted")),
                user_nickname: Some(row.get("user_nickname")),
                user_profile_image: row.get("user_profile_image"),
                referenced_content_id: row.get("referenced_content_id"),
                referenced_message: row.get("referenced_message"),
                referenced_sender_pubkey: row.get("referenced_sender_pubkey"),
                referenced_nickname: row.get("referenced_nickname"),
                referenced_profile_image: row.get("referenced_profile_image"),
            };

            posts.push(post_record);
        }

        let pagination = self.create_compound_pagination_metadata(&posts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: posts,
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
                        SELECT km.block_time, km.id
                        FROM k_mentions km
                        WHERE km.mentioned_pubkey = $1
                          AND km.sender_pubkey IS NOT NULL
                          AND km.sender_pubkey != $1
                          AND (km.block_time > $2 OR (km.block_time = $2 AND km.id > $3))
                          AND NOT EXISTS (
                              SELECT 1 FROM k_blocks kb
                              WHERE kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = km.sender_pubkey
                          )
                        ORDER BY block_time DESC, id DESC
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
                    SELECT km.block_time, km.id
                    FROM k_mentions km
                    WHERE km.mentioned_pubkey = $1
                      AND km.sender_pubkey IS NOT NULL
                      AND km.sender_pubkey != $1
                      AND NOT EXISTS (
                          SELECT 1 FROM k_blocks kb
                          WHERE kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = km.sender_pubkey
                      )
                    ORDER BY block_time DESC, id DESC
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

    async fn get_notifications(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<NotificationContentRecord>> {
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        // Build cursor conditions for filtering
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

        let final_order_clause = if options.sort_descending {
            "ORDER BY block_time DESC, notification_id DESC"
        } else {
            "ORDER BY block_time ASC, notification_id ASC"
        };
        let final_limit = format!("LIMIT ${}", bind_count + 1);

        // Optimized query: get all notifications from k_mentions table
        let query = format!(
            r#"
            WITH filtered_notifications AS (
                SELECT km.id as notification_id, km.content_id, km.content_type, km.block_time, km.sender_pubkey,
                       kc.referenced_content_id,
                       CASE WHEN km.content_type = 'quote' THEN 'quote' ELSE 'mention' END as notification_type
                FROM k_mentions km
                LEFT JOIN k_contents kc ON km.content_type = 'quote' AND km.content_id = kc.transaction_id
                WHERE km.mentioned_pubkey = $1
                  AND km.sender_pubkey IS NOT NULL
                  AND km.sender_pubkey != $1
                  AND NOT EXISTS (
                      SELECT 1 FROM k_blocks kb
                      WHERE kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = km.sender_pubkey
                  )
                {cursor_conditions}
                {final_order_clause}
                {final_limit}
            ),
            notifications_with_content AS (
                -- Step 2: Get content details for all notifications
                SELECT
                    CASE fn.content_type
                        WHEN 'post' THEN c.id
                        WHEN 'reply' THEN c.id
                        WHEN 'quote' THEN c.id
                        WHEN 'vote' THEN v.id
                    END as id,
                    fn.content_id as transaction_id,
                    fn.block_time,
                    fn.sender_pubkey,
                    CASE fn.content_type
                        WHEN 'post' THEN c.base64_encoded_message
                        WHEN 'reply' THEN c.base64_encoded_message
                        WHEN 'quote' THEN c.base64_encoded_message
                        WHEN 'vote' THEN ''
                    END as base64_encoded_message,
                    fn.notification_id,
                    COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                    b.base64_encoded_profile_image as user_profile_image,
                    fn.content_type,
                    fn.notification_type,
                    -- Vote-specific fields
                    CASE WHEN fn.content_type = 'vote' THEN v.vote ELSE NULL END as vote_type,
                    CASE WHEN fn.content_type = 'vote' THEN v.block_time ELSE NULL END as vote_block_time,
                    CASE WHEN fn.content_type = 'vote' THEN encode(v.post_id, 'hex') ELSE NULL END as content_id,
                    CASE WHEN fn.content_type = 'vote' THEN COALESCE(vc.base64_encoded_message, '') ELSE NULL END as voted_content,
                    -- Quote-specific fields: the original content that was quoted
                    encode(fn.referenced_content_id, 'hex') as quoted_content_id,
                    CASE WHEN fn.notification_type = 'quote' THEN original.base64_encoded_message ELSE NULL END as quoted_content_message
                FROM filtered_notifications fn
                LEFT JOIN k_contents c ON fn.content_type IN ('post', 'reply', 'quote') AND fn.content_id = c.transaction_id AND c.content_type = fn.content_type
                LEFT JOIN k_votes v ON fn.content_type = 'vote' AND fn.content_id = v.transaction_id
                -- Get user profile for sender
                LEFT JOIN LATERAL (
                    SELECT base64_encoded_nickname, base64_encoded_profile_image
                    FROM k_broadcasts b
                    WHERE b.sender_pubkey = fn.sender_pubkey
                    LIMIT 1
                ) b ON true
                -- For votes, get the content being voted on
                LEFT JOIN k_contents vc ON fn.content_type = 'vote' AND v.post_id = vc.transaction_id
                -- For quotes, get the original content that was quoted
                LEFT JOIN k_contents original ON fn.notification_type = 'quote' AND fn.referenced_content_id = original.transaction_id
                {final_order_clause}
            )
            SELECT * FROM notifications_with_content
            "#,
            cursor_conditions = cursor_conditions,
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
            let notification_id: i64 = row.get("notification_id");
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
                    content_type: None,
                    up_votes_count: None,
                    down_votes_count: None,
                    is_upvoted: None,
                    is_downvoted: None,
                    replies_count: None,
                    quotes_count: None,
                    user_nickname: Some(row.get("user_nickname")),
                    user_profile_image: row.get("user_profile_image"),
                    referenced_content_id: None,
                    referenced_message: None,
                    referenced_sender_pubkey: None,
                    referenced_nickname: None,
                    referenced_profile_image: None,
                };

                notifications.push(NotificationContentRecord {
                    content: ContentRecord::Post(post_record),
                    mention_id: notification_id,
                    mention_block_time: block_time as u64,
                });
            } else if content_type == "quote" {
                // Handle quote notifications - someone quoted my content
                let quoted_content_id: Option<String> = row.get("quoted_content_id");
                let quoted_content_message: Option<String> = row.get("quoted_content_message");

                let post_record = KPostRecord {
                    id: row.get::<i64, _>("id"),
                    transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                    block_time: row.get::<i64, _>("block_time") as u64,
                    sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                    sender_signature: String::new(),
                    base64_encoded_message: row.get("base64_encoded_message"),
                    mentioned_pubkeys: Vec::new(),
                    content_type: None,
                    up_votes_count: None,
                    down_votes_count: None,
                    is_upvoted: None,
                    is_downvoted: None,
                    replies_count: None,
                    quotes_count: None,
                    user_nickname: Some(row.get("user_nickname")),
                    user_profile_image: row.get("user_profile_image"),
                    referenced_content_id: quoted_content_id,
                    referenced_message: quoted_content_message,
                    referenced_sender_pubkey: None,
                    referenced_nickname: None,
                    referenced_profile_image: None,
                };

                notifications.push(NotificationContentRecord {
                    content: ContentRecord::Post(post_record),
                    mention_id: notification_id,
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
                    content_type: None,
                    replies_count: None,
                    quotes_count: None,
                    up_votes_count: None,
                    down_votes_count: None,
                    is_upvoted: None,
                    is_downvoted: None,
                    user_nickname: Some(row.get("user_nickname")),
                    user_profile_image: row.get("user_profile_image"),
                };

                notifications.push(NotificationContentRecord {
                    content: ContentRecord::Reply(reply_record),
                    mention_id: notification_id,
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
                    mention_id: notification_id,
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

    async fn get_network(&self) -> DatabaseResult<String> {
        self.get_network_from_db()
            .await
            .map_err(|e| DatabaseError::QueryError(e.to_string()))
    }

    async fn get_users_count(&self) -> DatabaseResult<u64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM k_broadcasts
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let count: i64 = row.get("count");
        Ok(count as u64)
    }

    async fn get_stats(&self) -> DatabaseResult<crate::database_trait::DatabaseStats> {
        let row = sqlx::query(
            r#"
            SELECT
                (SELECT COUNT(*) FROM k_broadcasts) as broadcasts_count,
                (SELECT COUNT(*) FROM k_contents WHERE content_type = 'post') as posts_count,
                (SELECT COUNT(*) FROM k_contents WHERE content_type = 'reply') as replies_count,
                (SELECT COUNT(*) FROM k_contents WHERE content_type = 'quote') as quotes_count,
                (SELECT COUNT(*) FROM k_votes) as votes_count,
                (SELECT COUNT(*) FROM k_follows) as follows_count,
                (SELECT COUNT(*) FROM k_blocks) as blocks_count
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        Ok(crate::database_trait::DatabaseStats {
            broadcasts_count: row.get("broadcasts_count"),
            posts_count: row.get("posts_count"),
            replies_count: row.get("replies_count"),
            quotes_count: row.get("quotes_count"),
            votes_count: row.get("votes_count"),
            follows_count: row.get("follows_count"),
            blocks_count: row.get("blocks_count"),
        })
    }

    /// Get content (posts, replies, quotes) containing a specific hashtag
    async fn get_hashtag_content(
        &self,
        hashtag: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>> {
        let requester_pubkey_bytes = Self::decode_hex_to_bytes(requester_pubkey)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        let mut bind_count = 2; // $1 for requester_pubkey, $2 for hashtag
        let mut cursor_conditions = String::new();

        // Add cursor logic
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                cursor_conditions.push_str(&format!(
                    " AND (c.block_time < ${} OR (c.block_time = ${} AND c.id < ${}))",
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
                    " AND (c.block_time > ${} OR (c.block_time = ${} AND c.id > ${}))",
                    bind_count - 1,
                    bind_count - 1,
                    bind_count
                ));
            }
        }

        let order_clause = if options.sort_descending {
            " ORDER BY c.block_time DESC, c.id DESC"
        } else {
            " ORDER BY c.block_time ASC, c.id ASC"
        };

        let final_order_clause = if options.sort_descending {
            " ORDER BY ps.block_time DESC, ps.id DESC"
        } else {
            " ORDER BY ps.block_time ASC, ps.id ASC"
        };

        let query = format!(
            r#"
            WITH hashtag_content AS (
                SELECT c.id, c.transaction_id, c.block_time, c.sender_pubkey,
                       c.sender_signature, c.base64_encoded_message, c.content_type,
                       c.referenced_content_id
                FROM k_contents c
                INNER JOIN k_hashtags h ON h.content_id = c.transaction_id
                LEFT JOIN k_blocks kb ON kb.sender_pubkey = $1 AND kb.blocked_user_pubkey = c.sender_pubkey
                WHERE h.hashtag = $2
                  AND c.content_type IN ('post', 'reply', 'quote')
                  AND kb.blocked_user_pubkey IS NULL{cursor_conditions}
                {order_clause}
                LIMIT ${limit_param}
            ), content_stats AS (
                SELECT hc.id, hc.transaction_id, hc.block_time, hc.sender_pubkey,
                       hc.sender_signature, hc.base64_encoded_message, hc.content_type,
                       hc.referenced_content_id,
                       COALESCE(r.replies_count, 0) as replies_count,
                       COALESCE(q.quotes_count, 0) as quotes_count,
                       COALESCE(v.up_votes_count, 0) as up_votes_count,
                       COALESCE(v.down_votes_count, 0) as down_votes_count,
                       COALESCE(v.user_upvoted, false) as is_upvoted,
                       COALESCE(v.user_downvoted, false) as is_downvoted
                FROM hashtag_content hc
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as replies_count
                    FROM k_contents r
                    WHERE r.content_type = 'reply'
                    GROUP BY referenced_content_id
                ) r ON hc.transaction_id = r.referenced_content_id
                LEFT JOIN (
                    SELECT referenced_content_id, COUNT(*) as quotes_count
                    FROM k_contents qt
                    WHERE qt.content_type = 'quote'
                    GROUP BY referenced_content_id
                ) q ON hc.transaction_id = q.referenced_content_id
                LEFT JOIN (
                    SELECT post_id,
                           COUNT(*) FILTER (WHERE vote = 'upvote') as up_votes_count,
                           COUNT(*) FILTER (WHERE vote = 'downvote') as down_votes_count,
                           bool_or(vote = 'upvote' AND sender_pubkey = $1) as user_upvoted,
                           bool_or(vote = 'downvote' AND sender_pubkey = $1) as user_downvoted
                    FROM k_votes v
                    GROUP BY post_id
                ) v ON hc.transaction_id = v.post_id
            )
            SELECT ps.id, ps.transaction_id, ps.block_time, ps.sender_pubkey,
                   ps.sender_signature, ps.base64_encoded_message, ps.content_type,
                   COALESCE(ARRAY(SELECT encode(m.mentioned_pubkey, 'hex') FROM k_mentions m
                                  WHERE m.content_id = ps.transaction_id AND m.content_type = ps.content_type), '{{}}') as mentioned_pubkeys,
                   ps.replies_count, ps.quotes_count, ps.up_votes_count, ps.down_votes_count,
                   ps.is_upvoted, ps.is_downvoted,
                   COALESCE(b.base64_encoded_nickname, '') as user_nickname,
                   b.base64_encoded_profile_image as user_profile_image,
                   encode(ps.referenced_content_id, 'hex') as referenced_content_id,
                   ref_c.base64_encoded_message as referenced_message,
                   encode(ref_c.sender_pubkey, 'hex') as referenced_sender_pubkey,
                   COALESCE(ref_b.base64_encoded_nickname, '') as referenced_nickname,
                   ref_b.base64_encoded_profile_image as referenced_profile_image
            FROM content_stats ps
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts b
                WHERE b.sender_pubkey = ps.sender_pubkey
                LIMIT 1
            ) b ON true
            LEFT JOIN LATERAL (
                SELECT base64_encoded_message, sender_pubkey
                FROM k_contents
                WHERE transaction_id = ps.referenced_content_id
                  AND ps.content_type = 'quote'
                LIMIT 1
            ) ref_c ON true
            LEFT JOIN LATERAL (
                SELECT base64_encoded_nickname, base64_encoded_profile_image
                FROM k_broadcasts
                WHERE sender_pubkey = ref_c.sender_pubkey
                LIMIT 1
            ) ref_b ON ref_c.sender_pubkey IS NOT NULL
            WHERE 1=1
            {final_order_clause}
            "#,
            cursor_conditions = cursor_conditions,
            order_clause = order_clause,
            final_order_clause = final_order_clause,
            limit_param = bind_count + 1
        );

        // Build query with parameter binding
        let mut query_builder = sqlx::query(&query)
            .bind(&requester_pubkey_bytes)
            .bind(hashtag);

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

        let rows = query_builder.fetch_all(&self.pool).await.map_err(|e| {
            DatabaseError::QueryError(format!("Failed to fetch hashtag content: {}", e))
        })?;

        // Process results and build pagination
        let mut items = Vec::new();
        let mut has_more = false;

        for (index, row) in rows.iter().enumerate() {
            if index >= limit as usize {
                has_more = true;
                break;
            }

            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_raw: Vec<String> = row.get("mentioned_pubkeys");

            let referenced_content_id: Option<String> = row.try_get("referenced_content_id").ok();
            let referenced_message: Option<String> = row.try_get("referenced_message").ok();
            let referenced_sender_pubkey: Option<String> =
                row.try_get("referenced_sender_pubkey").ok();
            let referenced_nickname: Option<String> = row.try_get("referenced_nickname").ok();
            let referenced_profile_image: Option<String> =
                row.try_get("referenced_profile_image").ok();

            let record = KPostRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_raw,
                content_type: row.try_get("content_type").ok(),
                replies_count: Some(row.get::<i64, _>("replies_count") as u64),
                up_votes_count: Some(row.get::<i64, _>("up_votes_count") as u64),
                down_votes_count: Some(row.get::<i64, _>("down_votes_count") as u64),
                quotes_count: Some(row.get::<i64, _>("quotes_count") as u64),
                is_upvoted: Some(row.get("is_upvoted")),
                is_downvoted: Some(row.get("is_downvoted")),
                user_nickname: Some(row.get("user_nickname")),
                user_profile_image: row.get("user_profile_image"),
                referenced_content_id,
                referenced_message,
                referenced_sender_pubkey,
                referenced_nickname,
                referenced_profile_image,
            };

            items.push(record);
        }

        // Build pagination metadata
        let pagination = if items.is_empty() {
            PaginationMetadata {
                has_more: false,
                next_cursor: None,
                prev_cursor: None,
            }
        } else {
            let first_item = items.first().unwrap();
            let last_item = items.last().unwrap();

            let next_cursor = if has_more {
                Some(Self::create_compound_cursor(
                    last_item.block_time,
                    last_item.id,
                ))
            } else {
                None
            };

            let prev_cursor = Some(Self::create_compound_cursor(
                first_item.block_time,
                first_item.id,
            ));

            PaginationMetadata {
                has_more,
                next_cursor,
                prev_cursor,
            }
        };

        Ok(PaginatedResult { items, pagination })
    }

    /// Get trending hashtags within a time window
    async fn get_trending_hashtags(
        &self,
        from_time: u64,
        to_time: u64,
        limit: u32,
    ) -> DatabaseResult<Vec<(String, u64)>> {
        let query = r#"
            SELECT hashtag, COUNT(*) as usage_count
            FROM k_hashtags
            WHERE block_time >= $1 AND block_time <= $2
            GROUP BY hashtag
            ORDER BY usage_count DESC, hashtag ASC
            LIMIT $3
        "#;

        let rows = sqlx::query(query)
            .bind(from_time as i64)
            .bind(to_time as i64)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                DatabaseError::QueryError(format!("Failed to fetch trending hashtags: {}", e))
            })?;

        let trending_hashtags: Vec<(String, u64)> = rows
            .iter()
            .map(|row| {
                let hashtag: String = row.get("hashtag");
                let usage_count: i64 = row.get("usage_count");
                (hashtag, usage_count as u64)
            })
            .collect();

        Ok(trending_hashtags)
    }
}
