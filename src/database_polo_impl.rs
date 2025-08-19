use async_trait::async_trait;
use polodb_core::{bson::doc, Collection, CollectionT, Database};
use workflow_log::prelude::*;

use crate::database_trait::{
    DatabaseError, DatabaseInterface, DatabaseResult, PaginatedResult, QueryOptions,
};
use crate::models::{KBroadcastRecord, KPostRecord, KReplyRecord, KVoteRecord, PaginationMetadata};

pub struct PoloDbManager {
    pub db: Database,
}

impl PoloDbManager {
    pub fn new(db_path: &str) -> Result<Self, polodb_core::Error> {
        let db = Database::open_path(db_path)?;
        Ok(Self { db })
    }

    pub fn get_k_posts_collection(&self) -> Collection<KPostRecord> {
        self.db.collection("k-posts")
    }

    pub fn get_k_replies_collection(&self) -> Collection<KReplyRecord> {
        self.db.collection("k-replies")
    }

    pub fn get_k_broadcasts_collection(&self) -> Collection<KBroadcastRecord> {
        self.db.collection("k-broadcasts")
    }

    pub fn get_k_votes_collection(&self) -> Collection<KVoteRecord> {
        self.db.collection("k-votes")
    }

    pub fn get_database(&self) -> &Database {
        &self.db
    }

    fn build_time_query(&self, options: &QueryOptions) -> polodb_core::bson::Document {
        let mut query = doc! {};

        if let Some(before_timestamp) = options.before {
            query.insert("block_time", doc! { "$lt": before_timestamp as i64 });
        }

        if let Some(after_timestamp) = options.after {
            query.insert("block_time", doc! { "$gt": after_timestamp as i64 });
        }

        query
    }

    fn build_sort_order(&self, options: &QueryOptions) -> polodb_core::bson::Document {
        if options.sort_descending {
            doc! { "block_time": -1 }
        } else {
            doc! { "block_time": 1 }
        }
    }

    fn create_pagination_metadata<T>(
        &self,
        items: &[T],
        limit: u32,
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
}

trait HasTimestamp {
    fn get_timestamp(&self) -> u64;
}

impl HasTimestamp for KPostRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
    }
}

impl HasTimestamp for KReplyRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
    }
}

impl HasTimestamp for KBroadcastRecord {
    fn get_timestamp(&self) -> u64 {
        self.block_time
    }
}

#[async_trait]
impl DatabaseInterface for PoloDbManager {
    async fn get_posts_by_user(
        &self,
        user_public_key: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>> {
        let k_posts_collection = self.get_k_posts_collection();

        let mut query = doc! { "sender_pubkey": user_public_key };
        let time_filter = self.build_time_query(&options);
        for (key, value) in time_filter {
            query.insert(key, value);
        }

        let sort_order = self.build_sort_order(&options);

        let query_limit = options.limit.unwrap_or(50) + 1; // +1 to detect if there are more items

        let query_result = k_posts_collection
            .find(query)
            .sort(sort_order)
            .limit(query_limit)
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let mut posts = Vec::new();
        for item in query_result {
            match item {
                Ok(post) => posts.push(post),
                Err(err) => {
                    log_error!("Error reading K post record: {}", err);
                    return Err(DatabaseError::SerializationError(err.to_string()));
                }
            }
        }

        let limit = options.limit.unwrap_or(50) as usize;
        let has_more = posts.len() > limit;

        if has_more {
            posts.truncate(limit);
        }

        let pagination = self.create_pagination_metadata(&posts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: posts,
            pagination,
        })
    }

