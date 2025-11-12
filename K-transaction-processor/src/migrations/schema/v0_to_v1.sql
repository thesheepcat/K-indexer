-- K-transaction-processor Schema v0 to v1 Migration
-- Adds all indexes, constraints, and extensions to complete v1 schema

-- Enable pg_stat_statements extension for query performance monitoring
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

-- ============================================================================
-- k_broadcasts indexes
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_transaction_id ON k_broadcasts(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_sender_pubkey ON k_broadcasts(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_block_time ON k_broadcasts(block_time);

-- ============================================================================
-- k_votes indexes
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_k_votes_transaction_id ON k_votes(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_votes_sender_pubkey ON k_votes(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id ON k_votes(post_id);
CREATE INDEX IF NOT EXISTS idx_k_votes_vote ON k_votes(vote);
CREATE INDEX IF NOT EXISTS idx_k_votes_block_time ON k_votes(block_time);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id_sender ON k_votes(post_id, sender_pubkey);

-- Signature-based deduplication index
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_votes_sender_signature_unique ON k_votes(sender_signature);

-- ============================================================================
-- k_blocks indexes
-- ============================================================================
-- Signature-based deduplication index
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_blocks_sender_signature_unique ON k_blocks(sender_signature);

-- Unique constraint: one block record per sender-blocked pair (prevents duplicates)
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_blocks_sender_blocked_user_unique ON k_blocks(sender_pubkey, blocked_user_pubkey);

-- Indexes for efficient blocking queries
CREATE INDEX IF NOT EXISTS idx_k_blocks_sender_pubkey ON k_blocks(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_blocks_blocked_user_pubkey ON k_blocks(blocked_user_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_blocks_block_time ON k_blocks(block_time);

-- ============================================================================
-- k_mentions indexes
-- ============================================================================
-- Comprehensive index that efficiently serves both get-notifications and get-mentions queries
-- This index supports:
-- 1. get-notifications: WHERE mentioned_pubkey = ? AND sender_pubkey NOT IN (blocked_users) ORDER BY block_time DESC, id DESC
-- 2. get-mentions: WHERE mentioned_pubkey = ? AND content_type = ? AND content_id = ?
CREATE INDEX IF NOT EXISTS idx_k_mentions_comprehensive ON k_mentions(mentioned_pubkey, sender_pubkey, content_type, content_id, block_time DESC, id DESC);

-- v8: Critical indexes for performance optimization
-- idx_k_mentions_content_id: Used for feed queries to fetch mentions array per content (20x per feed query)
CREATE INDEX IF NOT EXISTS idx_k_mentions_content_id ON k_mentions(content_id);

-- idx_k_mentions_mentioned_pubkey: Used for get-mentions endpoint to find all contents mentioning a user
CREATE INDEX IF NOT EXISTS idx_k_mentions_mentioned_pubkey ON k_mentions(mentioned_pubkey);

-- ============================================================================
-- k_follows indexes
-- ============================================================================
-- Signature-based deduplication index
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_follows_sender_signature_unique ON k_follows(sender_signature);

-- Unique constraint: one follow record per sender-followed pair (prevents duplicates)
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_follows_sender_followed_user_unique ON k_follows(sender_pubkey, followed_user_pubkey);

-- Index for queries: "who is following user X?"
CREATE INDEX IF NOT EXISTS idx_k_follows_followed_user_pubkey ON k_follows(followed_user_pubkey, block_time DESC);

-- Index for queries: "who does user X follow?"
CREATE INDEX IF NOT EXISTS idx_k_follows_sender_pubkey ON k_follows(sender_pubkey, block_time DESC);

-- Index for time-based queries
CREATE INDEX IF NOT EXISTS idx_k_follows_block_time ON k_follows(block_time DESC);

-- ============================================================================
-- k_contents indexes
-- ============================================================================
-- Primary indexes
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_contents_transaction_id ON k_contents(transaction_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_contents_sender_signature_unique ON k_contents(sender_signature);
CREATE INDEX IF NOT EXISTS idx_k_contents_sender_pubkey ON k_contents(sender_pubkey, block_time DESC);
CREATE INDEX IF NOT EXISTS idx_k_contents_block_time ON k_contents(block_time DESC, id DESC);

-- Partial index for replies: optimized for "get replies for content X"
CREATE INDEX IF NOT EXISTS idx_k_contents_replies ON k_contents(referenced_content_id, block_time DESC)
    WHERE content_type = 'reply';

-- Partial index for reposts: optimized for "get reposts of content X"
CREATE INDEX IF NOT EXISTS idx_k_contents_reposts ON k_contents(referenced_content_id, block_time DESC)
    WHERE content_type = 'repost';

-- Partial index for quotes: optimized for "get quotes of content X"
CREATE INDEX IF NOT EXISTS idx_k_contents_quotes ON k_contents(referenced_content_id, block_time DESC)
    WHERE content_type = 'quote';

-- Covering index for feed queries (posts + reposts + quotes, exclude replies)
-- This index is used for main feed and user timeline queries
CREATE INDEX IF NOT EXISTS idx_k_contents_feed_covering ON k_contents(block_time DESC, id DESC)
    INCLUDE (transaction_id, sender_pubkey, sender_signature, base64_encoded_message, content_type, referenced_content_id)
    WHERE content_type IN ('post', 'repost', 'quote');

-- Content type filtering index
CREATE INDEX IF NOT EXISTS idx_k_contents_content_type ON k_contents(content_type, block_time DESC);

-- User content index (all content types by user)
CREATE INDEX IF NOT EXISTS idx_k_contents_sender_content_type ON k_contents(sender_pubkey, content_type, block_time DESC);

-- Update schema version to v1
UPDATE k_vars SET value = '1' WHERE key = 'schema_version';
