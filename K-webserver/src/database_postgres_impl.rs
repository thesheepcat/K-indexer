use async_trait::async_trait;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use anyhow::Result;

use crate::database_trait::{
    DatabaseError, DatabaseInterface, DatabaseResult, PaginatedResult, QueryOptions,
};
use crate::models::{KBroadcastRecord, KPostRecord, KReplyRecord, KVoteRecord, PaginationMetadata};

pub struct PostgresDbManager {
    pub pool: PgPool,
}

impl PostgresDbManager {
    pub async fn new(connection_string: &str, max_connections: u32) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(std::time::Duration::from_secs(30))
            .connect(connection_string)
            .await?;
        
        // Test the pool connection
        sqlx::query("SELECT 1")
            .fetch_one(&pool)
            .await?;
        
        Ok(Self { pool })
    }

    fn create_pagination_metadata<T>(
        &self,
        items: &[T],
        _limit: u32,
        has_more: bool,
    ) -> PaginationMetadata
    where
        T: HasTimestamp,
    {
        let next_cursor = if has_more && !items.is_empty() {
            Some(items.last().unwrap().get_timestamp().to_string())
        } else {
            None
        };

        let prev_cursor = if !items.is_empty() {
            Some(items.first().unwrap().get_timestamp().to_string())
        } else {
            None
        };

        PaginationMetadata {
            has_more,
            next_cursor,
            prev_cursor,
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
            Some(Self::create_compound_cursor(last_item.get_timestamp(), last_item.get_id()))
        } else {
            None
        };

        let prev_cursor = if !items.is_empty() {
            let first_item = items.first().unwrap();
            Some(Self::create_compound_cursor(first_item.get_timestamp(), first_item.get_id()))
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
        hex::decode(hex_str).map_err(|e| DatabaseError::InvalidInput(format!("Invalid hex string: {}", e)))
    }

    fn encode_bytes_to_hex(bytes: &[u8]) -> String {
        hex::encode(bytes)
    }


    fn parse_compound_cursor(cursor: &str) -> DatabaseResult<(u64, i64)> {
        if cursor.contains('_') {
            let parts: Vec<&str> = cursor.split('_').collect();
            if parts.len() == 2 {
                let timestamp = parts[0].parse::<u64>()
                    .map_err(|_| DatabaseError::InvalidInput("Invalid timestamp in cursor".to_string()))?;
                let id = parts[1].parse::<i64>()
                    .map_err(|_| DatabaseError::InvalidInput("Invalid ID in cursor".to_string()))?;
                return Ok((timestamp, id));
            }
        }
        // Fallback: treat as simple timestamp cursor for backward compatibility
        let timestamp = cursor.parse::<u64>()
            .map_err(|_| DatabaseError::InvalidInput("Invalid cursor format".to_string()))?;
        Ok((timestamp, i64::MAX)) // Use MAX to include all records with same timestamp
    }

    fn create_compound_cursor(timestamp: u64, id: i64) -> String {
        format!("{}_{}", timestamp, id)
    }
}

trait HasTimestamp {
    fn get_timestamp(&self) -> u64;
}

trait HasCompoundCursor {
    fn get_timestamp(&self) -> u64;
    fn get_id(&self) -> i64;
}

impl HasTimestamp for KPostRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
    }
}

impl HasCompoundCursor for KPostRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
    }
    
    fn get_id(&self) -> i64 {
        self.id
    }
}

impl HasTimestamp for KReplyRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
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

impl HasTimestamp for KBroadcastRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
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

impl HasTimestamp for KVoteRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
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

