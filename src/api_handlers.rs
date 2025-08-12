use crate::database::DatabaseManager;
use crate::models::{ApiError, PostsResponse, UsersResponse, ServerPost, ServerReply, ServerUserPost, KPostRecord, KReplyRecord, KBroadcastRecord, KVoteRecord, PaginatedPostsResponse, PaginationMetadata, PaginatedRepliesResponse, PaginatedUsersResponse, PostDetailsResponse};
use polodb_core::{bson::doc, CollectionT};
use serde::{Deserialize};
use serde_json;
use std::sync::Arc;
use workflow_log::prelude::*;

pub struct ApiHandlers {
    db_manager: Arc<DatabaseManager>,
}

impl ApiHandlers {
    pub fn new(db_manager: Arc<DatabaseManager>) -> Self {
        Self { db_manager }
    }

    /// GET /get-posts?user={userPublicKey}&requesterPubkey={requesterPubkey}
    /// Fetch all posts for a specific user with voting status
    pub async fn get_posts(&self, user_public_key: &str, requester_pubkey: &str) -> Result<String, String> {

        // Validate user public key format (66 hex characters for compressed public key)
        if user_public_key.len() != 66 {
            return Err(self.create_error_response(
                "Invalid user public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !user_public_key.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid user public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !user_public_key.starts_with("02") && !user_public_key.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid user public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate requester public key format (66 hex characters for compressed public key)
        if requester_pubkey.len() != 66 {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !requester_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !requester_pubkey.starts_with("02") && !requester_pubkey.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid requester public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        // Query database for posts by this user
        let k_posts_collection = self.db_manager.get_k_posts_collection();
        
        let query_result = match k_posts_collection.find(doc! { "sender_pubkey": user_public_key }).run() {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while querying posts: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };
        
        let mut posts = Vec::new();
        let k_replies_collection = self.db_manager.get_k_replies_collection();
        let k_votes_collection = self.db_manager.get_k_votes_collection();
        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();
        
        for item in query_result {
            match item {
                Ok(k_post_record) => {
                    // Calculate replies count for this post
                    let replies_count = match k_replies_collection
                        .find(doc! { "post_id": &k_post_record.transaction_id })
                        .run() {
                        Ok(cursor) => {
                            cursor.count() as u64
                        },
                        Err(err) => {
                            log_error!("Error counting replies for post {}: {}", k_post_record.transaction_id, err);
                            0
                        }
                    };
                    
                    // Calculate vote counts and user's vote status
                    let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = 
                        self.get_vote_data(&k_post_record.transaction_id, requester_pubkey, &k_votes_collection);
                    
                    let mut server_post = ServerPost::from_k_post_record_with_replies_count_and_votes(
                        &k_post_record, 
                        replies_count,
                        up_votes_count,
                        down_votes_count,
                        is_upvoted,
                        is_downvoted
                    );
                    
                    // Enrich with user profile data from broadcasts
                    self.enrich_post_with_user_profile(&mut server_post, &k_broadcasts_collection);
                    
                    posts.push(server_post);
                }
                Err(err) => {
                    log_error!("Error reading K post record: {}", err);
                    continue;
                }
            }
        }

        // Sort posts by timestamp (newest first)
        posts.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let response = PostsResponse { posts };
        
        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize posts response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-posts-watching?requesterPubkey={requesterPubkey}
    /// Fetch all posts (not replies) available in the k-posts database with voting status
    pub async fn get_posts_watching(&self, requester_pubkey: &str) -> Result<String, String> {

        // Validate requester public key format (66 hex characters for compressed public key)
        if requester_pubkey.len() != 66 {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !requester_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !requester_pubkey.starts_with("02") && !requester_pubkey.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid requester public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        // Query database for all posts (no filter)
        let k_posts_collection = self.db_manager.get_k_posts_collection();
        
        let query_result = match k_posts_collection.find(doc! {}).run() {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while querying all posts: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };
        
        let mut posts = Vec::new();
        let k_replies_collection = self.db_manager.get_k_replies_collection();
        let k_votes_collection = self.db_manager.get_k_votes_collection();
        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();
        
        for item in query_result {
            match item {
                Ok(k_post_record) => {
                    // Calculate replies count for this post
                    let replies_count = match k_replies_collection
                        .find(doc! { "post_id": &k_post_record.transaction_id })
                        .run() {
                        Ok(cursor) => {
                            cursor.count() as u64
                        },
                        Err(err) => {
                            log_error!("Error counting replies for post {}: {}", k_post_record.transaction_id, err);
                            0
                        }
                    };
                    
                    // Calculate vote counts and user's vote status
                    let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = 
                        self.get_vote_data(&k_post_record.transaction_id, requester_pubkey, &k_votes_collection);
                    
                    let mut server_post = ServerPost::from_k_post_record_with_replies_count_and_votes(
                        &k_post_record, 
                        replies_count,
                        up_votes_count,
                        down_votes_count,
                        is_upvoted,
                        is_downvoted
                    );
                    
                    // Enrich with user profile data from broadcasts
                    self.enrich_post_with_user_profile(&mut server_post, &k_broadcasts_collection);
                    
                    posts.push(server_post);
                }
                Err(err) => {
                    log_error!("Error reading K post record: {}", err);
                    continue;
                }
            }
        }

        // Sort posts by timestamp (newest first)
        posts.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let response = PostsResponse { posts };
        
        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize posts response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-posts with pagination
    /// Fetch paginated posts for a specific user with cursor-based pagination and voting status
    pub async fn get_posts_paginated(&self, user_public_key: &str, requester_pubkey: &str, limit: u32, before: Option<u64>, after: Option<u64>) -> Result<String, String> {

        // Validate user public key format (66 hex characters for compressed public key)
        if user_public_key.len() != 66 {
            return Err(self.create_error_response(
                "Invalid user public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !user_public_key.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid user public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !user_public_key.starts_with("02") && !user_public_key.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid user public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate requester public key format (66 hex characters for compressed public key)
        if requester_pubkey.len() != 66 {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !requester_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !requester_pubkey.starts_with("02") && !requester_pubkey.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid requester public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        let k_posts_collection = self.db_manager.get_k_posts_collection();
        let k_replies_collection = self.db_manager.get_k_replies_collection();
        let k_votes_collection = self.db_manager.get_k_votes_collection();
        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();
        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();

        // Build query based on cursor parameters
        let mut query = doc! { "sender_pubkey": user_public_key };
        
        if let Some(before_timestamp) = before {
            query.insert("block_time", doc! { "$lt": before_timestamp as i64 });
        }
        
        if let Some(after_timestamp) = after {
            query.insert("block_time", doc! { "$gt": after_timestamp as i64 });
        }

        // Always sort descending (newest first) for consistency
        let sort_order = doc! { "block_time": -1 };

        // Query database with proper pagination: fetch one extra to determine hasMore
        let query_limit = (limit + 1) as u64;
        
        let query_result = match k_posts_collection
            .find(query)
            .sort(sort_order)
            .limit(query_limit)
            .run() {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while querying paginated posts for user {}: {}", user_public_key, err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };
        
        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();
        let mut all_posts = Vec::new();
        
        for item in query_result {
            match item {
                Ok(k_post_record) => {
                    // Calculate replies count for this post
                    let replies_count = match k_replies_collection
                        .find(doc! { "post_id": &k_post_record.transaction_id })
                        .run() {
                        Ok(cursor) => {
                            cursor.count() as u64
                        },
                        Err(err) => {
                            log_error!("Error counting replies for post {}: {}", k_post_record.transaction_id, err);
                            0
                        }
                    };
                    
                    // Calculate vote counts and user's vote status
                    let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = 
                        self.get_vote_data(&k_post_record.transaction_id, requester_pubkey, &k_votes_collection);
                    
                    let mut server_post = ServerPost::from_k_post_record_with_replies_count_and_votes(
                        &k_post_record, 
                        replies_count,
                        up_votes_count,
                        down_votes_count,
                        is_upvoted,
                        is_downvoted
                    );
                    
                    // Enrich with user profile data from broadcasts
                    self.enrich_post_with_user_profile(&mut server_post, &k_broadcasts_collection);
                    
                    all_posts.push(server_post);
                }
                Err(err) => {
                    log_error!("Error reading K post record: {}", err);
                    continue;
                }
            }
        }

        // Determine if there are more posts available
        let has_more = all_posts.len() > limit as usize;
        
        // Take only the requested number of posts (remove the extra one used for hasMore detection)
        if has_more {
            all_posts.truncate(limit as usize);
        }

        // Calculate pagination cursors
        let next_cursor = if has_more && !all_posts.is_empty() {
            // For next page (older posts), use the timestamp of the last post
            Some(all_posts.last().unwrap().timestamp.to_string())
        } else {
            None
        };
        
        let prev_cursor = if !all_posts.is_empty() {
            // For checking newer posts, use the timestamp of the first post
            Some(all_posts.first().unwrap().timestamp.to_string())
        } else {
            None
        };

        let pagination = PaginationMetadata {
            has_more,
            next_cursor,
            prev_cursor,
        };

        let response = PaginatedPostsResponse { posts: all_posts, pagination };
        
        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize paginated posts response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-posts-watching with pagination
    /// Fetch paginated posts for watching with cursor-based pagination and voting status
    pub async fn get_posts_watching_paginated(&self, requester_pubkey: &str, limit: u32, before: Option<u64>, after: Option<u64>) -> Result<String, String> {

        // Validate requester public key format (66 hex characters for compressed public key)
        if requester_pubkey.len() != 66 {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !requester_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !requester_pubkey.starts_with("02") && !requester_pubkey.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid requester public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        let k_posts_collection = self.db_manager.get_k_posts_collection();
        let k_replies_collection = self.db_manager.get_k_replies_collection();
        let k_votes_collection = self.db_manager.get_k_votes_collection();

        // Build query based on cursor parameters
        let mut query = doc! {};
        
        if let Some(before_timestamp) = before {
            query.insert("block_time", doc! { "$lt": before_timestamp as i64 });
        }
        
        if let Some(after_timestamp) = after {
            query.insert("block_time", doc! { "$gt": after_timestamp as i64 });
        }

        // Always sort descending (newest first) for consistency
        let sort_order = doc! { "block_time": -1 };

        // Query database with proper pagination: fetch one extra to determine hasMore
        let query_limit = (limit + 1) as u64;
        
        let query_result = match k_posts_collection
            .find(query)
            .sort(sort_order)
            .limit(query_limit)
            .run() {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while querying paginated posts: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };
        
        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();
        let mut all_posts = Vec::new();
        
        for item in query_result {
            match item {
                Ok(k_post_record) => {
                    // Calculate replies count for this post
                    let replies_count = match k_replies_collection
                        .find(doc! { "post_id": &k_post_record.transaction_id })
                        .run() {
                        Ok(cursor) => {
                            cursor.count() as u64
                        },
                        Err(err) => {
                            log_error!("Error counting replies for post {}: {}", k_post_record.transaction_id, err);
                            0
                        }
                    };
                    
                    // Calculate vote counts and user's vote status
                    let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = 
                        self.get_vote_data(&k_post_record.transaction_id, requester_pubkey, &k_votes_collection);
                    
                    let mut server_post = ServerPost::from_k_post_record_with_replies_count_and_votes(
                        &k_post_record, 
                        replies_count,
                        up_votes_count,
                        down_votes_count,
                        is_upvoted,
                        is_downvoted
                    );
                    
                    // Enrich with user profile data from broadcasts
                    self.enrich_post_with_user_profile(&mut server_post, &k_broadcasts_collection);
                    
                    all_posts.push(server_post);
                }
                Err(err) => {
                    log_error!("Error reading K post record: {}", err);
                    continue;
                }
            }
        }

        // Determine if there are more posts available
        let has_more = all_posts.len() > limit as usize;
        
        // Take only the requested number of posts (remove the extra one used for hasMore detection)
        if has_more {
            all_posts.truncate(limit as usize);
        }

        // Calculate pagination cursors
        let next_cursor = if has_more && !all_posts.is_empty() {
            // For next page (older posts), use the timestamp of the last post
            Some(all_posts.last().unwrap().timestamp.to_string())
        } else {
            None
        };
        
        let prev_cursor = if !all_posts.is_empty() {
            // For checking newer posts, use the timestamp of the first post
            Some(all_posts.first().unwrap().timestamp.to_string())
        } else {
            None
        };

        let pagination = PaginationMetadata {
            has_more,
            next_cursor,
            prev_cursor,
        };

        let response = PaginatedPostsResponse { posts: all_posts, pagination };
        
        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize paginated posts response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    

    /// GET /get-replies with pagination (Post Replies Mode)
    /// Fetch paginated replies for a specific post with cursor-based pagination and voting status
    pub async fn get_replies_paginated(&self, post_id: &str, requester_pubkey: &str, limit: u32, before: Option<u64>, after: Option<u64>) -> Result<String, String> {

        // Validate post ID format (64 hex characters for transaction hash)
        if post_id.len() != 64 {
            return Err(self.create_error_response(
                "Invalid post ID format. Must be 64 hex characters.",
                "INVALID_POST_ID",
            ));
        }

        if !post_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid post ID format. Must contain only hex characters.",
                "INVALID_POST_ID",
            ));
        }

        // Validate requester public key format (66 hex characters for compressed public key)
        if requester_pubkey.len() != 66 {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !requester_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !requester_pubkey.starts_with("02") && !requester_pubkey.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid requester public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        let k_replies_collection = self.db_manager.get_k_replies_collection();
        let k_votes_collection = self.db_manager.get_k_votes_collection();

        // Build query based on cursor parameters
        let mut query = doc! { "post_id": post_id };
        
        if let Some(before_timestamp) = before {
            query.insert("block_time", doc! { "$lt": before_timestamp as i64 });
        }
        
        if let Some(after_timestamp) = after {
            query.insert("block_time", doc! { "$gt": after_timestamp as i64 });
        }

        // Always sort descending (newest first) for consistency
        let sort_order = doc! { "block_time": -1 };

        // Query database with proper pagination: fetch one extra to determine hasMore
        let query_limit = (limit + 1) as u64;
        
        let query_result = match k_replies_collection
            .find(query)
            .sort(sort_order)
            .limit(query_limit)
            .run() {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while querying paginated replies for post {}: {}", post_id, err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };
        
        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();
        let mut all_replies = Vec::new();
        
        for item in query_result {
            match item {
                Ok(k_reply_record) => {
                    // Calculate replies count for this reply (nested replies)
                    let replies_count = match k_replies_collection
                        .find(doc! { "post_id": &k_reply_record.transaction_id })
                        .run() {
                        Ok(cursor) => {
                            cursor.count() as u64
                        },
                        Err(err) => {
                            log_error!("Error counting replies for reply {}: {}", k_reply_record.transaction_id, err);
                            0
                        }
                    };
                    
                    // Calculate vote counts and user's vote status
                    let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = 
                        self.get_vote_data(&k_reply_record.transaction_id, requester_pubkey, &k_votes_collection);
                    
                    let mut server_reply = ServerReply::from_k_reply_record_with_replies_count_and_votes(
                        &k_reply_record, 
                        replies_count,
                        up_votes_count,
                        down_votes_count,
                        is_upvoted,
                        is_downvoted
                    );
                    
                    // Enrich with user profile data from broadcasts
                    self.enrich_post_with_user_profile(&mut server_reply, &k_broadcasts_collection);
                    
                    all_replies.push(server_reply);
                }
                Err(err) => {
                    log_error!("Error reading K reply record: {}", err);
                    continue;
                }
            }
        }

        // Determine if there are more replies available
        let has_more = all_replies.len() > limit as usize;
        
        // Take only the requested number of replies (remove the extra one used for hasMore detection)
        if has_more {
            all_replies.truncate(limit as usize);
        }

        // Calculate pagination cursors
        let next_cursor = if has_more && !all_replies.is_empty() {
            // For next page (older replies), use the timestamp of the last reply
            Some(all_replies.last().unwrap().timestamp.to_string())
        } else {
            None
        };
        
        let prev_cursor = if !all_replies.is_empty() {
            // For checking newer replies, use the timestamp of the first reply
            Some(all_replies.first().unwrap().timestamp.to_string())
        } else {
            None
        };

        let pagination = PaginationMetadata {
            has_more,
            next_cursor,
            prev_cursor,
        };

        let response = PaginatedRepliesResponse { replies: all_replies, pagination };
        
        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize paginated replies response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-users
    /// Fetch user introduction posts from all users (broadcasts)
    pub async fn get_users(&self) -> Result<String, String> {

        // Query database for all broadcasts (user introductions)
        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();
        
        let query_result = match k_broadcasts_collection.find(doc! {}).run() {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while querying user broadcasts: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };
        
        let mut posts = Vec::new();
        
        for item in query_result {
            match item {
                Ok(k_broadcast_record) => {
                    let server_user_post = ServerUserPost::from_k_broadcast_record(&k_broadcast_record);
                    posts.push(server_user_post);
                }
                Err(err) => {
                    // Failed to deserialize record - likely old format without required fields
                    log_warn!("Error reading K broadcast record (skipping): {}", err);
                    continue;
                }
            }
        }

        // Sort posts by timestamp (newest first)
        posts.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let response = UsersResponse { posts };
        
        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize users response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-users with pagination
    /// Fetch paginated user introduction posts with cursor-based pagination
    pub async fn get_users_paginated(&self, limit: u32, before: Option<u64>, after: Option<u64>) -> Result<String, String> {

        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();

        // Build query based on cursor parameters
        let mut query = doc! {};
        
        if let Some(before_timestamp) = before {
            query.insert("block_time", doc! { "$lt": before_timestamp as i64 });
        }
        
        if let Some(after_timestamp) = after {
            query.insert("block_time", doc! { "$gt": after_timestamp as i64 });
        }

        // Always sort descending (newest first) for consistency
        let sort_order = doc! { "block_time": -1 };

        // Query database with proper pagination: fetch one extra to determine hasMore
        let query_limit = (limit + 1) as u64;
        
        let query_result = match k_broadcasts_collection
            .find(query)
            .sort(sort_order)
            .limit(query_limit)
            .run() {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while querying paginated user broadcasts: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };
        
        let mut all_posts = Vec::new();
        
        for item in query_result {
            match item {
                Ok(k_broadcast_record) => {
                    let mut server_user_post = ServerUserPost::from_k_broadcast_record(&k_broadcast_record);
                    
                    // Enrich with user profile data from broadcasts
                    self.enrich_user_post_with_user_profile(&mut server_user_post, &k_broadcasts_collection);
                    
                    all_posts.push(server_user_post);
                }
                Err(err) => {
                    // Failed to deserialize record - likely old format without required fields
                    log_warn!("Error reading K broadcast record (skipping): {}", err);
                    continue;
                }
            }
        }

        // Determine if there are more posts available
        let has_more = all_posts.len() > limit as usize;
        
        // Take only the requested number of posts (remove the extra one used for hasMore detection)
        if has_more {
            all_posts.truncate(limit as usize);
        }

        // Calculate pagination cursors
        let next_cursor = if has_more && !all_posts.is_empty() {
            // For next page (older posts), use the timestamp of the last post
            Some(all_posts.last().unwrap().timestamp.to_string())
        } else {
            None
        };
        
        let prev_cursor = if !all_posts.is_empty() {
            // For checking newer posts, use the timestamp of the first post
            Some(all_posts.first().unwrap().timestamp.to_string())
        } else {
            None
        };

        let pagination = PaginationMetadata {
            has_more,
            next_cursor,
            prev_cursor,
        };

        let response = PaginatedUsersResponse { posts: all_posts, pagination };
        
        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize paginated users response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-replies with pagination (User Replies Mode)
    /// Fetch paginated replies made by a specific user with cursor-based pagination and voting status
    pub async fn get_user_replies_paginated(&self, user_public_key: &str, requester_pubkey: &str, limit: u32, before: Option<u64>, after: Option<u64>) -> Result<String, String> {

        // Validate user public key format (66 hex characters for compressed public key)
        if user_public_key.len() != 66 {
            return Err(self.create_error_response(
                "Invalid user public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !user_public_key.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid user public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !user_public_key.starts_with("02") && !user_public_key.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid user public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate requester public key format (66 hex characters for compressed public key)
        if requester_pubkey.len() != 66 {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !requester_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !requester_pubkey.starts_with("02") && !requester_pubkey.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid requester public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        let k_replies_collection = self.db_manager.get_k_replies_collection();
        let k_votes_collection = self.db_manager.get_k_votes_collection();
        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();

        // Build query to find all replies by this user
        let mut query = doc! { "sender_pubkey": user_public_key };
        
        if let Some(before_timestamp) = before {
            query.insert("block_time", doc! { "$lt": before_timestamp as i64 });
        }
        
        if let Some(after_timestamp) = after {
            query.insert("block_time", doc! { "$gt": after_timestamp as i64 });
        }

        // Always sort descending (newest first) for consistency
        let sort_order = doc! { "block_time": -1 };

        // Query database with proper pagination: fetch one extra to determine hasMore
        let query_limit = (limit + 1) as u64;
        
        let query_result = match k_replies_collection
            .find(query)
            .sort(sort_order)
            .limit(query_limit)
            .run() {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while querying paginated user replies for user {}: {}", user_public_key, err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };
        
        let mut all_replies = Vec::new();
        
        for item in query_result {
            match item {
                Ok(k_reply_record) => {
                    // Calculate replies count for this reply (nested replies)
                    let replies_count = match k_replies_collection
                        .find(doc! { "post_id": &k_reply_record.transaction_id })
                        .run() {
                        Ok(cursor) => {
                            cursor.count() as u64
                        },
                        Err(err) => {
                            log_error!("Error counting replies for reply {}: {}", k_reply_record.transaction_id, err);
                            0
                        }
                    };
                    
                    // Calculate vote counts and user's vote status
                    let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = 
                        self.get_vote_data(&k_reply_record.transaction_id, requester_pubkey, &k_votes_collection);
                    
                    let mut server_reply = ServerReply::from_k_reply_record_with_replies_count_and_votes(
                        &k_reply_record, 
                        replies_count,
                        up_votes_count,
                        down_votes_count,
                        is_upvoted,
                        is_downvoted
                    );
                    
                    // Enrich with user profile data from broadcasts
                    self.enrich_post_with_user_profile(&mut server_reply, &k_broadcasts_collection);
                    
                    all_replies.push(server_reply);
                }
                Err(err) => {
                    log_error!("Error reading K reply record: {}", err);
                    continue;
                }
            }
        }

        // Determine if there are more replies available
        let has_more = all_replies.len() > limit as usize;
        
        // Take only the requested number of replies (remove the extra one used for hasMore detection)
        if has_more {
            all_replies.truncate(limit as usize);
        }

        // Calculate pagination cursors
        let next_cursor = if has_more && !all_replies.is_empty() {
            // For next page (older replies), use the timestamp of the last reply
            Some(all_replies.last().unwrap().timestamp.to_string())
        } else {
            None
        };
        
        let prev_cursor = if !all_replies.is_empty() {
            // For checking newer replies, use the timestamp of the first reply
            Some(all_replies.first().unwrap().timestamp.to_string())
        } else {
            None
        };

        let pagination = PaginationMetadata {
            has_more,
            next_cursor,
            prev_cursor,
        };

        let response = PaginatedRepliesResponse { replies: all_replies, pagination };
        
        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize paginated user replies response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }/// GET /get-mentions with pagination
    /// Fetch paginated posts and replies where a specific user has been mentioned with voting status
    pub async fn get_mentions_paginated(&self, user_public_key: &str, requester_pubkey: &str, limit: u32, before: Option<u64>, after: Option<u64>) -> Result<String, String> {

        // Validate user public key format (66 hex characters for compressed public key)
        if user_public_key.len() != 66 {
            return Err(self.create_error_response(
                "Invalid user public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !user_public_key.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid user public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !user_public_key.starts_with("02") && !user_public_key.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid user public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate requester public key format (66 hex characters for compressed public key)
        if requester_pubkey.len() != 66 {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !requester_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !requester_pubkey.starts_with("02") && !requester_pubkey.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid requester public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        let k_posts_collection = self.db_manager.get_k_posts_collection();
        let k_replies_collection = self.db_manager.get_k_replies_collection();
        let k_votes_collection = self.db_manager.get_k_votes_collection();
        let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();

        // Since PoloDB query operators don't work for array matching, we'll use manual search
        // but with proper pagination logic that collects results in chronological order
        
        let target_user = user_public_key.to_string();
        let mut all_mentions: Vec<ServerPost> = Vec::new();
        
        // First, collect ALL matching mentions from both posts and replies
        // Then we'll sort and apply pagination correctly
        
        // Search through posts (all posts, we'll filter and paginate after)
        let posts_cursor = match k_posts_collection
            .find(doc! {})
            .sort(doc! { "block_time": -1 })  // Most recent first
            .run() 
        {
            Ok(cursor) => cursor,
            Err(err) => {
                log_error!("Error getting posts for manual search: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during manual search",
                    "DATABASE_ERROR",
                ));
            }
        };
        
        for item in posts_cursor {
            match item {
                Ok(k_post_record) => {
                    // Check if this post mentions our target user
                    if k_post_record.mentioned_pubkeys.contains(&target_user) {
                        // Calculate replies count for this post
                        let replies_count = match k_replies_collection
                            .find(doc! { "post_id": &k_post_record.transaction_id })
                            .run() {
                            Ok(cursor) => cursor.count() as u64,
                            Err(err) => {
                                log_error!("Error counting replies for post {}: {}", k_post_record.transaction_id, err);
                                0
                            }
                        };
                        
                        // Calculate vote counts and user's vote status
                        let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = 
                            self.get_vote_data(&k_post_record.transaction_id, requester_pubkey, &k_votes_collection);
                        
                        let mut server_post = ServerPost::from_k_post_record_with_replies_count_and_votes(
                            &k_post_record, 
                            replies_count,
                            up_votes_count,
                            down_votes_count,
                            is_upvoted,
                            is_downvoted
                        );
                        
                        // Enrich with user profile data from broadcasts
                        self.enrich_post_with_user_profile(&mut server_post, &k_broadcasts_collection);
                        
                        all_mentions.push(server_post);
                    }
                }
                Err(err) => {
                    log_error!("Error reading post during manual search: {}", err);
                }
            }
        }
        
        // Search through replies (all replies, we'll filter and paginate after)
        let replies_cursor = match k_replies_collection
            .find(doc! {})
            .sort(doc! { "block_time": -1 })  // Most recent first
            .run() 
        {
            Ok(cursor) => cursor,
            Err(err) => {
                log_error!("Error getting replies for manual search: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during manual search",
                    "DATABASE_ERROR",
                ));
            }
        };
        
        for item in replies_cursor {
            match item {
                Ok(k_reply_record) => {
                    // Check if this reply mentions our target user
                    if k_reply_record.mentioned_pubkeys.contains(&target_user) {
                        // Calculate replies count for this reply (nested replies)
                        let replies_count = match k_replies_collection
                            .find(doc! { "post_id": &k_reply_record.transaction_id })
                            .run() {
                            Ok(cursor) => cursor.count() as u64,
                            Err(err) => {
                                log_error!("Error counting replies for reply {}: {}", k_reply_record.transaction_id, err);
                                0
                            }
                        };
                        
                        // Calculate vote counts and user's vote status
                        let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = 
                            self.get_vote_data(&k_reply_record.transaction_id, requester_pubkey, &k_votes_collection);
                        
                        let mut server_reply = ServerReply::from_k_reply_record_with_replies_count_and_votes(
                            &k_reply_record, 
                            replies_count,
                            up_votes_count,
                            down_votes_count,
                            is_upvoted,
                            is_downvoted
                        );
                        
                        // Enrich with user profile data from broadcasts
                        self.enrich_post_with_user_profile(&mut server_reply, &k_broadcasts_collection);
                        
                        all_mentions.push(server_reply);
                    }
                }
                Err(err) => {
                    log_error!("Error reading reply during manual search: {}", err);
                }
            }
        }

        // Sort all posts and replies by timestamp (newest first)
        all_mentions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply cursor filtering AFTER collecting and sorting all results
        let mut filtered_mentions = Vec::new();
        
        for mention in all_mentions {
            // Apply cursor filtering
            if let Some(before_timestamp) = before {
                if mention.timestamp >= before_timestamp {
                    continue;
                }
            }
            if let Some(after_timestamp) = after {
                if mention.timestamp <= after_timestamp {
                    continue;
                }
            }
            
            filtered_mentions.push(mention);
        }

        // Apply pagination logic: fetch limit + 1 to determine hasMore
        let has_more = filtered_mentions.len() > limit as usize;
        
        // Take only the requested number of mentions
        if has_more {
            filtered_mentions.truncate(limit as usize);
        }

        // Calculate pagination cursors
        let next_cursor = if has_more && !filtered_mentions.is_empty() {
            // For next page (older posts), use the timestamp of the last post
            Some(filtered_mentions.last().unwrap().timestamp.to_string())
        } else {
            None
        };
        
        let prev_cursor = if !filtered_mentions.is_empty() {
            // For checking newer posts, use the timestamp of the first post
            Some(filtered_mentions.first().unwrap().timestamp.to_string())
        } else {
            None
        };

        let pagination = PaginationMetadata {
            has_more,
            next_cursor,
            prev_cursor,
        };

        let response = PaginatedPostsResponse { posts: filtered_mentions, pagination };
        
        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize paginated mentions response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-post-details?id={postId}&requesterPubkey={requesterPubkey}
    /// Fetch details for a specific post or reply by its ID with voting information for the requesting user
    pub async fn get_post_details_with_votes(&self, post_id: &str, requester_pubkey: &str) -> Result<String, String> {

        // Validate post ID format (64 hex characters for transaction hash)
        if post_id.len() != 64 {
            return Err(self.create_error_response(
                "Invalid post ID format. Must be 64 hex characters.",
                "INVALID_POST_ID",
            ));
        }

        if !post_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid post ID format. Must contain only hex characters.",
                "INVALID_POST_ID",
            ));
        }

        // Validate requester public key format (66 hex characters for compressed public key)
        if requester_pubkey.len() != 66 {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !requester_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid requester public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !requester_pubkey.starts_with("02") && !requester_pubkey.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid requester public key format. Compressed public key must start with 02 or 03.",
                "INVALID_USER_KEY",
            ));
        }

        let k_posts_collection = self.db_manager.get_k_posts_collection();
        let k_replies_collection = self.db_manager.get_k_replies_collection();
        let k_votes_collection = self.db_manager.get_k_votes_collection();

        // First, try to find the post in the k-posts collection
        let post_query = doc! { "transaction_id": post_id };
        
        if let Ok(mut post_cursor) = k_posts_collection.find(post_query).run() {
            if let Some(Ok(k_post_record)) = post_cursor.next() {
                // Found in posts collection - calculate replies count
                let replies_count = match k_replies_collection
                    .find(doc! { "post_id": post_id })
                    .run() {
                    Ok(cursor) => cursor.count() as u64,
                    Err(err) => {
                        log_error!("Error counting replies for post {}: {}", post_id, err);
                        0
                    }
                };

                // Calculate vote counts and user's vote status
                let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = 
                    self.get_vote_data(post_id, requester_pubkey, &k_votes_collection);
                
                let mut server_post = ServerPost::from_k_post_record_with_replies_count_and_votes(
                        &k_post_record, 
                        replies_count,
                        up_votes_count,
                        down_votes_count,
                        is_upvoted,
                        is_downvoted
                    );
                    
                    // Enrich with user profile data from broadcasts
                    let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();
                    self.enrich_post_with_user_profile(&mut server_post, &k_broadcasts_collection);
                    
                    let response = PostDetailsResponse { post: server_post };
                
                return match serde_json::to_string(&response) {
                    Ok(json) => Ok(json),
                    Err(err) => {
                        log_error!("Failed to serialize post details response: {}", err);
                        Err(self.create_error_response(
                            "Internal server error during serialization",
                            "SERIALIZATION_ERROR",
                        ))
                    }
                };
            }
        }

        // If not found in posts collection, try the k-replies collection
        let reply_query = doc! { "transaction_id": post_id };
        
        if let Ok(mut reply_cursor) = k_replies_collection.find(reply_query).run() {
            if let Some(Ok(k_reply_record)) = reply_cursor.next() {
                // Found in replies collection - calculate nested replies count
                let replies_count = match k_replies_collection
                    .find(doc! { "post_id": post_id })
                    .run() {
                    Ok(cursor) => cursor.count() as u64,
                    Err(err) => {
                        log_error!("Error counting replies for reply {}: {}", post_id, err);
                        0
                    }
                };

                // Calculate vote counts and user's vote status
                let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = 
                    self.get_vote_data(post_id, requester_pubkey, &k_votes_collection);
                
                let mut server_reply = ServerReply::from_k_reply_record_with_replies_count_and_votes(
                    &k_reply_record, 
                    replies_count,
                    up_votes_count,
                    down_votes_count,
                    is_upvoted,
                    is_downvoted
                );
                
                // Enrich with user profile data from broadcasts
                let k_broadcasts_collection = self.db_manager.get_k_broadcasts_collection();
                self.enrich_post_with_user_profile(&mut server_reply, &k_broadcasts_collection);
                
                let response = PostDetailsResponse { post: server_reply };
                
                return match serde_json::to_string(&response) {
                    Ok(json) => Ok(json),
                    Err(err) => {
                        log_error!("Failed to serialize reply details response: {}", err);
                        Err(self.create_error_response(
                            "Internal server error during serialization",
                            "SERIALIZATION_ERROR",
                        ))
                    }
                };
            }
        }

        // Post/reply not found in either collection
        Err(self.create_error_response(
            "Post not found",
            "NOT_FOUND",
        ))
    }

    /// GET /get-post-details?id={postId} (legacy without voting info)
    /// Fetch details for a specific post or reply by its ID
    pub async fn get_post_details(&self, post_id: &str) -> Result<String, String> {
        // Call the new method with empty requester_pubkey to maintain backward compatibility
        // This will return voting fields as None
        self.get_post_details_with_votes(post_id, "").await
    }

    /// Get vote data for a specific post and requester
    /// Returns (up_votes_count, down_votes_count, is_upvoted, is_downvoted)
    fn get_vote_data(&self, post_id: &str, requester_pubkey: &str, k_votes_collection: &polodb_core::Collection<crate::models::KVoteRecord>) -> (u64, u64, bool, bool) {
        // If no requester_pubkey is provided, skip the user-specific vote check
        if requester_pubkey.is_empty() {
            let votes_cursor = match k_votes_collection
                .find(doc! { "post_id": post_id })
                .run() {
                Ok(cursor) => cursor,
                Err(err) => {
                    log_error!("Error querying votes for post {}: {}", post_id, err);
                    return (0, 0, false, false);
                }
            };

            let mut up_votes_count = 0u64;
            let mut down_votes_count = 0u64;

            for vote_result in votes_cursor {
                match vote_result {
                    Ok(vote_record) => {
                        match vote_record.vote.as_str() {
                            "upvote" => up_votes_count += 1,
                            "downvote" => down_votes_count += 1,
                            _ => log_error!("Invalid vote value found in database: {}", vote_record.vote),
                        }
                    },
                    Err(err) => {
                        log_error!("Error reading vote record for post {}: {}", post_id, err);
                    }
                }
            }

            return (up_votes_count, down_votes_count, false, false);
        }
        // Get all votes for this post
        let votes_cursor = match k_votes_collection
            .find(doc! { "post_id": post_id })
            .run() {
            Ok(cursor) => cursor,
            Err(err) => {
                log_error!("Error querying votes for post {}: {}", post_id, err);
                return (0, 0, false, false);
            }
        };

        let mut up_votes_count = 0u64;
        let mut down_votes_count = 0u64;
        let mut is_upvoted = false;
        let mut is_downvoted = false;

        for vote_result in votes_cursor {
            match vote_result {
                Ok(vote_record) => {
                    match vote_record.vote.as_str() {
                        "upvote" => {
                            up_votes_count += 1;
                            if vote_record.sender_pubkey == requester_pubkey {
                                is_upvoted = true;
                            }
                        },
                        "downvote" => {
                            down_votes_count += 1;
                            if vote_record.sender_pubkey == requester_pubkey {
                                is_downvoted = true;
                            }
                        },
                        _ => {
                            log_error!("Invalid vote value found in database: {}", vote_record.vote);
                        }
                    }
                },
                Err(err) => {
                    log_error!("Error reading vote record for post {}: {}", post_id, err);
                }
            }
        }

        (up_votes_count, down_votes_count, is_upvoted, is_downvoted)
    }

    /// Enrich a post with user profile information from broadcasts
    fn enrich_post_with_user_profile(
        &self,
        post: &mut ServerPost,
        k_broadcasts_collection: &polodb_core::Collection<crate::models::KBroadcastRecord>
    ) {
        // Look up the latest broadcast for this user's public key
        let broadcasts_query = doc! { "sender_pubkey": &post.user_public_key };
        
        match k_broadcasts_collection
            .find(broadcasts_query)
            .sort(doc! { "block_time": -1 }) // Latest first
            .limit(1)
            .run() {
            Ok(mut cursor) => {
                match cursor.next() {
                    Some(Ok(broadcast_record)) => {
                        // Set user profile fields if available
                        post.user_nickname = Some(broadcast_record.base64_encoded_nickname);
                        post.user_profile_image = broadcast_record.base64_encoded_profile_image;
                    },
                    Some(Err(_)) => {
                        // Failed to deserialize record - likely old format without required fields
                        // Return empty strings as fallback
                        post.user_nickname = Some(String::new());
                        post.user_profile_image = Some(String::new());
                    },
                    None => {
                        // No broadcast record found for this user
                        post.user_nickname = Some(String::new());
                        post.user_profile_image = Some(String::new());
                    }
                }
            },
            Err(err) => {
                log_error!("Error querying broadcasts for user {}: {}", post.user_public_key, err);
                // Return empty strings on database error
                post.user_nickname = Some(String::new());
                post.user_profile_image = Some(String::new());
            }
        }
    }

    /// Enrich a user post with user profile information from broadcasts  
    fn enrich_user_post_with_user_profile(
        &self,
        post: &mut ServerUserPost,
        k_broadcasts_collection: &polodb_core::Collection<crate::models::KBroadcastRecord>
    ) {
        // Look up the latest broadcast for this user's public key
        let broadcasts_query = doc! { "sender_pubkey": &post.user_public_key };
        
        match k_broadcasts_collection
            .find(broadcasts_query)
            .sort(doc! { "block_time": -1 }) // Latest first
            .limit(1)
            .run() {
            Ok(mut cursor) => {
                match cursor.next() {
                    Some(Ok(broadcast_record)) => {
                        // Set user profile fields if available
                        post.user_nickname = Some(broadcast_record.base64_encoded_nickname);
                        post.user_profile_image = broadcast_record.base64_encoded_profile_image;
                    },
                    Some(Err(_)) => {
                        // Failed to deserialize record - likely old format without required fields
                        // Return empty strings as fallback
                        post.user_nickname = Some(String::new());
                        post.user_profile_image = Some(String::new());
                    },
                    None => {
                        // No broadcast record found for this user
                        post.user_nickname = Some(String::new());
                        post.user_profile_image = Some(String::new());
                    }
                }
            },
            Err(err) => {
                log_error!("Error querying broadcasts for user {}: {}", post.user_public_key, err);
                // Return empty strings on database error
                post.user_nickname = Some(String::new());
                post.user_profile_image = Some(String::new());
            }
        }
    }

    /// Create a standardized error response
    fn create_error_response(&self, message: &str, code: &str) -> String {
        let error = ApiError {
            error: message.to_string(),
            code: code.to_string(),
        };
        
        serde_json::to_string(&error).unwrap_or_else(|_| {
            r#"{"error":"Internal error creating error response","code":"INTERNAL_ERROR"}"#.to_string()
        })
    }
}