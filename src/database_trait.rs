use crate::models::{KBroadcastRecord, KPostRecord, KReplyRecord, KVoteRecord, PaginationMetadata};
use async_trait::async_trait;
use polodb_core::bson::Document;
use std::result::Result as StdResult;

pub type DatabaseResult<T> = StdResult<T, DatabaseError>;

#[derive(Debug, Clone)]
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
    pub before: Option<u64>,
    pub after: Option<u64>,
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
pub trait DatabaseInterface: Send + Sync {
    // Post operations
    async fn get_posts_by_user(
        &self,
        user_public_key: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>>;

    async fn get_all_posts(
        &self,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>>;

    async fn get_post_by_id(&self, post_id: &str) -> DatabaseResult<Option<KPostRecord>>;

    async fn get_posts_mentioning_user(
        &self,
        user_public_key: &str,
        options: QueryOptions,
    ) -> DatabaseResult<Vec<KPostRecord>>;

    // Reply operations
    async fn get_replies_by_post_id(
        &self,
        post_id: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KReplyRecord>>;

    async fn get_replies_by_user(
        &self,
        user_public_key: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KReplyRecord>>;

    async fn get_reply_by_id(&self, reply_id: &str) -> DatabaseResult<Option<KReplyRecord>>;

    async fn get_replies_mentioning_user(
        &self,
        user_public_key: &str,
        options: QueryOptions,
    ) -> DatabaseResult<Vec<KReplyRecord>>;

    async fn count_replies_for_post(&self, post_id: &str) -> DatabaseResult<u64>;

    // Broadcast operations
    async fn get_all_broadcasts(
        &self,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KBroadcastRecord>>;

    async fn get_latest_broadcast_by_user(
        &self,
        user_public_key: &str,
    ) -> DatabaseResult<Option<KBroadcastRecord>>;

    // Vote operations
    async fn get_votes_for_post(&self, post_id: &str) -> DatabaseResult<Vec<KVoteRecord>>;

    async fn get_vote_counts(&self, post_id: &str) -> DatabaseResult<(u64, u64)>; // (upvotes, downvotes)

    async fn get_user_vote_for_post(
        &self,
        post_id: &str,
        user_public_key: &str,
    ) -> DatabaseResult<Option<KVoteRecord>>;

    async fn has_user_upvoted(&self, post_id: &str, user_public_key: &str) -> DatabaseResult<bool>;

    async fn has_user_downvoted(
        &self,
        post_id: &str,
        user_public_key: &str,
    ) -> DatabaseResult<bool>;

    // Combined vote data (upvotes, downvotes, user_upvoted, user_downvoted)
    async fn get_vote_data(
        &self,
        post_id: &str,
        requester_pubkey: &str,
    ) -> DatabaseResult<(u64, u64, bool, bool)>;
}