#[async_trait]
#[allow(unused_variables)]
impl DatabaseInterface for PostgresDbManager {
    // Post operations
    async fn get_posts_by_user(
        &self,
        user_public_key: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>> {
        let sender_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        let mut query = String::from(
            r#"
            SELECT p.id, p.transaction_id, p.block_time, p.sender_pubkey, p.sender_signature, 
                   p.base64_encoded_message, 
                   COALESCE(
                       array_agg(encode(m.mentioned_pubkey, 'hex')) FILTER (WHERE m.mentioned_pubkey IS NOT NULL),
                       '{}'::text[]
                   ) as mentioned_pubkeys
            FROM k_posts p
            LEFT JOIN k_mentions m ON p.transaction_id = m.content_id AND m.content_type = 'post'
            WHERE p.sender_pubkey = $1
            "#
        );

        let mut bind_count = 1;
        
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (p.block_time < ${} OR (p.block_time = ${} AND p.id < ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (p.block_time > ${} OR (p.block_time = ${} AND p.id > ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        query.push_str(" GROUP BY p.id, p.transaction_id, p.block_time, p.sender_pubkey, p.sender_signature, p.base64_encoded_message");

        if options.sort_descending {
            query.push_str(" ORDER BY p.block_time DESC, p.id DESC");
        } else {
            query.push_str(" ORDER BY p.block_time ASC, p.id ASC");
        }

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query).bind(&sender_pubkey_bytes);

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder
                    .bind(before_timestamp as i64)
                    .bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder
                    .bind(after_timestamp as i64)
                    .bind(after_id);
            }
        }

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(format!("Failed to fetch posts by user: {}", e)))?;

        let mut posts = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            posts.push(KPostRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
            });
        }

        let has_more = posts.len() > limit as usize;
        if has_more {
            posts.pop(); // Remove the extra item
        }

