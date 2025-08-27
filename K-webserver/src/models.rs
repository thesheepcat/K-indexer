use serde::{Deserialize, Serialize};

// K Protocol Data Models

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KTransaction {
    pub transaction_id: String,
    pub block_time: u64,
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
    #[serde(default)]
    pub base64_encoded_nickname: String,
    pub base64_encoded_profile_image: Option<String>,
    pub base64_encoded_message: String,
}

// Database model for K protocol broadcasts with additional metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KBroadcastRecord {
    pub id: i64,
    pub transaction_id: String,
    pub block_time: u64,
    pub sender_pubkey: String,
    pub sender_signature: String,
    #[serde(default)]
    pub base64_encoded_nickname: String,
    pub base64_encoded_profile_image: Option<String>,
    pub base64_encoded_message: String,
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
    pub id: i64,
    pub transaction_id: String,
    pub block_time: u64,
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub base64_encoded_message: String,
    pub mentioned_pubkeys: Vec<String>,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KReply {
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub post_id: String,
    pub base64_encoded_message: String,
    pub mentioned_pubkeys: Vec<String>,
}

// Database model for K protocol replies with additional metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KReplyRecord {
    pub id: i64,
    pub transaction_id: String,
    pub block_time: u64,
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
    pub vote: String,
}

// Database model for K protocol votes with additional metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KVoteRecord {
    pub id: i64,
    pub transaction_id: String,
    pub block_time: u64,
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub post_id: String,
    pub vote: String,
}


// API Response models
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerPost {
    pub id: String,
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String,
    #[serde(rename = "postContent")]
    pub post_content: String,
    pub signature: String,
    pub timestamp: u64,
    #[serde(rename = "repliesCount")]
    pub replies_count: u64,
    #[serde(rename = "upVotesCount")]
    pub up_votes_count: u64,
    #[serde(rename = "downVotesCount")]
    pub down_votes_count: u64,
    #[serde(rename = "repostsCount")]
    pub reposts_count: u64,
    #[serde(rename = "parentPostId")]
    pub parent_post_id: Option<String>,
    #[serde(rename = "mentionedPubkeys")]
    pub mentioned_pubkeys: Vec<String>,
    #[serde(rename = "isUpvoted", skip_serializing_if = "Option::is_none")]
    pub is_upvoted: Option<bool>,
    #[serde(rename = "isDownvoted", skip_serializing_if = "Option::is_none")]
    pub is_downvoted: Option<bool>,
    #[serde(rename = "userNickname", skip_serializing_if = "Option::is_none")]
    pub user_nickname: Option<String>,
    #[serde(rename = "userProfileImage", skip_serializing_if = "Option::is_none")]
    pub user_profile_image: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostsResponse {
    pub posts: Vec<ServerPost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationMetadata {
    #[serde(rename = "hasMore")]
    pub has_more: bool,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
    #[serde(rename = "prevCursor")]
    pub prev_cursor: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedPostsResponse {
    pub posts: Vec<ServerPost>,
    pub pagination: PaginationMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerUserPost {
    pub id: String,
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String,
    #[serde(rename = "postContent")]
    pub post_content: String,
    pub signature: String,
    pub timestamp: u64,
    #[serde(rename = "userNickname", skip_serializing_if = "Option::is_none")]
    pub user_nickname: Option<String>,
    #[serde(rename = "userProfileImage", skip_serializing_if = "Option::is_none")]
    pub user_profile_image: Option<String>,
}

impl ServerUserPost {
    pub fn from_k_broadcast_record(record: &KBroadcastRecord) -> Self {
        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content: record.base64_encoded_message.clone(),
            signature: record.sender_signature.clone(),
            timestamp: record.block_time,
            user_nickname: Some(record.base64_encoded_nickname.clone()),
            user_profile_image: record.base64_encoded_profile_image.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsersResponse {
    pub posts: Vec<ServerUserPost>,
}

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
            reposts_count: 0,
            parent_post_id: None,
            mentioned_pubkeys: record.mentioned_pubkeys.clone(),
            is_upvoted: Some(is_upvoted),
            is_downvoted: Some(is_downvoted),
            user_nickname: None,
            user_profile_image: None,
        }
    }
}

pub type ServerReply = ServerPost;

#[derive(Debug, Serialize, Deserialize)]
pub struct RepliesResponse {
    pub replies: Vec<ServerReply>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedRepliesResponse {
    pub replies: Vec<ServerReply>,
    pub pagination: PaginationMetadata,
}

impl ServerReply {
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
            reposts_count: 0,
            parent_post_id: Some(record.post_id.clone()),
            mentioned_pubkeys: record.mentioned_pubkeys.clone(),
            is_upvoted: Some(is_upvoted),
            is_downvoted: Some(is_downvoted),
            user_nickname: None,
            user_profile_image: None,
        }
    }
}