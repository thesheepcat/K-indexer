use crate::database_trait::{DatabaseInterface, QueryOptions};
use crate::models::{
    ApiError, ContentRecord, PaginatedPostsResponse, PaginatedRepliesResponse,
    PaginatedUsersResponse, PostDetailsResponse, ServerPost, ServerReply, ServerUserPost,
};
use serde_json;
use std::sync::Arc;
use tracing::error as log_error;

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

        // Use the new optimized single-query method with blocking awareness
        let posts_result = match self
            .db
            .get_posts_by_user_with_metadata_and_block_status(
                user_public_key,
                requester_pubkey,
                options,
            )
            .await
        {
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

        // Convert enriched KPostRecords to ServerPosts with blocking awareness for get-posts
        let all_posts: Vec<ServerPost> = posts_result
            .items
            .iter()
            .map(|(post_record, is_blocked)| {
                ServerPost::from_enriched_k_post_record_with_block_status(post_record, *is_blocked)
            })
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

        // Use the new optimized single-query method with blocking awareness
        let posts_result = match self
            .db
            .get_all_posts_with_metadata_and_block_status(requester_pubkey, options)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying paginated posts with metadata: {}",
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Convert enriched KPostRecords to ServerPosts with blocking awareness for get-posts-watching
        let all_posts: Vec<ServerPost> = posts_result
            .items
            .iter()
            .map(|(post_record, is_blocked)| {
                ServerPost::from_enriched_k_post_record_with_block_status(post_record, *is_blocked)
            })
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

    /// GET /get-users with pagination and blocked users awareness
    /// Fetch paginated user introduction posts with cursor-based pagination and blocking status
    pub async fn get_users_paginated(
        &self,
        limit: u32,
        requester_pubkey: &str,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<String, String> {
        let options = QueryOptions {
            limit: Some(limit as u64),
            before,
            after,
            sort_descending: true,
        };

        let broadcasts_result = match self
            .db
            .get_all_broadcasts_with_block_status(requester_pubkey, options)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying paginated user broadcasts with block status: {}",
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        let mut all_posts = Vec::new();

        for (k_broadcast_record, is_blocked) in broadcasts_result.items {
            let mut server_user_post = ServerUserPost::from_k_broadcast_record_with_block_status(
                &k_broadcast_record,
                is_blocked,
            );

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
                log_error!(
                    "Failed to serialize paginated users response with block status: {}",
                    err
                );
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

        // Use the new optimized single-query method with blocking awareness
        let replies_result = match self
            .db
            .get_replies_by_post_id_with_metadata_and_block_status(
                post_id,
                requester_pubkey,
                options,
            )
            .await
        {
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

        // Convert enriched KReplyRecords to ServerReplies with blocking awareness for post replies
        let all_replies: Vec<ServerReply> = replies_result
            .items
            .iter()
            .map(|(reply_record, is_blocked)| {
                ServerReply::from_enriched_k_reply_record_with_block_status(
                    reply_record,
                    *is_blocked,
                )
            })
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

        // Use the new optimized single-query method with blocking awareness
        let replies_result = match self
            .db
            .get_replies_by_user_with_metadata_and_block_status(
                user_public_key,
                requester_pubkey,
                options,
            )
            .await
        {
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

        // Convert enriched KReplyRecords to ServerReplies with blocking awareness for user replies
        let all_replies: Vec<ServerReply> = replies_result
            .items
            .iter()
            .map(|(reply_record, is_blocked)| {
                ServerReply::from_enriched_k_reply_record_with_block_status(
                    reply_record,
                    *is_blocked,
                )
            })
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

        // Use the new optimized single-query method with blocking awareness
        let mentions_result = match self
            .db
            .get_contents_mentioning_user_with_metadata_and_block_status(
                user_public_key,
                requester_pubkey,
                options,
            )
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

        // Convert enriched ContentRecords (posts and replies) to ServerPosts with blocking awareness
        let all_mentions: Vec<ServerPost> = mentions_result
            .items
            .iter()
            .map(|(content_record, is_blocked)| match content_record {
                ContentRecord::Post(post_record) => {
                    ServerPost::from_enriched_k_post_record_with_block_status(
                        post_record,
                        *is_blocked,
                    )
                }
                ContentRecord::Reply(reply_record) => {
                    ServerReply::from_enriched_k_reply_record_with_block_status(
                        reply_record,
                        *is_blocked,
                    )
                }
            })
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

    /// GET /get-notifications?requesterPubkey={requesterPubkey}&limit={limit}&before={before}&after={after}
    /// Fetch notifications for a user based on mentions in k_mentions table with detailed content information
    pub async fn get_notifications_paginated(
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

        // Fetch limit + 1 to check if there are more results
        let fetch_limit = limit + 1;
        let options = QueryOptions {
            limit: Some(fetch_limit as u64),
            before,
            after,
            sort_descending: true,
        };

        // Use the database method to get notifications with content details
        let notifications_result = match self
            .db
            .get_notifications_with_content_details(
                requester_pubkey,
                options,
            )
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!("Error getting notifications for user: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Convert enriched ContentRecords (posts, replies, votes) to ServerPosts with blocking awareness
        let all_notifications: Vec<ServerPost> = notifications_result
            .items
            .iter()
            .map(|(content_record, is_blocked)| match content_record {
                ContentRecord::Post(post_record) => {
                    ServerPost::from_enriched_k_post_record_with_block_status(
                        &post_record,
                        *is_blocked,
                    )
                }
                ContentRecord::Reply(reply_record) => {
                    ServerReply::from_enriched_k_reply_record_with_block_status(
                        &reply_record,
                        *is_blocked,
                    )
                }
            })
            .collect();

        let pagination = notifications_result.pagination;

        let response = PaginatedPostsResponse {
            posts: all_notifications,
            pagination,
        };

        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize paginated notifications response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-post-details?id={postId}&requesterPubkey={requesterPubkey}
    /// Fetch details for a specific post or reply by its ID with voting information for the requesting user

    /// GET /get-post-details?id={postId}&requesterPubkey={requesterPubkey}
    /// Fetch details for a specific post or reply by its ID with voting information and blocking status for the requesting user
    pub async fn get_post_details(
        &self,
        content_id: &str,
        requester_pubkey: &str,
    ) -> Result<String, String> {
        // Validate content ID format (64 hex characters for transaction hash)
        if content_id.len() != 64 {
            return Err(self.create_error_response(
                "Invalid content ID format. Must be 64 hex characters.",
                "INVALID_POST_ID",
            ));
        }

        if !content_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid content ID format. Must contain only hex characters.",
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

        // Use the new blocking-aware function to get content with metadata and block status
        match self
            .db
            .get_content_by_id_with_metadata_and_block_status(content_id, requester_pubkey)
            .await
        {
            Ok(Some((content_record, is_blocked))) => {
                let response = match content_record {
                    ContentRecord::Post(k_post_record) => {
                        let server_post = ServerPost::from_enriched_k_post_record_with_block_status(
                            &k_post_record,
                            is_blocked,
                        );
                        PostDetailsResponse { post: server_post }
                    }
                    ContentRecord::Reply(k_reply_record) => {
                        let server_reply =
                            ServerReply::from_enriched_k_reply_record_with_block_status(
                                &k_reply_record,
                                is_blocked,
                            );
                        PostDetailsResponse { post: server_reply }
                    }
                };

                match serde_json::to_string(&response) {
                    Ok(json) => Ok(json),
                    Err(err) => {
                        log_error!("Failed to serialize content details response: {}", err);
                        Err(self.create_error_response(
                            "Internal server error during serialization",
                            "SERIALIZATION_ERROR",
                        ))
                    }
                }
            }
            Ok(None) => {
                // Content not found
                Err(self.create_error_response("Content not found", "NOT_FOUND"))
            }
            Err(err) => {
                log_error!(
                    "Database error while querying content by ID {}: {}",
                    content_id,
                    err
                );
                Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ))
            }
        }
    }

    /// GET /get-user-details with user parameter
    /// Fetch user details from k_broadcast table for a specific user public key
    pub async fn get_user_details(
        &self,
        user_public_key: &str,
        requester_pubkey: &str,
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

        // Get the user's broadcast record from k_broadcast table with block status
        let broadcast_result = match self
            .db
            .get_broadcast_by_user_with_block_status(user_public_key, requester_pubkey)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying user details for user {}: {}",
                    user_public_key,
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Handle user data (even if no broadcast exists)
        let server_user_post = match broadcast_result {
            Some((record, blocked)) => {
                // Check if this is a dummy record (no real broadcast data)
                if record.id == 0 && record.transaction_id.is_empty() {
                    // User has no broadcast data - create minimal response with empty fields
                    ServerUserPost {
                        id: String::new(),
                        user_public_key: user_public_key.to_string(),
                        post_content: String::new(),
                        signature: String::new(),
                        timestamp: 0,
                        user_nickname: None,
                        user_profile_image: None,
                        blocked_user: Some(blocked), // Use the actual blocking status from database
                    }
                } else {
                    // User has real broadcast data
                    let mut user_post =
                        ServerUserPost::from_k_broadcast_record_with_block_status(&record, blocked);
                    user_post.user_nickname = Some(record.base64_encoded_nickname);
                    user_post.user_profile_image = record.base64_encoded_profile_image;
                    user_post
                }
            }
            None => {
                // This case should no longer happen with our new implementation
                ServerUserPost {
                    id: String::new(),
                    user_public_key: user_public_key.to_string(),
                    post_content: String::new(),
                    signature: String::new(),
                    timestamp: 0,
                    user_nickname: None,
                    user_profile_image: None,
                    blocked_user: Some(false),
                }
            }
        };

        match serde_json::to_string(&server_user_post) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize user details response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-blocked-users with pagination
    /// Fetch paginated list of users blocked by the requester
    pub async fn get_blocked_users_paginated(
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

        let broadcasts_result = match self
            .db
            .get_blocked_users_by_requester(requester_pubkey, options)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying blocked users for requester {}: {}",
                    requester_pubkey,
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

            // Since these are blocked users, set blocked_user to true
            server_user_post.blocked_user = Some(true);

            // Remove post content for blocked users
            server_user_post.post_content = String::new();

            all_posts.push(server_user_post);
        }

        let response = PaginatedUsersResponse {
            posts: all_posts,
            pagination: broadcasts_result.pagination,
        };

        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!(
                    "Failed to serialize paginated blocked users response: {}",
                    err
                );
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /get-notifications-amount
    /// Get count of notifications for a specific user, optionally with cursor
    pub async fn get_notification_count(
        &self,
        requester_pubkey: &str,
        cursor: Option<String>,
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

        // Get notification count from database
        match self
            .db
            .get_notification_count(requester_pubkey, cursor)
            .await
        {
            Ok(count) => {
                let response = serde_json::json!({
                    "count": count
                });
                match serde_json::to_string(&response) {
                    Ok(json_response) => Ok(json_response),
                    Err(err) => {
                        log_error!("Failed to serialize notification count response: {}", err);
                        Err(self.create_error_response(
                            "Internal server error during serialization",
                            "SERIALIZATION_ERROR",
                        ))
                    }
                }
            }
            Err(err) => {
                log_error!(
                    "Database error while getting notification count for user {}: {}",
                    requester_pubkey,
                    err
                );
                Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ))
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
            r#"{"error":"Internal error creating error response","code":"INTERNAL_ERROR"}"#
                .to_string()
        })
    }
}
