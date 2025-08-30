use crate::database_trait::{DatabaseInterface, QueryOptions};
use crate::models::{
    ApiError, KPostRecord, KReplyRecord, PaginatedPostsResponse,
    PaginatedRepliesResponse, PaginatedUsersResponse, PostDetailsResponse,
    ServerPost, ServerReply, ServerUserPost,
};
use serde_json;
use std::sync::Arc;
use tracing::{error as log_error};

pub struct ApiHandlers {
    db: Arc<dyn DatabaseInterface>,
}

impl ApiHandlers {
    pub fn new(db: Arc<dyn DatabaseInterface>) -> Self {
        Self { db }
    }

    /// GET /get-posts with pagination
    /// Fetch paginated posts for a specific user with cursor-based pagination and voting status
    pub async fn get_posts_paginated(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        limit: u32,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<String, String> {
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

        let options = QueryOptions {
            limit: Some(limit as u64),
            before,
            after,
            sort_descending: true,
        };

        // Use the new optimized single-query method
        let posts_result = match self.db.get_posts_by_user_with_metadata(user_public_key, requester_pubkey, options).await {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying paginated posts with metadata for user {}: {}",
                    user_public_key,
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Convert enriched KPostRecords to ServerPosts using the new method
        let all_posts: Vec<ServerPost> = posts_result.items
            .iter()
            .map(ServerPost::from_enriched_k_post_record)
            .collect();

        let response = PaginatedPostsResponse {
            posts: all_posts,
            pagination: posts_result.pagination,
        };

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

    /// GET /get-posts-watching with pagination (OPTIMIZED VERSION)
    /// Fetch paginated posts for watching with cursor-based pagination and voting status
    /// Uses a single optimized database query to avoid N+1 query problem
    pub async fn get_posts_watching_paginated(
        &self,
        requester_pubkey: &str,
        limit: u32,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<String, String> {
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

        let options = QueryOptions {
            limit: Some(limit as u64),
            before,
            after,
            sort_descending: true,
        };

        // Use the new optimized single-query method
        let posts_result = match self.db.get_all_posts_with_metadata(requester_pubkey, options).await {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while querying paginated posts with metadata: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Convert enriched KPostRecords to ServerPosts using the new method
        let all_posts: Vec<ServerPost> = posts_result.items
            .iter()
            .map(ServerPost::from_enriched_k_post_record)
            .collect();

        let response = PaginatedPostsResponse {
            posts: all_posts,
            pagination: posts_result.pagination,
        };

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

    /// GET /get-users with pagination
    /// Fetch paginated user introduction posts with cursor-based pagination
    pub async fn get_users_paginated(
        &self,
        limit: u32,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<String, String> {
        let options = QueryOptions {
            limit: Some(limit as u64),
            before,
            after,
            sort_descending: true,
        };

        let broadcasts_result = match self.db.get_all_broadcasts(options).await {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying paginated user broadcasts: {}",
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        let mut all_posts = Vec::new();

        for k_broadcast_record in broadcasts_result.items {
            let mut server_user_post = ServerUserPost::from_k_broadcast_record(&k_broadcast_record);

            // Enrich with user profile data from broadcasts (self-enrichment)
            server_user_post.user_nickname = Some(k_broadcast_record.base64_encoded_nickname);
            server_user_post.user_profile_image = k_broadcast_record.base64_encoded_profile_image;

            all_posts.push(server_user_post);
        }

        let response = PaginatedUsersResponse {
            posts: all_posts,
            pagination: broadcasts_result.pagination,
        };

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

    /// GET /get-replies with pagination (Post Replies Mode)
    /// Fetch paginated replies for a specific post with cursor-based pagination and voting status
    pub async fn get_replies_paginated(
        &self,
        post_id: &str,
        requester_pubkey: &str,
        limit: u32,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<String, String> {
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

        let options = QueryOptions {
            limit: Some(limit as u64),
            before,
            after,
            sort_descending: true,
        };

        // Use the new optimized single-query method
        let replies_result = match self.db.get_replies_by_post_id_with_metadata(post_id, requester_pubkey, options).await {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying paginated replies with metadata for post {}: {}",
                    post_id,
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Convert enriched KReplyRecords to ServerReplies using the new method
        let all_replies: Vec<ServerReply> = replies_result.items
            .iter()
            .map(ServerReply::from_enriched_k_reply_record)
            .collect();

        let response = PaginatedRepliesResponse {
            replies: all_replies,
            pagination: replies_result.pagination,
        };

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

    /// GET /get-replies with pagination (User Replies Mode)
    /// Fetch paginated replies made by a specific user with cursor-based pagination and voting status
    pub async fn get_user_replies_paginated(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        limit: u32,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<String, String> {
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

        let options = QueryOptions {
            limit: Some(limit as u64),
            before,
            after,
            sort_descending: true,
        };

        // Use the new optimized single-query method
        let replies_result = match self.db.get_replies_by_user_with_metadata(user_public_key, requester_pubkey, options).await {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying paginated user replies with metadata for user {}: {}",
                    user_public_key,
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Convert enriched KReplyRecords to ServerReplies using the new method
        let all_replies: Vec<ServerReply> = replies_result.items
            .iter()
            .map(ServerReply::from_enriched_k_reply_record)
            .collect();

        let response = PaginatedRepliesResponse {
            replies: all_replies,
            pagination: replies_result.pagination,
        };

        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!(
                    "Failed to serialize paginated user replies response: {}",
                    err
                );
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-mentions with pagination
    /// Fetch paginated posts and replies where a specific user has been mentioned with voting status
    pub async fn get_mentions_paginated(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
        limit: u32,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<String, String> {
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

        // Fetch limit + 1 to check if there are more results
        let fetch_limit = limit + 1;
        let options = QueryOptions {
            limit: Some(fetch_limit as u64),
            before,
            after,
            sort_descending: true,
        };

        // Use the new optimized single-query method
        let mentions_result = match self
            .db
            .get_posts_mentioning_user_with_metadata(user_public_key, requester_pubkey, options)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!("Error getting mentions with metadata for user: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Convert enriched KPostRecords to ServerPosts using the new method
        let all_mentions: Vec<ServerPost> = mentions_result.items
            .iter()
            .map(ServerPost::from_enriched_k_post_record)
            .collect();

        let pagination = mentions_result.pagination;

        let response = PaginatedPostsResponse {
            posts: all_mentions,
            pagination,
        };

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
    pub async fn get_post_details_with_votes(
        &self,
        post_id: &str,
        requester_pubkey: &str,
    ) -> Result<String, String> {
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

        // First, try to find the post in the k-posts collection using optimized method
        if let Ok(Some(k_post_record)) = self.db.get_post_by_id_with_metadata(post_id, requester_pubkey).await {
            let server_post = ServerPost::from_enriched_k_post_record(&k_post_record);
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

        // If not found in posts collection, try the k-replies collection using optimized method
        if let Ok(Some(k_reply_record)) = self.db.get_reply_by_id_with_metadata(post_id, requester_pubkey).await {
            let server_reply = ServerReply::from_enriched_k_reply_record(&k_reply_record);
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

        // Post/reply not found in either collection
        Err(self.create_error_response("Post not found", "NOT_FOUND"))
    }

    // Helper method to enrich posts with metadata (replies count, votes, user profiles)
    async fn enrich_posts_with_metadata(
        &self,
        posts: Vec<KPostRecord>,
        requester_pubkey: &str,
    ) -> Vec<ServerPost> {
        let mut result = Vec::new();

        for k_post_record in posts {
            // Calculate replies count for this post
            let replies_count = match self
                .db
                .count_replies_for_post(&k_post_record.transaction_id)
                .await
            {
                Ok(count) => count,
                Err(err) => {
                    log_error!(
                        "Error counting replies for post {}: {}",
                        k_post_record.transaction_id,
                        err
                    );
                    0
                }
            };

            // Calculate vote counts and user's vote status
            let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = match self
                .db
                .get_vote_data(&k_post_record.transaction_id, requester_pubkey)
                .await
            {
                Ok(data) => data,
                Err(err) => {
                    log_error!(
                        "Error getting vote data for post {}: {}",
                        k_post_record.transaction_id,
                        err
                    );
                    (0, 0, false, false)
                }
            };

            let mut server_post = ServerPost::from_k_post_record_with_replies_count_and_votes(
                &k_post_record,
                replies_count,
                up_votes_count,
                down_votes_count,
                is_upvoted,
                is_downvoted,
            );

            // Enrich with user profile data from broadcasts
            match self
                .db
                .get_latest_broadcast_by_user(&k_post_record.sender_pubkey)
                .await
            {
                Ok(Some(broadcast)) => {
                    server_post.user_nickname = Some(broadcast.base64_encoded_nickname);
                    server_post.user_profile_image = broadcast.base64_encoded_profile_image;
                }
                Ok(None) => {
                    server_post.user_nickname = Some(String::new());
                    server_post.user_profile_image = Some(String::new());
                }
                Err(err) => {
                    log_error!(
                        "Error querying broadcasts for user {}: {}",
                        k_post_record.sender_pubkey,
                        err
                    );
                    server_post.user_nickname = Some(String::new());
                    server_post.user_profile_image = Some(String::new());
                }
            }

            result.push(server_post);
        }

        result
    }

    // Helper method to enrich replies with metadata (replies count, votes, user profiles)
    async fn enrich_replies_with_metadata(
        &self,
        replies: Vec<KReplyRecord>,
        requester_pubkey: &str,
    ) -> Vec<ServerReply> {
        let mut result = Vec::new();

        for k_reply_record in replies {
            // Calculate replies count for this reply (nested replies)
            let replies_count = match self
                .db
                .count_replies_for_post(&k_reply_record.transaction_id)
                .await
            {
                Ok(count) => count,
                Err(err) => {
                    log_error!(
                        "Error counting replies for reply {}: {}",
                        k_reply_record.transaction_id,
                        err
                    );
                    0
                }
            };

            // Calculate vote counts and user's vote status
            let (up_votes_count, down_votes_count, is_upvoted, is_downvoted) = match self
                .db
                .get_vote_data(&k_reply_record.transaction_id, requester_pubkey)
                .await
            {
                Ok(data) => data,
                Err(err) => {
                    log_error!(
                        "Error getting vote data for reply {}: {}",
                        k_reply_record.transaction_id,
                        err
                    );
                    (0, 0, false, false)
                }
            };

            let mut server_reply = ServerReply::from_k_reply_record_with_replies_count_and_votes(
                &k_reply_record,
                replies_count,
                up_votes_count,
                down_votes_count,
                is_upvoted,
                is_downvoted,
            );

            // Enrich with user profile data from broadcasts
            match self
                .db
                .get_latest_broadcast_by_user(&k_reply_record.sender_pubkey)
                .await
            {
                Ok(Some(broadcast)) => {
                    server_reply.user_nickname = Some(broadcast.base64_encoded_nickname);
                    server_reply.user_profile_image = broadcast.base64_encoded_profile_image;
                }
                Ok(None) => {
                    server_reply.user_nickname = Some(String::new());
                    server_reply.user_profile_image = Some(String::new());
                }
                Err(err) => {
                    log_error!(
                        "Error querying broadcasts for user {}: {}",
                        k_reply_record.sender_pubkey,
                        err
                    );
                    server_reply.user_nickname = Some(String::new());
                    server_reply.user_profile_image = Some(String::new());
                }
            }

            result.push(server_reply);
        }

        result
    }

    /// Create a standardized error response
    fn create_error_response(&self, message: &str, code: &str) -> String {
        let error = ApiError {
            error: message.to_string(),
            code: code.to_string(),
        };

        serde_json::to_string(&error).unwrap_or_else(|_| {
            r#"{"error":"Internal error creating error response","code":"INTERNAL_ERROR"}"#
                .to_string()
        })
    }
}
