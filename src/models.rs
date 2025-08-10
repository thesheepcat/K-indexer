use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// K Protocol Data Models

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KTransaction {
    pub transaction_id: String,
    pub block_time: u64,
    pub sender_address: String,
    pub receiver_address: String,
    pub payload: String,
    pub action_type: KActionType,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum KActionType {
    Broadcast(KBroadcast),
    Post(KPost),
    Reply(KReply),
    Vote(KVote),
    Unknown(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KBroadcast {
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub base64_encoded_message: String,
}

// Database model for K protocol broadcasts with additional metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KBroadcastRecord {
    pub transaction_id: String,
    pub block_time: u64,
    pub sender_address: String,
    pub receiver_address: String,
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub base64_encoded_message: String, // Stored as Base64 encoded string
    pub created_at: u64, // Timestamp when record was created
}

impl KBroadcastRecord {
    pub fn new(
        transaction_id: String,
        block_time: u64,
        sender_address: String,
        receiver_address: String,
        k_broadcast: KBroadcast,
    ) -> Self {
        Self {
            transaction_id,
            block_time,
            sender_address,
            receiver_address,
            sender_pubkey: k_broadcast.sender_pubkey,
            sender_signature: k_broadcast.sender_signature,
            base64_encoded_message: k_broadcast.base64_encoded_message, // Keep as Base64 encoded
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KPost {
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub base64_encoded_message: String,
    pub mentioned_pubkeys: Vec<String>,
}

// Database model for K protocol posts with additional metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KPostRecord {
    pub transaction_id: String,
    pub block_time: u64,
    pub sender_address: String,
    pub receiver_address: String,
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub base64_encoded_message: String, // Stored as Base64 encoded string
    pub mentioned_pubkeys: Vec<String>,
    pub created_at: u64, // Timestamp when record was created
}

impl KPostRecord {
    pub fn new(
        transaction_id: String,
        block_time: u64,
        sender_address: String,
        receiver_address: String,
        k_post: KPost,
    ) -> Self {
        Self {
            transaction_id,
            block_time,
            sender_address,
            receiver_address,
            sender_pubkey: k_post.sender_pubkey,
            sender_signature: k_post.sender_signature,
            base64_encoded_message: k_post.base64_encoded_message, // Keep as Base64 encoded
            mentioned_pubkeys: k_post.mentioned_pubkeys,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

// API Response models
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerPost {
    pub id: String,              // 32-byte cryptographic hash (64 hex characters)
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String, // 32-byte public key (64 hex characters)
    #[serde(rename = "postContent")]
    pub post_content: String,    // Base64 encoded content
    pub signature: String,       // Schnorr signature as hex string
    pub timestamp: u64,          // Unix timestamp
    #[serde(rename = "repliesCount")]
    pub replies_count: u64,     // Number of direct replies
    #[serde(rename = "upVotesCount")]
    pub up_votes_count: u64,     // Number of upvotes
    #[serde(rename = "downVotesCount")]
    pub down_votes_count: u64,   // Number of downvotes
    #[serde(rename = "repostsCount")]
    pub reposts_count: u64,      // Number of reposts
    #[serde(rename = "parentPostId")]
    pub parent_post_id: Option<String>, // ID of the post being replied to (null for original posts)
    #[serde(rename = "mentionedPubkeys")]
    pub mentioned_pubkeys: Vec<String>, // Array of pubkeys mentioned in this post/reply
    #[serde(rename = "isUpvoted", skip_serializing_if = "Option::is_none")]
    pub is_upvoted: Option<bool>, // Whether the requesting user has upvoted this post (only for get-post-details)
    #[serde(rename = "isDownvoted", skip_serializing_if = "Option::is_none")]
    pub is_downvoted: Option<bool>, // Whether the requesting user has downvoted this post (only for get-post-details)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostsResponse {
    pub posts: Vec<ServerPost>,
}

// Pagination metadata for paginated endpoints
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationMetadata {
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
    #[serde(rename = "prevCursor")]  
    pub prev_cursor: Option<String>,
}

// Paginated response for watching posts
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedPostsResponse {
    pub posts: Vec<ServerPost>,
    pub pagination: PaginationMetadata,
}

// API Response model for user broadcasts (simplified structure for Users API)
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerUserPost {
    pub id: String,              // 32-byte cryptographic hash (64 hex characters)
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String, // 32-byte public key (64 hex characters)
    #[serde(rename = "postContent")]
    pub post_content: String,    // Base64 encoded content (max 100 chars when decoded)
    pub signature: String,       // Schnorr signature as hex string
    pub timestamp: u64,          // Unix timestamp
    // Note: Users API omits repliesCount, upVotesCount, downVotesCount, repostsCount, parentPostId, mentionedPubkeys
}

impl ServerUserPost {
    pub fn from_k_broadcast_record(record: &KBroadcastRecord) -> Self {
        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content: record.base64_encoded_message.clone(),
            signature: record.sender_signature.clone(),
            timestamp: record.block_time,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsersResponse {
    pub posts: Vec<ServerUserPost>,
}

// Paginated response for users
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedUsersResponse {
    pub posts: Vec<ServerUserPost>,
    pub pagination: PaginationMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostDetailsResponse {
    pub post: ServerPost,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub code: String,
}

impl ServerPost {
    pub fn from_k_post_record_with_replies_count(record: &KPostRecord, replies_count: u64) -> Self {
        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content: record.base64_encoded_message.clone(),
            signature: record.sender_signature.clone(),
            timestamp: record.block_time,
            replies_count,
            up_votes_count: 0,    // Hardcoded default value
            down_votes_count: 0,  // Hardcoded default value
            reposts_count: 0,  // TODO: Implement repost counting
            parent_post_id: None, // Original posts have no parent
            mentioned_pubkeys: record.mentioned_pubkeys.clone(),
            is_upvoted: None,
            is_downvoted: None,
        }
    }

    pub fn from_k_post_record_with_replies_count_and_votes(
        record: &KPostRecord,
        replies_count: u64,
        up_votes_count: u64,
        down_votes_count: u64,
        is_upvoted: bool,
        is_downvoted: bool,
    ) -> Self {
        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content: record.base64_encoded_message.clone(),
            signature: record.sender_signature.clone(),
            timestamp: record.block_time,
            replies_count,
            up_votes_count,
            down_votes_count,
            reposts_count: 0,  // TODO: Implement repost counting
            parent_post_id: None, // Original posts have no parent
            mentioned_pubkeys: record.mentioned_pubkeys.clone(),
            is_upvoted: Some(is_upvoted),
            is_downvoted: Some(is_downvoted),
        }
    }
}

// Database model for K protocol replies with additional metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KReplyRecord {
    pub transaction_id: String,
    pub block_time: u64,
    pub sender_address: String,
    pub receiver_address: String,
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub post_id: String, // ID of the post being replied to
    pub base64_encoded_message: String, // Stored as Base64 encoded string
    pub mentioned_pubkeys: Vec<String>,
    pub created_at: u64, // Timestamp when record was created
}

impl KReplyRecord {
    pub fn new(
        transaction_id: String,
        block_time: u64,
        sender_address: String,
        receiver_address: String,
        k_reply: KReply,
    ) -> Self {
        Self {
            transaction_id,
            block_time,
            sender_address,
            receiver_address,
            sender_pubkey: k_reply.sender_pubkey,
            sender_signature: k_reply.sender_signature,
            post_id: k_reply.post_id,
            base64_encoded_message: k_reply.base64_encoded_message, // Keep as Base64 encoded
            mentioned_pubkeys: k_reply.mentioned_pubkeys,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

// API Response model for replies (same structure as ServerPost)
pub type ServerReply = ServerPost;

#[derive(Debug, Serialize, Deserialize)]
pub struct RepliesResponse {
    pub replies: Vec<ServerReply>,
}

// Paginated response for replies
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedRepliesResponse {
    pub replies: Vec<ServerReply>,
    pub pagination: PaginationMetadata,
}

impl ServerReply {
    pub fn from_k_reply_record_with_replies_count(record: &KReplyRecord, replies_count: u64) -> Self {
        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content: record.base64_encoded_message.clone(),
            signature: record.sender_signature.clone(),
            timestamp: record.block_time,
            replies_count,
            up_votes_count: 0,              // TODO: Hardcoded default value
            down_votes_count: 0,            // TODO: Hardcoded default value
            reposts_count: 0,               // TODO: Hardcoded default value
            parent_post_id: Some(record.post_id.clone()), // Replies always have a parent post while posts don't.
            mentioned_pubkeys: record.mentioned_pubkeys.clone(),
            is_upvoted: None,
            is_downvoted: None,
        }
    }

    pub fn from_k_reply_record_with_replies_count_and_votes(
        record: &KReplyRecord,
        replies_count: u64,
        up_votes_count: u64,
        down_votes_count: u64,
        is_upvoted: bool,
        is_downvoted: bool,
    ) -> Self {
        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content: record.base64_encoded_message.clone(),
            signature: record.sender_signature.clone(),
            timestamp: record.block_time,
            replies_count,
            up_votes_count,
            down_votes_count,
            reposts_count: 0,               // TODO: Hardcoded default value
            parent_post_id: Some(record.post_id.clone()), // Replies always have a parent post while posts don't.
            mentioned_pubkeys: record.mentioned_pubkeys.clone(),
            is_upvoted: Some(is_upvoted),
            is_downvoted: Some(is_downvoted),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KReply {
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub post_id: String,
    pub base64_encoded_message: String,
    pub mentioned_pubkeys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KVote {
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub post_id: String,
    pub vote: String, // "upvote" or "downvote"
}

// Database model for K protocol votes with additional metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KVoteRecord {
    pub transaction_id: String,
    pub block_time: u64,
    pub sender_address: String,
    pub receiver_address: String,
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub post_id: String,
    pub vote: String, // "upvote" or "downvote"
    pub created_at: u64, // Timestamp when record was created
}

impl KVoteRecord {
    pub fn new(
        transaction_id: String,
        block_time: u64,
        sender_address: String,
        receiver_address: String,
        k_vote: KVote,
    ) -> Self {
        Self {
            transaction_id,
            block_time,
            sender_address,
            receiver_address,
            sender_pubkey: k_vote.sender_pubkey,
            sender_signature: k_vote.sender_signature,
            post_id: k_vote.post_id,
            vote: k_vote.vote,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}