    async fn get_all_posts(
        &self,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KPostRecord>> {
        let k_posts_collection = self.get_k_posts_collection();

        let query = self.build_time_query(&options);
        let sort_order = self.build_sort_order(&options);

        let query_limit = options.limit.unwrap_or(50) + 1;

        let query_result = k_posts_collection
            .find(query)
            .sort(sort_order)
            .limit(query_limit)
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let mut posts = Vec::new();
        for item in query_result {
            match item {
                Ok(post) => posts.push(post),
                Err(err) => {
                    log_error!("Error reading K post record: {}", err);
                    return Err(DatabaseError::SerializationError(err.to_string()));
                }
            }
        }

        let limit = options.limit.unwrap_or(50) as usize;
        let has_more = posts.len() > limit;

        if has_more {
            posts.truncate(limit);
        }

        let pagination = self.create_pagination_metadata(&posts, limit as u32, has_more);

        Ok(PaginatedResult {
            items: posts,
            pagination,
        })
    }

    async fn get_post_by_id(&self, post_id: &str) -> DatabaseResult<Option<KPostRecord>> {
        let k_posts_collection = self.get_k_posts_collection();
        let query = doc! { "transaction_id": post_id };

        let mut cursor = k_posts_collection
            .find(query)
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        match cursor.next() {
            Some(Ok(post)) => Ok(Some(post)),
            Some(Err(err)) => Err(DatabaseError::SerializationError(err.to_string())),
            None => Ok(None),
        }
    }

    async fn get_posts_mentioning_user(
        &self,
        user_public_key: &str,
        _options: QueryOptions,
    ) -> DatabaseResult<Vec<KPostRecord>> {
        let k_posts_collection = self.get_k_posts_collection();
        let query = doc! {};

        let query_result = k_posts_collection
            .find(query)
            .sort(doc! { "block_time": -1 })
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let mut posts = Vec::new();
        let target_user = user_public_key.to_string();

        for item in query_result {
            match item {
                Ok(post) => {
                    if post.mentioned_pubkeys.contains(&target_user) {
                        posts.push(post);
                    }
                }
                Err(err) => {
                    log_error!("Error reading K post record during manual search: {}", err);
                }
            }
        }

        Ok(posts)
    }

    async fn get_replies_by_post_id(
        &self,
        post_id: &str,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KReplyRecord>> {
        let k_replies_collection = self.get_k_replies_collection();

        let mut query = doc! { "post_id": post_id };
        let time_filter = self.build_time_query(&options);
        for (key, value) in time_filter {
            query.insert(key, value);
        }

        let sort_order = self.build_sort_order(&options);
        let query_limit = options.limit.unwrap_or(50) + 1;

        let query_result = k_replies_collection
            .find(query)
            .sort(sort_order)
            .limit(query_limit)
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let mut replies = Vec::new();
        for item in query_result {
            match item {
                Ok(reply) => replies.push(reply),
                Err(err) => {
                    log_error!("Error reading K reply record: {}", err);
                    return Err(DatabaseError::SerializationError(err.to_string()));
                }
            }
        }

        let limit = options.limit.unwrap_or(50) as usize;
        let has_more = replies.len() > limit;

        if has_more {
            replies.truncate(limit);
        }

        let pagination = self.create_pagination_metadata(&replies, limit as u32, has_more);

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
        let k_replies_collection = self.get_k_replies_collection();

        let mut query = doc! { "sender_pubkey": user_public_key };
        let time_filter = self.build_time_query(&options);
        for (key, value) in time_filter {
            query.insert(key, value);
        }

        let sort_order = self.build_sort_order(&options);
        let query_limit = options.limit.unwrap_or(50) + 1;

        let query_result = k_replies_collection
            .find(query)
            .sort(sort_order)
            .limit(query_limit)
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let mut replies = Vec::new();
        for item in query_result {
            match item {
                Ok(reply) => replies.push(reply),
                Err(err) => {
                    log_error!("Error reading K reply record: {}", err);
                    return Err(DatabaseError::SerializationError(err.to_string()));
                }
            }
        }

        let limit = options.limit.unwrap_or(50) as usize;
        let has_more = replies.len() > limit;

        if has_more {
            replies.truncate(limit);
        }

        let pagination = self.create_pagination_metadata(&replies, limit as u32, has_more);

        Ok(PaginatedResult {
            items: replies,
            pagination,
        })
    }

    async fn get_reply_by_id(&self, reply_id: &str) -> DatabaseResult<Option<KReplyRecord>> {
        let k_replies_collection = self.get_k_replies_collection();
        let query = doc! { "transaction_id": reply_id };

        let mut cursor = k_replies_collection
            .find(query)
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        match cursor.next() {
            Some(Ok(reply)) => Ok(Some(reply)),
            Some(Err(err)) => Err(DatabaseError::SerializationError(err.to_string())),
            None => Ok(None),
        }
    }

    async fn get_replies_mentioning_user(
        &self,
        user_public_key: &str,
        _options: QueryOptions,
    ) -> DatabaseResult<Vec<KReplyRecord>> {
        let k_replies_collection = self.get_k_replies_collection();
        let query = doc! {};

        let query_result = k_replies_collection
            .find(query)
            .sort(doc! { "block_time": -1 })
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let mut replies = Vec::new();
        let target_user = user_public_key.to_string();

        for item in query_result {
            match item {
                Ok(reply) => {
                    if reply.mentioned_pubkeys.contains(&target_user) {
                        replies.push(reply);
                    }
                }
                Err(err) => {
                    log_error!("Error reading K reply record during manual search: {}", err);
                }
            }
        }

        Ok(replies)
    }

    async fn count_replies_for_post(&self, post_id: &str) -> DatabaseResult<u64> {
        let k_replies_collection = self.get_k_replies_collection();

        let cursor = k_replies_collection
            .find(doc! { "post_id": post_id })
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        Ok(cursor.count() as u64)
    }

    async fn get_all_broadcasts(
        &self,
        options: QueryOptions,
    ) -> DatabaseResult<PaginatedResult<KBroadcastRecord>> {
        let k_broadcasts_collection = self.get_k_broadcasts_collection();

        let query = self.build_time_query(&options);
        let sort_order = self.build_sort_order(&options);
        let query_limit = options.limit.unwrap_or(50) + 1;

        let query_result = k_broadcasts_collection
            .find(query)
            .sort(sort_order)
            .limit(query_limit)
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let mut broadcasts = Vec::new();
        for item in query_result {
            match item {
                Ok(broadcast) => broadcasts.push(broadcast),
                Err(err) => {
                    log_warn!("Error reading K broadcast record (skipping): {}", err);
                    continue;
                }
            }
        }

        let limit = options.limit.unwrap_or(50) as usize;
        let has_more = broadcasts.len() > limit;

        if has_more {
            broadcasts.truncate(limit);
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
        let k_broadcasts_collection = self.get_k_broadcasts_collection();
        let query = doc! { "sender_pubkey": user_public_key };

        let mut cursor = k_broadcasts_collection
            .find(query)
            .sort(doc! { "block_time": -1 })
            .limit(1)
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        match cursor.next() {
            Some(Ok(broadcast)) => Ok(Some(broadcast)),
            Some(Err(_)) => Ok(None), // Failed to deserialize, likely old format
            None => Ok(None),
        }
    }

    async fn get_votes_for_post(&self, post_id: &str) -> DatabaseResult<Vec<KVoteRecord>> {
        let k_votes_collection = self.get_k_votes_collection();

        let cursor = k_votes_collection
            .find(doc! { "post_id": post_id })
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        let mut votes = Vec::new();
        for vote_result in cursor {
            match vote_result {
                Ok(vote) => votes.push(vote),
                Err(err) => {
                    log_error!("Error reading vote record for post {}: {}", post_id, err);
                }
            }
        }

        Ok(votes)
    }

    async fn get_vote_counts(&self, post_id: &str) -> DatabaseResult<(u64, u64)> {
        let votes = self.get_votes_for_post(post_id).await?;

        let mut up_votes_count = 0u64;
        let mut down_votes_count = 0u64;

        for vote in votes {
            match vote.vote.as_str() {
                "upvote" => up_votes_count += 1,
                "downvote" => down_votes_count += 1,
                _ => log_error!("Invalid vote value found in database: {}", vote.vote),
            }
        }

        Ok((up_votes_count, down_votes_count))
    }

    async fn get_user_vote_for_post(
        &self,
        post_id: &str,
        user_public_key: &str,
    ) -> DatabaseResult<Option<KVoteRecord>> {
        let k_votes_collection = self.get_k_votes_collection();

        let mut cursor = k_votes_collection
            .find(doc! {
                "post_id": post_id,
                "sender_pubkey": user_public_key
            })
            .run()
            .map_err(|e| DatabaseError::QueryError(e.to_string()))?;

        match cursor.next() {
            Some(Ok(vote)) => Ok(Some(vote)),
            Some(Err(err)) => {
                log_error!("Error reading vote record: {}", err);
                Ok(None)
            }
            None => Ok(None),
        }
    }

    async fn has_user_upvoted(&self, post_id: &str, user_public_key: &str) -> DatabaseResult<bool> {
        match self
            .get_user_vote_for_post(post_id, user_public_key)
            .await?
        {
            Some(vote) => Ok(vote.vote == "upvote"),
            None => Ok(false),
        }
    }

    async fn has_user_downvoted(
        &self,
        post_id: &str,
        user_public_key: &str,
    ) -> DatabaseResult<bool> {
        match self
            .get_user_vote_for_post(post_id, user_public_key)
            .await?
        {
            Some(vote) => Ok(vote.vote == "downvote"),
            None => Ok(false),
        }
    }

    async fn get_vote_data(
        &self,
        post_id: &str,
        requester_pubkey: &str,
    ) -> DatabaseResult<(u64, u64, bool, bool)> {
        if requester_pubkey.is_empty() {
            let (up_votes_count, down_votes_count) = self.get_vote_counts(post_id).await?;
            return Ok((up_votes_count, down_votes_count, false, false));
        }

        let votes = self.get_votes_for_post(post_id).await?;

        let mut up_votes_count = 0u64;
        let mut down_votes_count = 0u64;
        let mut is_upvoted = false;
        let mut is_downvoted = false;

        for vote in votes {
            match vote.vote.as_str() {
                "upvote" => {
                    up_votes_count += 1;
                    if vote.sender_pubkey == requester_pubkey {
                        is_upvoted = true;
                    }
                }
                "downvote" => {
                    down_votes_count += 1;
                    if vote.sender_pubkey == requester_pubkey {
                        is_downvoted = true;
                    }
                }
                _ => {
                    log_error!("Invalid vote value found in database: {}", vote.vote);
                }
            }
        }

        Ok((up_votes_count, down_votes_count, is_upvoted, is_downvoted))
    }
}