        let pagination = self.create_compound_pagination_metadata(&posts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: posts,
            pagination,
        })
    }

    async fn get_all_posts(
        &self,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>> {
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        let mut query = String::from(
            r#"
            SELECT p.id, p.transaction_id, p.block_time, p.sender_pubkey, p.sender_signature, 
                   p.base64_encoded_message,
                   COALESCE(
                       array_agg(encode(m.mentioned_pubkey, 'hex')) FILTER (WHERE m.mentioned_pubkey IS NOT NULL),
                       '{}'::text[]
                   ) as mentioned_pubkeys
            FROM k_posts p
            LEFT JOIN k_mentions m ON p.transaction_id = m.content_id AND m.content_type = 'post'
            WHERE 1=1
            "#
        );

        let mut bind_count = 0;
        
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (p.block_time < ${} OR (p.block_time = ${} AND p.id < ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (p.block_time > ${} OR (p.block_time = ${} AND p.id > ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        query.push_str(" GROUP BY p.id, p.transaction_id, p.block_time, p.sender_pubkey, p.sender_signature, p.base64_encoded_message");

        if options.sort_descending {
            query.push_str(" ORDER BY p.block_time DESC, p.id DESC");
        } else {
            query.push_str(" ORDER BY p.block_time ASC, p.id ASC");
        }

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query);

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder
                    .bind(before_timestamp as i64)
                    .bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder
                    .bind(after_timestamp as i64)
                    .bind(after_id);
            }
        }

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(format!("Failed to fetch all posts: {}", e)))?;

        let mut posts = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            posts.push(KPostRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
            });
        }

        let has_more = posts.len() > limit as usize;
        if has_more {
            posts.pop();
        }

        let pagination = self.create_compound_pagination_metadata(&posts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: posts,
            pagination,
        })
    }

    async fn get_post_by_id(&self, post_id: &str) -> DatabaseResult<Option<KPostRecord>> {
        let transaction_id_bytes = Self::decode_hex_to_bytes(post_id)?;

        let row = sqlx::query(
            r#"
            SELECT p.id, p.transaction_id, p.block_time, p.sender_pubkey, p.sender_signature, 
                   p.base64_encoded_message,
                   COALESCE(
                       array_agg(encode(m.mentioned_pubkey, 'hex')) FILTER (WHERE m.mentioned_pubkey IS NOT NULL),
                       '{}'::text[]
                   ) as mentioned_pubkeys
            FROM k_posts p
            LEFT JOIN k_mentions m ON p.transaction_id = m.content_id AND m.content_type = 'post'
            WHERE p.transaction_id = $1
            GROUP BY p.id, p.transaction_id, p.block_time, p.sender_pubkey, p.sender_signature, p.base64_encoded_message
            "#
        )
        .bind(&transaction_id_bytes)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("Failed to fetch post by id: {}", e)))?;

        if let Some(row) = row {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            Ok(Some(KPostRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
            }))
        } else {
            Ok(None)
        }
    }

    async fn get_posts_mentioning_user(
        &self,
        user_public_key: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>> {
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1; // Get one extra to check if there are more

        // Convert hex user public key to bytes for comparison
        let user_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;
        
        // Build the combined query using UNION to search both k_posts and k_replies via k_mentions
        let mut query = String::from(
            r#"
            (
                SELECT 
                    p.id,
                    p.transaction_id, 
                    p.block_time, 
                    p.sender_pubkey, 
                    p.sender_signature, 
                    p.base64_encoded_message,
                    COALESCE(
                        array_agg(encode(m2.mentioned_pubkey, 'hex')) FILTER (WHERE m2.mentioned_pubkey IS NOT NULL),
                        '{}'::text[]
                    ) as mentioned_pubkeys,
                    NULL as post_id,
                    'post' as content_type
                FROM k_posts p
                INNER JOIN k_mentions m1 ON p.transaction_id = m1.content_id AND m1.content_type = 'post' AND m1.mentioned_pubkey = $1
                LEFT JOIN k_mentions m2 ON p.transaction_id = m2.content_id AND m2.content_type = 'post'
                GROUP BY p.id, p.transaction_id, p.block_time, p.sender_pubkey, p.sender_signature, p.base64_encoded_message
            )
            UNION ALL
            (
                SELECT 
                    r.id,
                    r.transaction_id, 
                    r.block_time, 
                    r.sender_pubkey, 
                    r.sender_signature, 
                    r.base64_encoded_message,
                    COALESCE(
                        array_agg(encode(m2.mentioned_pubkey, 'hex')) FILTER (WHERE m2.mentioned_pubkey IS NOT NULL),
                        '{}'::text[]
                    ) as mentioned_pubkeys,
                    r.post_id,
                    'reply' as content_type
                FROM k_replies r
                INNER JOIN k_mentions m1 ON r.transaction_id = m1.content_id AND m1.content_type = 'reply' AND m1.mentioned_pubkey = $1
                LEFT JOIN k_mentions m2 ON r.transaction_id = m2.content_id AND m2.content_type = 'reply'
                GROUP BY r.id, r.transaction_id, r.block_time, r.sender_pubkey, r.sender_signature, r.base64_encoded_message, r.post_id
            )
            "#
        );

        let mut bind_count = 1;
        let mut where_conditions = Vec::new();
        
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                where_conditions.push(format!(
                    "(block_time < ${} OR (block_time = ${} AND id < ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                where_conditions.push(format!(
                    "(block_time > ${} OR (block_time = ${} AND id > ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        // Add WHERE conditions to the outer query if needed
        if !where_conditions.is_empty() {
            query = format!(
                "SELECT * FROM ({}) AS combined WHERE {}",
                query,
                where_conditions.join(" AND ")
            );
        }

        // Add ORDER BY and LIMIT to the outer query
        if options.sort_descending {
            query.push_str(" ORDER BY block_time DESC, id DESC");
        } else {
            query.push_str(" ORDER BY block_time ASC, id ASC");
        }

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query).bind(&user_pubkey_bytes);

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder
                    .bind(before_timestamp as i64)
                    .bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder
                    .bind(after_timestamp as i64)
                    .bind(after_id);
            }
        }

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(format!("Failed to fetch posts mentioning user: {}", e)))?;

        let mut posts = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            posts.push(KPostRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
            });
        }

        let has_more = posts.len() > limit as usize;
        if has_more {
            posts.pop();
        }

        let pagination = self.create_compound_pagination_metadata(&posts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: posts,
            pagination,
        })
    }

    // Reply operations
    async fn get_replies_by_post_id(
        &self,
        post_id: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KReplyRecord>> {
        let post_id_bytes = Self::decode_hex_to_bytes(post_id)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1;

        let mut query = String::from(
            r#"
            SELECT r.id, r.transaction_id, r.block_time, r.sender_pubkey, r.sender_signature, 
                   r.post_id, r.base64_encoded_message,
                   COALESCE(
                       array_agg(encode(m.mentioned_pubkey, 'hex')) FILTER (WHERE m.mentioned_pubkey IS NOT NULL),
                       '{}'::text[]
                   ) as mentioned_pubkeys
            FROM k_replies r
            LEFT JOIN k_mentions m ON r.transaction_id = m.content_id AND m.content_type = 'reply'
            WHERE r.post_id = $1
            "#
        );

        let mut bind_count = 1;
        
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (r.block_time < ${} OR (r.block_time = ${} AND r.id < ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (r.block_time > ${} OR (r.block_time = ${} AND r.id > ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        query.push_str(" GROUP BY r.id, r.transaction_id, r.block_time, r.sender_pubkey, r.sender_signature, r.post_id, r.base64_encoded_message");

        if options.sort_descending {
            query.push_str(" ORDER BY r.block_time DESC, r.id DESC");
        } else {
            query.push_str(" ORDER BY r.block_time ASC, r.id ASC");
        }

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query).bind(&post_id_bytes);

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder
                    .bind(before_timestamp as i64)
                    .bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder
                    .bind(after_timestamp as i64)
                    .bind(after_id);
            }
        }

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(format!("Failed to fetch replies by post id: {}", e)))?;

        let mut replies = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let post_id: Vec<u8> = row.get("post_id");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            replies.push(KReplyRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                post_id: Self::encode_bytes_to_hex(&post_id),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
            });
        }

        let has_more = replies.len() > limit as usize;
        if has_more {
            replies.pop();
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
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KReplyRecord>> {
        let sender_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1;

        let mut query = String::from(
            r#"
            SELECT r.id, r.transaction_id, r.block_time, r.sender_pubkey, r.sender_signature, 
                   r.post_id, r.base64_encoded_message,
                   COALESCE(
                       array_agg(encode(m.mentioned_pubkey, 'hex')) FILTER (WHERE m.mentioned_pubkey IS NOT NULL),
                       '{}'::text[]
                   ) as mentioned_pubkeys
            FROM k_replies r
            LEFT JOIN k_mentions m ON r.transaction_id = m.content_id AND m.content_type = 'reply'
            WHERE r.sender_pubkey = $1
            "#
        );

        let mut bind_count = 1;
        
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (r.block_time < ${} OR (r.block_time = ${} AND r.id < ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (r.block_time > ${} OR (r.block_time = ${} AND r.id > ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        query.push_str(" GROUP BY r.id, r.transaction_id, r.block_time, r.sender_pubkey, r.sender_signature, r.post_id, r.base64_encoded_message");

        if options.sort_descending {
            query.push_str(" ORDER BY r.block_time DESC, r.id DESC");
        } else {
            query.push_str(" ORDER BY r.block_time ASC, r.id ASC");
        }

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query).bind(&sender_pubkey_bytes);

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder
                    .bind(before_timestamp as i64)
                    .bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder
                    .bind(after_timestamp as i64)
                    .bind(after_id);
            }
        }

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(format!("Failed to fetch replies by user: {}", e)))?;

        let mut replies = Vec::new();
        for row in &rows {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let post_id: Vec<u8> = row.get("post_id");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            replies.push(KReplyRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                post_id: Self::encode_bytes_to_hex(&post_id),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
            });
        }

        let has_more = replies.len() > limit as usize;
        if has_more {
            replies.pop();
        }

        let pagination = self.create_compound_pagination_metadata(&replies, limit as u32, has_more);

        Ok(PaginatedResult {
            items: replies,
            pagination,
        })
    }

    async fn get_reply_by_id(&self, reply_id: &str) -> DatabaseResult<Option<KReplyRecord>> {
        let transaction_id_bytes = Self::decode_hex_to_bytes(reply_id)?;

        let row = sqlx::query(
            r#"
            SELECT r.id, r.transaction_id, r.block_time, r.sender_pubkey, r.sender_signature, 
                   r.post_id, r.base64_encoded_message,
                   COALESCE(
                       array_agg(encode(m.mentioned_pubkey, 'hex')) FILTER (WHERE m.mentioned_pubkey IS NOT NULL),
                       '{}'::text[]
                   ) as mentioned_pubkeys
            FROM k_replies r
            LEFT JOIN k_mentions m ON r.transaction_id = m.content_id AND m.content_type = 'reply'
            WHERE r.transaction_id = $1
            GROUP BY r.id, r.transaction_id, r.block_time, r.sender_pubkey, r.sender_signature, r.post_id, r.base64_encoded_message
            "#
        )
        .bind(&transaction_id_bytes)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("Failed to fetch reply by id: {}", e)))?;

        if let Some(row) = row {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let post_id: Vec<u8> = row.get("post_id");
            let mentioned_pubkeys_array: Vec<String> = row.get("mentioned_pubkeys");

            Ok(Some(KReplyRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                post_id: Self::encode_bytes_to_hex(&post_id),
                base64_encoded_message: row.get("base64_encoded_message"),
                mentioned_pubkeys: mentioned_pubkeys_array,
            }))
        } else {
            Ok(None)
        }
    }

    async fn count_replies_for_post(&self, post_id: &str) -> DatabaseResult<u64> {
        let post_id_bytes = Self::decode_hex_to_bytes(post_id)?;

        let row = sqlx::query("SELECT COUNT(*) FROM k_replies WHERE post_id = $1")
            .bind(&post_id_bytes)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(format!("Failed to count replies: {}", e)))?;

        let count: i64 = row.get(0);
        Ok(count as u64)
    }

    // Broadcast operations
    async fn get_all_broadcasts(
        &self,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KBroadcastRecord>> {
        let limit = options.limit.unwrap_or(20) as i64;
        let offset_limit = limit + 1;

        let mut query = String::from(
            r#"
            SELECT id, transaction_id, block_time, sender_pubkey, sender_signature, 
                   base64_encoded_nickname, base64_encoded_profile_image, base64_encoded_message
            FROM k_broadcasts 
            WHERE 1=1
            "#
        );

        let mut bind_count = 0;
        
        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (block_time < ${} OR (block_time = ${} AND id < ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                bind_count += 2;
                query.push_str(&format!(
                    " AND (block_time > ${} OR (block_time = ${} AND id > ${}))",
                    bind_count - 1, bind_count - 1, bind_count
                ));
            }
        }

        if options.sort_descending {
            query.push_str(" ORDER BY block_time DESC, id DESC");
        } else {
            query.push_str(" ORDER BY block_time ASC, id ASC");
        }

        bind_count += 1;
        query.push_str(&format!(" LIMIT ${}", bind_count));

        let mut query_builder = sqlx::query(&query);

        if let Some(before_cursor) = &options.before {
            if let Ok((before_timestamp, before_id)) = Self::parse_compound_cursor(before_cursor) {
                query_builder = query_builder
                    .bind(before_timestamp as i64)
                    .bind(before_id);
            }
        }

        if let Some(after_cursor) = &options.after {
            if let Ok((after_timestamp, after_id)) = Self::parse_compound_cursor(after_cursor) {
                query_builder = query_builder
                    .bind(after_timestamp as i64)
                    .bind(after_id);
            }
        }

        query_builder = query_builder.bind(offset_limit);

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DatabaseError::QueryError(format!("Failed to fetch all broadcasts: {}", e)))?;

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

        let pagination = self.create_pagination_metadata(&broadcasts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: broadcasts,
            pagination,
        })
    }

    async fn get_latest_broadcast_by_user(
        &self,
        user_public_key: &str,
    ) -> DatabaseResult<Option<KBroadcastRecord>> {
        let sender_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;

        let row = sqlx::query(
            r#"
            SELECT id, transaction_id, block_time, sender_pubkey, sender_signature, 
                   base64_encoded_nickname, base64_encoded_profile_image, base64_encoded_message
            FROM k_broadcasts 
            WHERE sender_pubkey = $1 
            ORDER BY block_time DESC 
            LIMIT 1
            "#
        )
        .bind(&sender_pubkey_bytes)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("Failed to fetch latest broadcast by user: {}", e)))?;

        if let Some(row) = row {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");

            Ok(Some(KBroadcastRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                base64_encoded_nickname: row.get("base64_encoded_nickname"),
                base64_encoded_profile_image: row.get("base64_encoded_profile_image"),
                base64_encoded_message: row.get("base64_encoded_message"),
            }))
        } else {
            Ok(None)
        }
    }

    // Vote operations
    async fn get_vote_counts(&self, post_id: &str) -> DatabaseResult<(u64, u64)> {
        let post_id_bytes = Self::decode_hex_to_bytes(post_id)?;

        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) FILTER (WHERE vote = 'upvote') as upvotes,
                COUNT(*) FILTER (WHERE vote = 'downvote') as downvotes
            FROM k_votes 
            WHERE post_id = $1
            "#
        )
        .bind(&post_id_bytes)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("Failed to get vote counts: {}", e)))?;

        let upvotes: i64 = row.get("upvotes");
        let downvotes: i64 = row.get("downvotes");

        Ok((upvotes as u64, downvotes as u64))
    }

    async fn get_user_vote_for_post(
        &self,
        post_id: &str,
        user_public_key: &str,
    ) -> DatabaseResult<Option<KVoteRecord>> {
        let post_id_bytes = Self::decode_hex_to_bytes(post_id)?;
        let sender_pubkey_bytes = Self::decode_hex_to_bytes(user_public_key)?;

        let row = sqlx::query(
            r#"
            SELECT id, transaction_id, block_time, sender_pubkey, sender_signature, 
                   post_id, vote
            FROM k_votes 
            WHERE post_id = $1 AND sender_pubkey = $2 
            ORDER BY block_time DESC 
            LIMIT 1
            "#
        )
        .bind(&post_id_bytes)
        .bind(&sender_pubkey_bytes)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryError(format!("Failed to get user vote for post: {}", e)))?;

        if let Some(row) = row {
            let transaction_id: Vec<u8> = row.get("transaction_id");
            let sender_pubkey: Vec<u8> = row.get("sender_pubkey");
            let sender_signature: Vec<u8> = row.get("sender_signature");
            let post_id: Vec<u8> = row.get("post_id");

            Ok(Some(KVoteRecord {
                id: row.get::<i64, _>("id"),
                transaction_id: Self::encode_bytes_to_hex(&transaction_id),
                block_time: row.get::<i64, _>("block_time") as u64,
                sender_pubkey: Self::encode_bytes_to_hex(&sender_pubkey),
                sender_signature: Self::encode_bytes_to_hex(&sender_signature),
                post_id: Self::encode_bytes_to_hex(&post_id),
                vote: row.get("vote"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn has_user_upvoted(&self, post_id: &str, user_public_key: &str) -> DatabaseResult<bool> {
        match self.get_user_vote_for_post(post_id, user_public_key).await? {
            Some(vote) => Ok(vote.vote == "upvote"),
            None => Ok(false),
        }
    }

    async fn has_user_downvoted(&self, post_id: &str, user_public_key: &str) -> DatabaseResult<bool> {
        match self.get_user_vote_for_post(post_id, user_public_key).await? {
            Some(vote) => Ok(vote.vote == "downvote"),
            None => Ok(false),
        }
    }

    async fn get_vote_data(
        &self,
        post_id: &str,
        requester_pubkey: &str,
    ) -> DatabaseResult<(u64, u64, bool, bool)> {
        let (upvotes, downvotes) = self.get_vote_counts(post_id).await?;
        let user_upvoted = self.has_user_upvoted(post_id, requester_pubkey).await?;
        let user_downvoted = self.has_user_downvoted(post_id, requester_pubkey).await?;

        Ok((upvotes, downvotes, user_upvoted, user_downvoted))
    }
}