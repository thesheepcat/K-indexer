use anyhow::Result;
use serde_json;
use tracing::{info, error, warn};
use crate::database::{DbPool, Transaction};
use hex;

// Kaspa message signature verification imports (from main K-indexer)
use kaspa_wallet_core::message::{verify_message, PersonalMessage};
use secp256k1::XOnlyPublicKey;

// K Protocol Data Models (ported from main K-indexer)
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KPost {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KVote {
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub post_id: String,
    pub vote: String, // "upvote" or "downvote"
}

// Database record structures for PostgreSQL
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KPostRecord {
    pub transaction_id: String,
    pub block_time: i64,
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub base64_encoded_message: String,
    pub mentioned_pubkeys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KReplyRecord {
    pub transaction_id: String,
    pub block_time: i64,
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub post_id: String,
    pub base64_encoded_message: String,
    pub mentioned_pubkeys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KBroadcastRecord {
    pub transaction_id: String,
    pub block_time: i64,
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub base64_encoded_nickname: String,
    pub base64_encoded_profile_image: Option<String>,
    pub base64_encoded_message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KVoteRecord {
    pub transaction_id: String,
    pub block_time: i64,
    pub sender_pubkey: String,
    pub sender_signature: String,
    pub post_id: String,
    pub vote: String,
    pub author_pubkey: String, // Public key of the original post author (for future implementation)
}

pub struct KProtocolProcessor {
    db_pool: DbPool,
}

impl KProtocolProcessor {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    /// Verify a Kaspa message signature using the proper kaspa-wallet-core verification
    /// This uses Kaspa's PersonalMessageSigningHash and Schnorr signature verification
    fn verify_kaspa_signature(&self, message: &str, signature: &str, public_key_hex: &str) -> bool {
        // Create PersonalMessage from the message string
        let personal_message = PersonalMessage(message);
        
        // Parse signature from hex (64 bytes for Schnorr signature)
        let signature_bytes = match hex::decode(signature) {
            Ok(bytes) => {
                if bytes.len() != 64 {
                    error!("Invalid signature length: expected 64 bytes, got {}", bytes.len());
                    return false;
                }
                bytes
            },
            Err(err) => {
                error!("Failed to decode signature hex '{}': {}", signature, err);
                return false;
            }
        };
        
        // Parse public key from hex
        let public_key_bytes = match hex::decode(public_key_hex) {
            Ok(bytes) => {
                if bytes.len() == 33 {
                    // Remove the compression prefix byte for x-only key (Schnorr uses x-only keys)
                    bytes[1..].to_vec()
                } else if bytes.len() == 32 {
                    // Already x-only format
                    bytes
                } else {
                    error!("Invalid public key length: expected 32 or 33 bytes, got {}", bytes.len());
                    return false;
                }
            },
            Err(err) => {
                error!("Failed to decode public key hex '{}': {}", public_key_hex, err);
                return false;
            }
        };
        
        // Create XOnlyPublicKey for verification
        let public_key = match XOnlyPublicKey::from_slice(&public_key_bytes) {
            Ok(key) => key,
            Err(err) => {
                error!("Failed to create XOnlyPublicKey: {}", err);
                return false;
            }
        };
        
        // Verify the message signature using Kaspa's verify_message function
        match verify_message(&personal_message, &signature_bytes, &public_key) {
            Ok(()) => {
                info!("Kaspa message signature verification successful");
                true
            },
            Err(err) => {
                error!("Kaspa message signature verification failed: {}", err);
                false
            }
        }
    }

    /// Parse K protocol payload and extract action type
    pub fn parse_k_protocol_payload(&self, payload: &str) -> Result<KActionType> {
        // Remove the K protocol prefix "k:1:"
        if !payload.starts_with("k:1:") {
            return Err(anyhow::anyhow!("Invalid K protocol prefix"));
        }

        let k_payload = &payload[4..]; // Remove "k:1:" prefix
        
        // Split by colons to get the components
        let parts: Vec<&str> = k_payload.split(':').collect();
        
        if parts.is_empty() {
            return Err(anyhow::anyhow!("Empty K protocol payload after removing prefix"));
        }
        
        let action = parts[0];

        match action {
            "broadcast" => {
                // Expected format: broadcast:sender_pubkey:sender_signature:base64_encoded_nickname:base64_encoded_profile_image:base64_encoded_message
                if parts.len() < 6 {
                    return Err(anyhow::anyhow!("Invalid broadcast format: expected 6 parts, got {}", parts.len()));
                }

                let sender_pubkey = parts[1].to_string();
                let sender_signature = parts[2].to_string();
                let base64_encoded_nickname = parts[3].to_string();
                let base64_encoded_profile_image = if parts[4].is_empty() { None } else { Some(parts[4].to_string()) };
                let base64_encoded_message = parts[5].to_string();

                Ok(KActionType::Broadcast(KBroadcast {
                    sender_pubkey,
                    sender_signature,
                    base64_encoded_nickname,
                    base64_encoded_profile_image,
                    base64_encoded_message,
                }))
            }
            "post" => {
                // Expected format: post:sender_pubkey:sender_signature:base64_message:mentioned_pubkeys_json
                if parts.len() < 4 {
                    return Err(anyhow::anyhow!("Invalid post format: expected at least 4 parts, got {}", parts.len()));
                }

                let sender_pubkey = parts[1].to_string();
                let sender_signature = parts[2].to_string();
                let base64_encoded_message = parts[3].to_string();
                
                // Parse mentioned_pubkeys from JSON if present
                let mentioned_pubkeys: Vec<String> = if parts.len() > 4 {
                    let mentioned_pubkeys_json = parts[4];
                    match serde_json::from_str::<Vec<String>>(mentioned_pubkeys_json) {
                        Ok(pubkeys) => pubkeys,
                        Err(err) => {
                            error!("Failed to parse mentioned_pubkeys JSON '{}': {}", mentioned_pubkeys_json, err);
                            Vec::new() // Default to empty array on parse error
                        }
                    }
                } else {
                    Vec::new() // No mentioned_pubkeys field
                };

                Ok(KActionType::Post(KPost {
                    sender_pubkey,
                    sender_signature,
                    base64_encoded_message,
                    mentioned_pubkeys,
                }))
            }
            "reply" => {
                // Expected format: reply:sender_pubkey:sender_signature:post_id:base64_message:mentioned_pubkeys_json
                if parts.len() < 5 {
                    return Err(anyhow::anyhow!("Invalid reply format: expected at least 5 parts, got {}", parts.len()));
                }

                let sender_pubkey = parts[1].to_string();
                let sender_signature = parts[2].to_string();
                let post_id = parts[3].to_string();
                let base64_encoded_message = parts[4].to_string();
                
                // Parse mentioned_pubkeys from JSON if present
                let mentioned_pubkeys: Vec<String> = if parts.len() > 5 {
                    let mentioned_pubkeys_json = parts[5];
                    match serde_json::from_str::<Vec<String>>(mentioned_pubkeys_json) {
                        Ok(pubkeys) => pubkeys,
                        Err(err) => {
                            error!("Failed to parse mentioned_pubkeys JSON '{}': {}", mentioned_pubkeys_json, err);
                            Vec::new() // Default to empty array on parse error
                        }
                    }
                } else {
                    Vec::new() // No mentioned_pubkeys field
                };

                Ok(KActionType::Reply(KReply {
                    sender_pubkey,
                    sender_signature,
                    post_id,
                    base64_encoded_message,
                    mentioned_pubkeys,
                }))
            }
            "vote" => {
                // Expected format: vote:sender_pubkey:sender_signature:post_id:vote
                if parts.len() < 5 {
                    return Err(anyhow::anyhow!("Invalid vote format: expected 5 parts, got {}", parts.len()));
                }

                let sender_pubkey = parts[1].to_string();
                let sender_signature = parts[2].to_string();
                let post_id = parts[3].to_string();
                let vote = parts[4].to_string();

                // Validate vote value
                if vote != "upvote" && vote != "downvote" {
                    return Err(anyhow::anyhow!("Invalid vote value: expected 'upvote' or 'downvote', got '{}'", vote));
                }

                Ok(KActionType::Vote(KVote {
                    sender_pubkey,
                    sender_signature,
                    post_id,
                    vote,
                }))
            }
            _ => Ok(KActionType::Unknown(action.to_string())),
        }
    }

    /// Check if transaction already exists in any K protocol table
    pub async fn transaction_exists(&self, transaction_id: &str) -> Result<bool> {
        // Check in k_posts table
        let posts_result = sqlx::query(
            "SELECT transaction_id FROM k_posts WHERE transaction_id = $1"
        )
        .bind(transaction_id)
        .fetch_optional(&self.db_pool)
        .await?;

        if posts_result.is_some() {
            return Ok(true);
        }

        // Check in k_replies table
        let replies_result = sqlx::query(
            "SELECT transaction_id FROM k_replies WHERE transaction_id = $1"
        )
        .bind(transaction_id)
        .fetch_optional(&self.db_pool)
        .await?;

        if replies_result.is_some() {
            return Ok(true);
        }

        // Check in k_broadcasts table
        let broadcasts_result = sqlx::query(
            "SELECT transaction_id FROM k_broadcasts WHERE transaction_id = $1"
        )
        .bind(transaction_id)
        .fetch_optional(&self.db_pool)
        .await?;

        if broadcasts_result.is_some() {
            return Ok(true);
        }

        // Check in k_votes table
        let votes_result = sqlx::query(
            "SELECT transaction_id FROM k_votes WHERE transaction_id = $1"
        )
        .bind(transaction_id)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(votes_result.is_some())
    }

    /// Process K protocol transaction
    pub async fn process_k_transaction(&self, transaction: &Transaction) -> Result<()> {
        let transaction_id = &transaction.transaction_id;

        // Get payload as hex string
        let payload_hex = match &transaction.payload {
            Some(hex_payload) => hex_payload,
            None => {
                warn!("Transaction {} has no payload", transaction_id);
                return Ok(());
            }
        };

        // Convert hex payload to UTF-8 string
        let payload_bytes = match hex::decode(payload_hex) {
            Ok(bytes) => bytes,
            Err(err) => {
                error!("Failed to decode hex payload for transaction {}: {}", transaction_id, err);
                return Ok(());
            }
        };

        let payload_str = match std::str::from_utf8(&payload_bytes) {
            Ok(payload_str) => payload_str,
            Err(err) => {
                error!("Invalid UTF-8 in transaction payload for ID: {}: {}", transaction_id, err);
                return Ok(());
            }
        };

        // Clean the payload string by removing null bytes and other control characters
        let cleaned_payload = payload_str.chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
            .collect::<String>();

        // Check if this transaction already exists in the database to avoid duplicates
        if self.transaction_exists(transaction_id).await? {
            info!("Transaction {} already exists, skipping", transaction_id);
            return Ok(());
        }

        // Parse K protocol payload
        match self.parse_k_protocol_payload(&cleaned_payload) {
            Ok(action_type) => {
                match action_type {
                    KActionType::Broadcast(k_broadcast) => {
                        self.save_k_broadcast_to_database(transaction, k_broadcast).await?;
                    }
                    KActionType::Post(k_post) => {
                        self.save_k_post_to_database(transaction, k_post).await?;
                    }
                    KActionType::Reply(k_reply) => {
                        self.save_k_reply_to_database(transaction, k_reply).await?;
                    }
                    KActionType::Vote(k_vote) => {
                        self.save_k_vote_to_database(transaction, k_vote).await?;
                    }
                    KActionType::Unknown(action) => {
                        warn!("Unknown K protocol action '{}' in transaction {}", action, transaction_id);
                    }
                }
            }
            Err(err) => {
                error!("Failed to parse K protocol payload for transaction {}: {}", transaction_id, err);
            }
        }

        Ok(())
    }

    /// Save K post to database
    pub async fn save_k_post_to_database(&self, transaction: &Transaction, k_post: KPost) -> Result<()> {
        let transaction_id = &transaction.transaction_id;

        // Construct the message to verify - it's the base64 message + mentioned_pubkeys JSON
        let mentioned_pubkeys_json_str = serde_json::to_string(&k_post.mentioned_pubkeys).unwrap_or_else(|_| "[]".to_string());
        let message_to_verify = format!("{}:{}", k_post.base64_encoded_message, mentioned_pubkeys_json_str);

        // Verify the signature
        if !self.verify_kaspa_signature(&message_to_verify, &k_post.sender_signature, &k_post.sender_pubkey) {
            error!("Invalid signature for post {}, skipping", transaction_id);
            return Ok(()); // Skip posts with invalid signatures
        }

        // Extract block time (use transaction block time or current time as fallback)
        let block_time = transaction.block_time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        // Convert mentioned_pubkeys to JSONB
        let mentioned_pubkeys_json = serde_json::to_value(&k_post.mentioned_pubkeys)?;

        // Insert into k_posts table
        sqlx::query(
            r#"
            INSERT INTO k_posts (
                transaction_id, block_time, sender_pubkey, sender_signature, 
                base64_encoded_message, mentioned_pubkeys
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(transaction_id)
        .bind(block_time)
        .bind(k_post.sender_pubkey)
        .bind(k_post.sender_signature)
        .bind(k_post.base64_encoded_message)
        .bind(mentioned_pubkeys_json)
        .execute(&self.db_pool)
        .await?;

        info!("Saved K post: {}", transaction_id);
        Ok(())
    }

    /// Save K reply to database
    pub async fn save_k_reply_to_database(&self, transaction: &Transaction, k_reply: KReply) -> Result<()> {
        let transaction_id = &transaction.transaction_id;

        // Convert mentioned_pubkeys to JSONB
        let mentioned_pubkeys_json = serde_json::to_value(&k_reply.mentioned_pubkeys)?;

        // Store values we need for logging before they're moved
        let post_id_for_log = k_reply.post_id.clone();
        
        // Insert into k_replies table
        sqlx::query(
            r#"
            INSERT INTO k_replies (
                transaction_id, block_time, sender_pubkey, sender_signature, 
                post_id, base64_encoded_message, mentioned_pubkeys
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(transaction_id)
        .bind(transaction.block_time.unwrap_or(0))
        .bind(k_reply.sender_pubkey)
        .bind(k_reply.sender_signature)
        .bind(k_reply.post_id)
        .bind(k_reply.base64_encoded_message)
        .bind(mentioned_pubkeys_json)
        .execute(&self.db_pool)
        .await?;

        info!("Saved K reply: {} -> {}", transaction_id, post_id_for_log);
        Ok(())
    }

    /// Save K broadcast to database
    pub async fn save_k_broadcast_to_database(&self, transaction: &Transaction, k_broadcast: KBroadcast) -> Result<()> {
        let transaction_id = &transaction.transaction_id;

        // Insert into k_broadcasts table
        sqlx::query(
            r#"
            INSERT INTO k_broadcasts (
                transaction_id, block_time, sender_pubkey, sender_signature, 
                base64_encoded_nickname, base64_encoded_profile_image, base64_encoded_message
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(transaction_id)
        .bind(transaction.block_time.unwrap_or(0))
        .bind(k_broadcast.sender_pubkey)
        .bind(k_broadcast.sender_signature)
        .bind(k_broadcast.base64_encoded_nickname)
        .bind(k_broadcast.base64_encoded_profile_image)
        .bind(k_broadcast.base64_encoded_message)
        .execute(&self.db_pool)
        .await?;

        info!("Saved K broadcast: {}", transaction_id);
        Ok(())
    }

    /// Save K vote to database
    pub async fn save_k_vote_to_database(&self, transaction: &Transaction, k_vote: KVote) -> Result<()> {
        let transaction_id = &transaction.transaction_id;

        // Store values we need for logging before they're moved
        let post_id_for_log = k_vote.post_id.clone();
        let vote_for_log = k_vote.vote.clone();
        
        // Insert into k_votes table
        sqlx::query(
            r#"
            INSERT INTO k_votes (
                transaction_id, block_time, sender_pubkey, sender_signature, 
                post_id, vote, author_pubkey
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(transaction_id)
        .bind(transaction.block_time.unwrap_or(0))
        .bind(k_vote.sender_pubkey)
        .bind(k_vote.sender_signature)
        .bind(k_vote.post_id)
        .bind(k_vote.vote)
        .bind("") // Empty string for author_pubkey (future implementation)
        .execute(&self.db_pool)
        .await?;

        info!("Saved K vote: {} -> {} ({})", transaction_id, post_id_for_log, vote_for_log);
        Ok(())
    }
}