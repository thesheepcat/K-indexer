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
    pub content_type: Option<String>,
    // Optional enriched metadata fields for optimized queries
    pub replies_count: Option<u64>,
    pub up_votes_count: Option<u64>,
    pub down_votes_count: Option<u64>,
    pub quotes_count: Option<u64>,
    pub is_upvoted: Option<bool>,
    pub is_downvoted: Option<bool>,
    pub user_nickname: Option<String>,
    pub user_profile_image: Option<String>,
    // Quote fields - only populated if this is a quote
    pub referenced_content_id: Option<String>,
    pub referenced_message: Option<String>,
    pub referenced_sender_pubkey: Option<String>,
    pub referenced_nickname: Option<String>,
    pub referenced_profile_image: Option<String>,
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
    pub content_type: Option<String>,
    // Optional enriched metadata fields for optimized queries
    pub replies_count: Option<u64>,
    pub up_votes_count: Option<u64>,
    pub down_votes_count: Option<u64>,
    pub quotes_count: Option<u64>,
    pub is_upvoted: Option<bool>,
    pub is_downvoted: Option<bool>,
    pub user_nickname: Option<String>,
    pub user_profile_image: Option<String>,
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
    // Optional enriched metadata fields for notifications
    pub mention_block_time: Option<u64>,
    pub voted_content: Option<String>,
    pub user_nickname: Option<String>,
    pub user_profile_image: Option<String>,
}

// Merged content record for unified content retrieval
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ContentRecord {
    Post(KPostRecord),
    Reply(KReplyRecord),
    Vote(KVoteRecord),
}

// Content record with mention metadata for notifications
#[derive(Debug, Clone)]
pub struct NotificationContentRecord {
    pub content: ContentRecord,
    pub mention_id: i64,
    pub mention_block_time: u64,
}

// Referenced content data for quotes (only the original content being quoted)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuoteData {
    #[serde(rename = "referencedContentId")]
    pub referenced_content_id: String,
    #[serde(rename = "referencedMessage")]
    pub referenced_message: String,
    #[serde(rename = "referencedSenderPubkey")]
    pub referenced_sender_pubkey: String,
    #[serde(rename = "referencedNickname", skip_serializing_if = "Option::is_none")]
    pub referenced_nickname: Option<String>,
    #[serde(
        rename = "referencedProfileImage",
        skip_serializing_if = "Option::is_none"
    )]
    pub referenced_profile_image: Option<String>,
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
    #[serde(rename = "quotesCount")]
    pub quotes_count: u64,
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
    #[serde(rename = "blockedUser", skip_serializing_if = "Option::is_none")]
    pub blocked_user: Option<bool>,
    #[serde(rename = "contentType", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(rename = "isQuote")]
    pub is_quote: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<QuoteData>,
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
pub struct PaginatedNotificationsResponse {
    pub notifications: Vec<NotificationPost>,
    pub pagination: PaginationMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrendingHashtag {
    pub hashtag: String,
    #[serde(rename = "usageCount")]
    pub usage_count: u64,
    pub rank: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrendingHashtagsResponse {
    #[serde(rename = "timeWindow")]
    pub time_window: String,
    #[serde(rename = "fromTime")]
    pub from_time: u64,
    #[serde(rename = "toTime")]
    pub to_time: u64,
    pub hashtags: Vec<TrendingHashtag>,
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
    #[serde(rename = "blockedUser", skip_serializing_if = "Option::is_none")]
    pub blocked_user: Option<bool>,
    #[serde(rename = "followedUser", skip_serializing_if = "Option::is_none")]
    pub followed_user: Option<bool>,
    #[serde(rename = "followersCount", skip_serializing_if = "Option::is_none")]
    pub followers_count: Option<i64>,
    #[serde(rename = "followingCount", skip_serializing_if = "Option::is_none")]
    pub following_count: Option<i64>,
    #[serde(rename = "blockedCount", skip_serializing_if = "Option::is_none")]
    pub blocked_count: Option<i64>,
    #[serde(rename = "contentsCount", skip_serializing_if = "Option::is_none")]
    pub contents_count: Option<i64>,
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
            blocked_user: None,
            followed_user: None,
            followers_count: None,
            following_count: None,
            blocked_count: None,
            contents_count: None,
        }
    }

