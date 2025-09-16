use crate::models::{
    ContentRecord, KBroadcastRecord, KPostRecord, KReplyRecord, PaginationMetadata,
};
use async_trait::async_trait;
use std::result::Result as StdResult;

pub type DatabaseResult<T> = StdResult<T, DatabaseError>;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum DatabaseError {
    ConnectionError(String),
    QueryError(String),
    SerializationError(String),
    NotFound,
    InvalidInput(String),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            DatabaseError::QueryError(msg) => write!(f, "Query error: {}", msg),
            DatabaseError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            DatabaseError::NotFound => write!(f, "Record not found"),
            DatabaseError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
        }
    }
}

impl std::error::Error for DatabaseError {}

#[derive(Debug, Clone)]
pub struct QueryOptions {
    pub limit: Option<u64>,
    pub before: Option<String>, // Compound cursors like "timestamp_id"
    pub after: Option<String>,  // Compound cursors like "timestamp_id"
    pub sort_descending: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            limit: None,
            before: None,
            after: None,
            sort_descending: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub pagination: PaginationMetadata,
}

#[async_trait]
#[allow(dead_code)]
pub trait DatabaseInterface: Send + Sync {
    // Post operations (optimized versions with metadata)

    // Broadcast operations
    async fn get_all_broadcasts(
        &self,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KBroadcastRecord>>;

    async fn get_broadcast_by_user(
        &self,
        user_public_key: &str,
    ) -> DatabaseResult<Option<KBroadcastRecord>>;

    async fn get_broadcast_by_user_with_block_status(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
    ) -> DatabaseResult<Option<(KBroadcastRecord, bool)>>;

    async fn get_blocked_users_by_requester(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KBroadcastRecord>>;

    // Optimized single-query method for get-posts-watching API
    async fn get_all_posts_with_metadata(
        &self,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>>;

    // Optimized single-query method for get-posts API
    async fn get_posts_by_user_with_metadata(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>>;

    // Optimized single-query method for get-replies API (by post)
    async fn get_replies_by_post_id_with_metadata(
        &self,
        post_id: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KReplyRecord>>;

    // Optimized single-query method for get-replies API (by user)
    async fn get_replies_by_user_with_metadata(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KReplyRecord>>;

    // Optimized single-query method for get-mentions API
    async fn get_contents_mentioning_user_with_metadata(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<ContentRecord>>;

    // Merged optimized single-query method for get-post-details API (posts and replies)
    async fn get_content_by_id_with_metadata(
        &self,
        content_id: &str,
        requester_pubkey: &str,
    ) -> DatabaseResult<Option<ContentRecord>>;
}
