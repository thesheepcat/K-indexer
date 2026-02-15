use crate::database_trait::{DatabaseInterface, QueryOptions};
use crate::models::{
    ApiError, ContentRecord, NotificationPost, PaginatedNotificationsResponse,
    PaginatedPostsResponse, PaginatedRepliesResponse, PaginatedUsersResponse, PostDetailsResponse,
    ServerPost, ServerReply, ServerUserPost,
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

        // Use the new k_contents table method with blocking awareness
        let posts_result = match self
            .db
            .get_posts_by_user(user_public_key, requester_pubkey, options)
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

        // Convert enriched KPostRecords to ServerPosts (blocked users already excluded)
        let all_posts: Vec<ServerPost> = posts_result
            .items
            .iter()
            .map(|post_record| {
                ServerPost::from_enriched_k_post_record_with_block_status(post_record, false)
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

        // Use the new k_contents table query method with blocking awareness
        let posts_result = match self.db.get_all_posts(requester_pubkey, options).await {
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

        // Convert enriched KPostRecords to ServerPosts (blocked users already excluded)
        let all_posts: Vec<ServerPost> = posts_result
            .items
            .iter()
            .map(|post_record| {
                ServerPost::from_enriched_k_post_record_with_block_status(post_record, false)
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

    /// GET /get-content-following with pagination
    /// Fetch paginated content (posts, replies, quotes) from followed users
    pub async fn get_content_following_paginated(
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

        // Get content from followed users
        let content_result = match self
            .db
            .get_content_following(requester_pubkey, options)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying content from followed users: {}",
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Convert enriched KPostRecords to ServerPosts (blocked users already excluded)
        let all_posts: Vec<ServerPost> = content_result
            .items
            .iter()
            .map(|post_record| {
                ServerPost::from_enriched_k_post_record_with_block_status(post_record, false)
            })
            .collect();

        let response = PaginatedPostsResponse {
            posts: all_posts,
            pagination: content_result.pagination,
        };

        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize paginated content response: {}", err);
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

        let broadcasts_result = match self.db.get_all_users(requester_pubkey, options).await {
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

        for (k_broadcast_record, is_blocked, is_followed) in broadcasts_result.items {
            let mut server_user_post = ServerUserPost::from_k_broadcast_record_with_block_status(
                &k_broadcast_record,
                is_blocked,
            );

            // Enrich with user profile data from broadcasts (self-enrichment)
            server_user_post.user_nickname = Some(k_broadcast_record.base64_encoded_nickname);
            server_user_post.user_profile_image = k_broadcast_record.base64_encoded_profile_image;
            server_user_post.followed_user = Some(is_followed);

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

    /// GET /get-most-active-users with pagination
    /// Fetch users ranked by total content count (posts, replies, quotes) in k_contents
    /// within a specific time window
    pub async fn get_most_active_users_paginated(
        &self,
        limit: u32,
        requester_pubkey: &str,
        time_window: &str,
        before: Option<String>,
        after: Option<String>,
    ) -> Result<String, String> {
        use std::time::{SystemTime, UNIX_EPOCH};

        // Calculate time window in milliseconds (block_time is stored in milliseconds)
        let window_millis = match time_window {
            "1h" => 3_600_000_u64,
            "6h" => 21_600_000_u64,
            "24h" => 86_400_000_u64,
            "7d" => 604_800_000_u64,
            "30d" => 2_592_000_000_u64,
            _ => {
                return Err(self
                    .create_error_response("Invalid time window parameter", "INVALID_PARAMETER"));
            }
        };

        let to_time_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let from_time_millis = to_time_millis.saturating_sub(window_millis);

        let options = QueryOptions {
            limit: Some(limit as u64),
            before,
            after,
            sort_descending: true,
        };

        let result = match self
            .db
            .get_most_active_users(requester_pubkey, options, from_time_millis, to_time_millis)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while querying most active users: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        let mut all_posts = Vec::new();

        for (k_broadcast_record, is_blocked, is_followed, content_count) in result.items {
            let mut server_user_post = ServerUserPost::from_k_broadcast_record_with_block_status(
                &k_broadcast_record,
                is_blocked,
            );

            server_user_post.user_nickname = Some(k_broadcast_record.base64_encoded_nickname);
            server_user_post.user_profile_image = k_broadcast_record.base64_encoded_profile_image;
            server_user_post.followed_user = Some(is_followed);
            server_user_post.contents_count = Some(content_count);

            all_posts.push(server_user_post);
        }

        let response = PaginatedUsersResponse {
            posts: all_posts,
            pagination: result.pagination,
        };

        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize most active users response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    /// GET /search-users with pagination
    /// Search users with optional filters for pubkey or nickname
    pub async fn search_users_paginated(
        &self,
        limit: u32,
        requester_pubkey: &str,
        before: Option<String>,
        after: Option<String>,
        searched_user_pubkey: Option<String>,
        searched_user_nickname: Option<String>,
    ) -> Result<String, String> {
        // Validate searched_user_pubkey if provided
        if let Some(ref pubkey) = searched_user_pubkey {
            if pubkey.len() != 66 {
                return Err(self.create_error_response(
                    "Invalid searched user public key format. Must be 66 hex characters.",
                    "INVALID_USER_KEY",
                ));
            }
            if !pubkey.starts_with("02") && !pubkey.starts_with("03") {
                return Err(self.create_error_response(
                    "Invalid searched user public key prefix. Must start with 02 or 03.",
                    "INVALID_USER_KEY",
                ));
            }
        }

        let options = QueryOptions {
            limit: Some(limit as u64),
            before,
            after,
            sort_descending: true,
        };

        // Strip the 02/03 prefix from the searched pubkey to match both variants
        let searched_pubkey_without_prefix = searched_user_pubkey.map(|pk| pk[2..].to_string());

        let broadcasts_result = match self
            .db
            .search_users(
                requester_pubkey,
                options,
                searched_pubkey_without_prefix,
                searched_user_nickname,
            )
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!("Database error while searching users: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        let mut all_posts = Vec::new();

        for (k_broadcast_record, is_blocked, is_followed) in broadcasts_result.items {
            let mut server_user_post = ServerUserPost::from_k_broadcast_record_with_block_status(
                &k_broadcast_record,
                is_blocked,
            );

            // Enrich with user profile data from broadcasts (self-enrichment)
            server_user_post.user_nickname = Some(k_broadcast_record.base64_encoded_nickname);
            server_user_post.user_profile_image = k_broadcast_record.base64_encoded_profile_image;
            server_user_post.followed_user = Some(is_followed);

            all_posts.push(server_user_post);
        }

        let response = PaginatedUsersResponse {
            posts: all_posts,
            pagination: broadcasts_result.pagination,
        };

        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize search users response: {}", err);
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

        // Use the new k_contents table method with blocking awareness
        let replies_result = match self
            .db
            .get_replies_by_post_id(post_id, requester_pubkey, options)
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

        // Convert enriched KReplyRecords to ServerReplies (blocked users already excluded)
        let all_replies: Vec<ServerReply> = replies_result
            .items
            .iter()
            .map(|reply_record| {
                ServerReply::from_enriched_k_reply_record_with_block_status(reply_record, false)
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

        // Use the new k_contents table method with blocking awareness
        let replies_result = match self
            .db
            .get_replies_by_user(user_public_key, requester_pubkey, options)
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

        // Convert enriched KReplyRecords to ServerReplies (blocked users already excluded)
        let all_replies: Vec<ServerReply> = replies_result
            .items
            .iter()
            .map(|reply_record| {
                ServerReply::from_enriched_k_reply_record_with_block_status(reply_record, false)
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

        // Use the new k_contents table method with blocking awareness
        let mentions_result = match self
            .db
            .get_contents_mentioning_user(user_public_key, requester_pubkey, options)
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

        // Convert enriched ContentRecords (posts and replies) to ServerPosts (blocked users already excluded)
        let all_mentions: Vec<ServerPost> = mentions_result
            .items
            .iter()
            .map(|content_record| match content_record {
                ContentRecord::Post(post_record) => {
                    ServerPost::from_enriched_k_post_record_with_block_status(post_record, false)
                }
                ContentRecord::Reply(reply_record) => {
                    ServerReply::from_enriched_k_reply_record_with_block_status(reply_record, false)
                }
                ContentRecord::Vote(_vote_record) => {
                    // For get-mentions, votes are returned as ServerReply (same structure as ServerPost)
                    // but with minimal content since votes don't have message content
                    ServerPost {
                        id: _vote_record.transaction_id.clone(),
                        user_public_key: _vote_record.sender_pubkey.clone(),
                        post_content: String::new(), // Votes don't have content in mentions
                        signature: _vote_record.sender_signature.clone(),
                        timestamp: _vote_record.block_time,
                        replies_count: 0,
                        quotes_count: 0,
                        up_votes_count: 0,
                        down_votes_count: 0,
                        reposts_count: 0,
                        parent_post_id: Some(_vote_record.post_id.clone()),
                        mentioned_pubkeys: Vec::new(),
                        is_upvoted: None,
                        is_downvoted: None,
                        user_nickname: _vote_record.user_nickname.clone(),
                        user_profile_image: _vote_record.user_profile_image.clone(),
                        blocked_user: Some(false),
                        content_type: Some("vote".to_string()),
                        is_quote: false,
                        quote: None,
                    }
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

        // Use the new k_contents table method to get notifications with content details
        let notifications_result = match self.db.get_notifications(requester_pubkey, options).await
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

        // Convert NotificationContentRecords to NotificationPost with correct mention cursors
        let all_notifications: Vec<NotificationPost> = notifications_result
            .items
            .iter()
            .map(|notification_record| match &notification_record.content {
                ContentRecord::Post(post_record) => {
                    NotificationPost::from_k_post_record_with_mention_cursor(
                        &post_record,
                        notification_record.mention_id,
                        notification_record.mention_block_time,
                    )
                }
                ContentRecord::Reply(reply_record) => {
                    NotificationPost::from_k_reply_record_with_mention_cursor(
                        &reply_record,
                        notification_record.mention_id,
                        notification_record.mention_block_time,
                    )
                }
                ContentRecord::Vote(vote_record) => {
                    // For votes, we now have enriched vote record with all necessary data
                    NotificationPost::from_k_vote_record_with_mention_cursor(
                        &vote_record,
                        notification_record.mention_id,
                        notification_record.mention_block_time,
                        vote_record.voted_content.clone().unwrap_or_default(),
                        vote_record.user_nickname.clone(),
                        vote_record.user_profile_image.clone(),
                    )
                }
            })
            .collect();

        let pagination = notifications_result.pagination;

        let response = PaginatedNotificationsResponse {
            notifications: all_notifications,
            pagination,
        };

        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!(
                    "Failed to serialize paginated notifications response: {}",
                    err
                );
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

        // Use the new k_contents table function to get content with metadata and block status
        match self
            .db
            .get_content_by_id(content_id, requester_pubkey)
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
                    ContentRecord::Vote(k_vote_record) => {
                        // For get-post-details, votes are returned as ServerPost with vote-specific info
                        let server_vote = ServerPost {
                            id: k_vote_record.transaction_id.clone(),
                            user_public_key: k_vote_record.sender_pubkey.clone(),
                            post_content: String::new(), // Votes don't have content
                            signature: k_vote_record.sender_signature.clone(),
                            timestamp: k_vote_record.block_time,
                            replies_count: 0,
                            quotes_count: 0,
                            up_votes_count: 0,
                            down_votes_count: 0,
                            reposts_count: 0,
                            parent_post_id: Some(k_vote_record.post_id.clone()),
                            mentioned_pubkeys: Vec::new(),
                            is_upvoted: None,
                            is_downvoted: None,
                            user_nickname: k_vote_record.user_nickname.clone(),
                            user_profile_image: k_vote_record.user_profile_image.clone(),
                            blocked_user: Some(is_blocked),
                            content_type: Some("vote".to_string()),
                            is_quote: false,
                            quote: None,
                        };
                        PostDetailsResponse { post: server_vote }
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

        // Get the user's broadcast record from k_broadcast table with block/follow status
        let broadcast_result = match self
            .db
            .get_user_details(user_public_key, requester_pubkey)
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
            Some((record, blocked, followed, followers_count, following_count, blocked_count)) => {
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
                        followed_user: Some(followed), // Use the actual following status from database
                        followers_count: Some(followers_count),
                        following_count: Some(following_count),
                        blocked_count: Some(blocked_count),
                        contents_count: None,
                    }
                } else {
                    // User has real broadcast data
                    let mut user_post =
                        ServerUserPost::from_k_broadcast_record_with_block_and_follow_status(
                            &record, blocked, followed,
                        );
                    user_post.user_nickname = Some(record.base64_encoded_nickname);
                    user_post.user_profile_image = record.base64_encoded_profile_image;
                    user_post.followers_count = Some(followers_count);
                    user_post.following_count = Some(following_count);
                    user_post.blocked_count = Some(blocked_count);
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
                    followed_user: Some(false),
                    followers_count: Some(0),
                    following_count: Some(0),
                    blocked_count: Some(0),
                    contents_count: None,
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

    pub async fn get_followed_users_paginated(
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
            .get_followed_users_by_requester(requester_pubkey, options)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying followed users for requester {}: {}",
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

            // Since these are followed users, set followed_user to true
            server_user_post.followed_user = Some(true);

            // Remove post content for followed users
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
                    "Failed to serialize paginated followed users response: {}",
                    err
                );
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    pub async fn get_users_following_paginated(
        &self,
        requester_pubkey: &str,
        user_pubkey: &str,
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

        // Validate user public key format (66 hex characters for compressed public key)
        if user_pubkey.len() != 66 {
            return Err(self.create_error_response(
                "Invalid user public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !user_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid user public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !user_pubkey.starts_with("02") && !user_pubkey.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid user public key format. Compressed public key must start with 02 or 03.",
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
            .get_users_following(requester_pubkey, user_pubkey, options)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying users following for user {}: {}",
                    user_pubkey,
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        let mut all_posts = Vec::new();

        for (k_broadcast_record, is_followed_by_requester) in broadcasts_result.items {
            let mut server_user_post = ServerUserPost::from_k_broadcast_record(&k_broadcast_record);

            // Enrich with user profile data from broadcasts (self-enrichment)
            server_user_post.user_nickname = Some(k_broadcast_record.base64_encoded_nickname);
            server_user_post.user_profile_image = k_broadcast_record.base64_encoded_profile_image;

            // Set followed_user based on whether requester follows this user
            server_user_post.followed_user = Some(is_followed_by_requester);

            // Remove post content
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
                    "Failed to serialize paginated users following response: {}",
                    err
                );
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }

    pub async fn get_users_followers_paginated(
        &self,
        requester_pubkey: &str,
        user_pubkey: &str,
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

        // Validate user public key format (66 hex characters for compressed public key)
        if user_pubkey.len() != 66 {
            return Err(self.create_error_response(
                "Invalid user public key format. Must be 66 hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        if !user_pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(self.create_error_response(
                "Invalid user public key format. Must contain only hex characters.",
                "INVALID_USER_KEY",
            ));
        }

        // Validate compressed public key prefix (should start with 02 or 03)
        if !user_pubkey.starts_with("02") && !user_pubkey.starts_with("03") {
            return Err(self.create_error_response(
                "Invalid user public key format. Compressed public key must start with 02 or 03.",
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
            .get_users_followers(requester_pubkey, user_pubkey, options)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying users followers for user {}: {}",
                    user_pubkey,
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        let mut all_posts = Vec::new();

        for (k_broadcast_record, is_followed_by_requester) in broadcasts_result.items {
            let mut server_user_post = ServerUserPost::from_k_broadcast_record(&k_broadcast_record);

            // Enrich with user profile data from broadcasts (self-enrichment)
            server_user_post.user_nickname = Some(k_broadcast_record.base64_encoded_nickname);
            server_user_post.user_profile_image = k_broadcast_record.base64_encoded_profile_image;

            // Set followed_user based on whether requester follows this user
            server_user_post.followed_user = Some(is_followed_by_requester);

            // Remove post content
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
                    "Failed to serialize paginated users followers response: {}",
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

        // Get notification count from database
        match self
            .db
            .get_notification_count(requester_pubkey, after)
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

    pub async fn get_users_count(&self) -> Result<String, String> {
        // Get users count from database
        match self.db.get_users_count().await {
            Ok(count) => {
                let response = serde_json::json!({
                    "count": count
                });
                match serde_json::to_string(&response) {
                    Ok(json_response) => Ok(json_response),
                    Err(err) => {
                        log_error!("Failed to serialize users count response: {}", err);
                        Err(self.create_error_response(
                            "Internal server error during serialization",
                            "SERIALIZATION_ERROR",
                        ))
                    }
                }
            }
            Err(err) => {
                log_error!("Database error while getting users count: {}", err);
                Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ))
            }
        }
    }

    /// GET /get-hashtag-content with pagination
    /// Fetch paginated content (posts, replies, quotes) containing a specific hashtag
    pub async fn get_hashtag_content_paginated(
        &self,
        hashtag: &str,
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

        // Get content with this hashtag
        let content_result = match self
            .db
            .get_hashtag_content(hashtag, requester_pubkey, options)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                log_error!(
                    "Database error while querying hashtag content for #{}: {}",
                    hashtag,
                    err
                );
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Convert enriched KPostRecords to ServerPosts (blocked users already excluded)
        let all_posts: Vec<ServerPost> = content_result
            .items
            .iter()
            .map(|post_record| {
                ServerPost::from_enriched_k_post_record_with_block_status(post_record, false)
            })
            .collect();

        let response = PaginatedPostsResponse {
            posts: all_posts,
            pagination: content_result.pagination,
        };

        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!(
                    "Failed to serialize paginated hashtag content response: {}",
                    err
                );
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
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

    /// GET /get-trending-hashtags
    /// Fetch trending hashtags within a time window
    pub async fn get_trending_hashtags(
        &self,
        time_window: &str,
        limit: u32,
    ) -> Result<String, String> {
        use crate::models::{TrendingHashtag, TrendingHashtagsResponse};
        use std::time::{SystemTime, UNIX_EPOCH};

        // Calculate time window in milliseconds (block_time is stored in milliseconds)
        let window_millis = match time_window {
            "1h" => 3_600_000_u64,      // 1 hour = 3,600,000 ms
            "6h" => 21_600_000_u64,     // 6 hours = 21,600,000 ms
            "24h" => 86_400_000_u64,    // 24 hours = 86,400,000 ms
            "7d" => 604_800_000_u64,    // 7 days = 604,800,000 ms
            "30d" => 2_592_000_000_u64, // 30 days = 2,592,000,000 ms
            _ => {
                return Err(self
                    .create_error_response("Invalid time window parameter", "INVALID_PARAMETER"));
            }
        };

        // Get current Unix timestamp in milliseconds
        let to_time_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let from_time_millis = to_time_millis.saturating_sub(window_millis);

        // Convert to seconds for the response (API returns seconds in fromTime/toTime)
        let to_time = to_time_millis / 1000;
        let from_time = from_time_millis / 1000;

        // Query database for trending hashtags (using milliseconds for block_time comparison)
        let trending_hashtags = match self
            .db
            .get_trending_hashtags(from_time_millis, to_time_millis, limit)
            .await
        {
            Ok(hashtags) => hashtags,
            Err(err) => {
                log_error!("Database error while querying trending hashtags: {}", err);
                return Err(self.create_error_response(
                    "Internal server error during database query",
                    "DATABASE_ERROR",
                ));
            }
        };

        // Add rank to each hashtag
        let hashtags_with_rank: Vec<TrendingHashtag> = trending_hashtags
            .into_iter()
            .enumerate()
            .map(|(index, (hashtag, usage_count))| TrendingHashtag {
                hashtag,
                usage_count,
                rank: (index + 1) as u32,
            })
            .collect();

        let response = TrendingHashtagsResponse {
            time_window: time_window.to_string(),
            from_time,
            to_time,
            hashtags: hashtags_with_rank,
        };

        match serde_json::to_string(&response) {
            Ok(json) => Ok(json),
            Err(err) => {
                log_error!("Failed to serialize trending hashtags response: {}", err);
                Err(self.create_error_response(
                    "Internal server error during serialization",
                    "SERIALIZATION_ERROR",
                ))
            }
        }
    }
}