    pub fn from_k_broadcast_record_with_block_status(
        record: &KBroadcastRecord,
        is_blocked: bool,
    ) -> Self {
        // Use base64 encoded "**********" for blocked users, otherwise use original message
        let post_content = if is_blocked {
            // Base64 encoded version of "**********"
            "KioqKioqKioqKg==".to_string()
        } else {
            record.base64_encoded_message.clone()
        };

        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content,
            signature: record.sender_signature.clone(),
            timestamp: record.block_time,
            user_nickname: Some(record.base64_encoded_nickname.clone()),
            user_profile_image: record.base64_encoded_profile_image.clone(),
            blocked_user: Some(is_blocked),
            followed_user: None,
            followers_count: None,
            following_count: None,
            blocked_count: None,
            contents_count: None,
        }
    }

    pub fn from_k_broadcast_record_with_block_and_follow_status(
        record: &KBroadcastRecord,
        is_blocked: bool,
        is_followed: bool,
    ) -> Self {
        // Use base64 encoded "**********" for blocked users, otherwise use original message
        let post_content = if is_blocked {
            // Base64 encoded version of "**********"
            "KioqKioqKioqKg==".to_string()
        } else {
            record.base64_encoded_message.clone()
        };

        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content,
            signature: record.sender_signature.clone(),
            timestamp: record.block_time,
            user_nickname: Some(record.base64_encoded_nickname.clone()),
            user_profile_image: record.base64_encoded_profile_image.clone(),
            blocked_user: Some(is_blocked),
            followed_user: Some(is_followed),
            followers_count: None,
            following_count: None,
            blocked_count: None,
            contents_count: None,
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
    // New method to construct from enriched KPostRecord with blocking status
    pub fn from_enriched_k_post_record_with_block_status(
        record: &KPostRecord,
        is_blocked: bool,
    ) -> Self {
        // Use base64 encoded "**********" for blocked users, otherwise use original message
        let post_content = if is_blocked {
            // Base64 encoded version of "**********"
            "KioqKioqKioqKg==".to_string()
        } else {
            record.base64_encoded_message.clone()
        };

        // Build quote data if this is a quote and we have referenced content
        let quote = if let (Some(ref_id), Some(ref_msg), Some(ref_pubkey)) = (
            record.referenced_content_id.as_ref(),
            record.referenced_message.as_ref(),
            record.referenced_sender_pubkey.as_ref(),
        ) {
            Some(QuoteData {
                referenced_content_id: ref_id.clone(),
                referenced_message: ref_msg.clone(),
                referenced_sender_pubkey: ref_pubkey.clone(),
                referenced_nickname: record.referenced_nickname.clone(),
                referenced_profile_image: record.referenced_profile_image.clone(),
            })
        } else {
            None
        };

        let is_quote = quote.is_some();

        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content,
            signature: record.sender_signature.clone(),
            timestamp: record.block_time,
            replies_count: record.replies_count.unwrap_or(0),
            up_votes_count: record.up_votes_count.unwrap_or(0),
            down_votes_count: record.down_votes_count.unwrap_or(0),
            quotes_count: record.quotes_count.unwrap_or(0),
            reposts_count: 0,
            parent_post_id: None,
            mentioned_pubkeys: record.mentioned_pubkeys.clone(),
            is_upvoted: record.is_upvoted,
            is_downvoted: record.is_downvoted,
            user_nickname: record.user_nickname.clone(),
            user_profile_image: record.user_profile_image.clone(),
            blocked_user: Some(is_blocked),
            content_type: record.content_type.clone(),
            is_quote,
            quote,
        }
    }
}

pub type ServerReply = ServerPost;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPost {
    pub id: String,
    pub user_public_key: String,
    pub post_content: String,
    pub timestamp: u64,
    pub user_nickname: Option<String>,
    pub user_profile_image: Option<String>,
    pub content_type: String, // "post", "reply", or "vote" from k_mentions table
    pub cursor: String,       // Compound cursor combining block_time and k_mentions.id
    // Vote-specific fields
    pub vote_type: Option<String>,       // "upvote" or "downvote"
    pub mention_block_time: Option<u64>, // block_time from k_mentions table
    pub content_id: Option<String>,      // The ID of the content being voted on
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_id: Option<String>, // The post ID that the vote refers to
    pub voted_content: Option<String>,   // Content of the post/reply being voted on
}

impl NotificationPost {
    pub fn from_k_post_record_with_mention_cursor(
        record: &KPostRecord,
        mention_id: i64,
        mention_block_time: u64,
    ) -> Self {
        // Determine content type - if referenced_content_id is set, it's a quote
        let content_type = if record.referenced_content_id.is_some() {
            "quote".to_string()
        } else {
            "post".to_string()
        };

        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content: record.base64_encoded_message.clone(),
            timestamp: mention_block_time,
            user_nickname: record.user_nickname.clone(),
            user_profile_image: record.user_profile_image.clone(),
            content_type,
            cursor: format!("{}_{}", mention_block_time, mention_id),
            vote_type: None,
            mention_block_time: None,
            content_id: None,
            post_id: None,
            voted_content: None,
        }
    }

    pub fn from_k_reply_record_with_mention_cursor(
        record: &KReplyRecord,
        mention_id: i64,
        mention_block_time: u64,
    ) -> Self {
        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content: record.base64_encoded_message.clone(),
            timestamp: mention_block_time,
            user_nickname: record.user_nickname.clone(),
            user_profile_image: record.user_profile_image.clone(),
            content_type: "reply".to_string(),
            cursor: format!("{}_{}", mention_block_time, mention_id),
            vote_type: None,
            mention_block_time: None,
            content_id: None,
            post_id: None,
            voted_content: None,
        }
    }

    pub fn from_k_vote_record_with_mention_cursor(
        vote_record: &KVoteRecord,
        mention_id: i64,
        mention_block_time: u64,
        voted_content: String,
        user_nickname: Option<String>,
        user_profile_image: Option<String>,
    ) -> Self {
        Self {
            id: vote_record.transaction_id.clone(),
            user_public_key: vote_record.sender_pubkey.clone(),
            post_content: String::new(), // Votes don't have content
            timestamp: mention_block_time,
            user_nickname,
            user_profile_image,
            content_type: "vote".to_string(),
            cursor: format!("{}_{}", mention_block_time, mention_id),
            vote_type: Some(vote_record.vote.clone()),
            mention_block_time: Some(mention_block_time),
            content_id: Some(vote_record.post_id.clone()),
            post_id: None,
            voted_content: Some(voted_content),
        }
    }
}

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
    // New method to construct from enriched KReplyRecord with blocking status
    pub fn from_enriched_k_reply_record_with_block_status(
        record: &KReplyRecord,
        is_blocked: bool,
    ) -> Self {
        // Use base64 encoded "**********" for blocked users, otherwise use original message
        let post_content = if is_blocked {
            // Base64 encoded version of "**********"
            "KioqKioqKioqKg==".to_string()
        } else {
            record.base64_encoded_message.clone()
        };

        Self {
            id: record.transaction_id.clone(),
            user_public_key: record.sender_pubkey.clone(),
            post_content,
            signature: record.sender_signature.clone(),
            timestamp: record.block_time,
            replies_count: record.replies_count.unwrap_or(0),
            up_votes_count: record.up_votes_count.unwrap_or(0),
            down_votes_count: record.down_votes_count.unwrap_or(0),
            quotes_count: record.quotes_count.unwrap_or(0),
            reposts_count: 0,
            parent_post_id: Some(record.post_id.clone()),
            mentioned_pubkeys: record.mentioned_pubkeys.clone(),
            is_upvoted: record.is_upvoted,
            is_downvoted: record.is_downvoted,
            user_nickname: record.user_nickname.clone(),
            user_profile_image: record.user_profile_image.clone(),
            blocked_user: Some(is_blocked),
            content_type: record.content_type.clone(),
            is_quote: false,
            quote: None,
        }
    }
}
