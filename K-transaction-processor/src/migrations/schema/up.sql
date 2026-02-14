-- K-transaction-processor Schema v1
-- Complete schema for fresh installation

-- Enable pg_stat_statements extension for query performance monitoring
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

-- Create system variables table first
CREATE TABLE IF NOT EXISTS k_vars (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL
);

-- Insert initial schema version (v2 = complete K protocol schema with hashtags)
INSERT INTO k_vars (key, value) VALUES ('schema_version', '2') ON CONFLICT (key) DO NOTHING;

-- NOTE: k_posts and k_replies tables removed in v6 (replaced by k_contents table in v4)
-- Create K protocol tables

CREATE TABLE IF NOT EXISTS k_broadcasts (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    base64_encoded_nickname TEXT NOT NULL DEFAULT '',
    base64_encoded_profile_image TEXT,
    base64_encoded_message TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS k_votes (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    post_id BYTEA NOT NULL,
    vote VARCHAR(10) NOT NULL CHECK (vote IN ('upvote', 'downvote'))
);

CREATE TABLE IF NOT EXISTS k_mentions (
    id BIGSERIAL PRIMARY KEY,
    content_id BYTEA NOT NULL,
    content_type VARCHAR(10) NOT NULL,
    mentioned_pubkey BYTEA NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA
);

-- Create indexes for K protocol tables
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_transaction_id ON k_broadcasts(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_sender_pubkey ON k_broadcasts(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_block_time ON k_broadcasts(block_time);
CREATE INDEX IF NOT EXISTS idx_k_votes_transaction_id ON k_votes(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_votes_sender_pubkey ON k_votes(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id ON k_votes(post_id);
CREATE INDEX IF NOT EXISTS idx_k_votes_vote ON k_votes(vote);
CREATE INDEX IF NOT EXISTS idx_k_votes_block_time ON k_votes(block_time);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id_sender ON k_votes(post_id, sender_pubkey);

-- Create k_blocks table for blocking/unblocking users
CREATE TABLE IF NOT EXISTS k_blocks (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    blocking_action VARCHAR(10) NOT NULL CHECK (blocking_action IN ('block')),
    blocked_user_pubkey BYTEA NOT NULL
);

-- Signature-based deduplication indexes
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_votes_sender_signature_unique ON k_votes(sender_signature);
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_blocks_sender_signature_unique ON k_blocks(sender_signature);

-- Create indexes for efficient blocking queries
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_blocks_sender_blocked_user_unique ON k_blocks(sender_pubkey, blocked_user_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_blocks_sender_pubkey ON k_blocks(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_blocks_blocked_user_pubkey ON k_blocks(blocked_user_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_blocks_block_time ON k_blocks(block_time);

-- Create comprehensive index that efficiently serves both get-notifications and get-mentions queries
-- This index supports:
-- 1. get-notifications: WHERE mentioned_pubkey = ? AND sender_pubkey NOT IN (blocked_users) ORDER BY block_time DESC, id DESC
-- 2. get-mentions: WHERE mentioned_pubkey = ? AND content_type = ? AND content_id = ?
CREATE INDEX IF NOT EXISTS idx_k_mentions_comprehensive ON k_mentions(mentioned_pubkey, sender_pubkey, content_type, content_id, block_time DESC, id DESC);

-- v1: Critical k_mentions indexes for performance optimization
-- idx_k_mentions_content_id: Used for feed queries to fetch mentions array per content (20x per feed query)
CREATE INDEX IF NOT EXISTS idx_k_mentions_content_id ON k_mentions(content_id);
-- idx_k_mentions_mentioned_pubkey: Used for get-mentions endpoint to find all contents mentioning a user
CREATE INDEX IF NOT EXISTS idx_k_mentions_mentioned_pubkey ON k_mentions(mentioned_pubkey);

-- ============================================================================
-- NEW in v5: k_follows table for following/unfollowing users
-- ============================================================================

-- Create k_follows table for following/unfollowing users
CREATE TABLE IF NOT EXISTS k_follows (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    following_action VARCHAR(10) NOT NULL CHECK (following_action IN ('follow')),
    followed_user_pubkey BYTEA NOT NULL
);

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
-- NEW in v4: Unified k_contents table for posts, replies, reposts, and quotes
-- ============================================================================

-- Create unified contents table (posts, replies, reposts, quotes)
CREATE TABLE IF NOT EXISTS k_contents (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    base64_encoded_message TEXT NOT NULL,
    -- Content type discriminator: 'post', 'reply', 'repost', 'quote'
    content_type VARCHAR(10) NOT NULL CHECK (content_type IN ('post', 'reply', 'repost', 'quote')),
    -- Optional reference to parent content (NULL for posts, NOT NULL for replies/reposts/quotes)
    referenced_content_id BYTEA
);

-- Primary indexes for k_contents
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
-- Note: base64_encoded_message is excluded from INCLUDE to avoid btree size limit errors
-- when messages contain many hashtags (btree v4 max is 2704 bytes)
CREATE INDEX IF NOT EXISTS idx_k_contents_feed_optimized ON k_contents(block_time DESC, id DESC)
    INCLUDE (transaction_id, sender_pubkey, sender_signature, content_type, referenced_content_id)
    WHERE content_type IN ('post', 'repost', 'quote');

-- Content type filtering index
CREATE INDEX IF NOT EXISTS idx_k_contents_content_type ON k_contents(content_type, block_time DESC);

-- User content index (all content types by user)
CREATE INDEX IF NOT EXISTS idx_k_contents_sender_content_type ON k_contents(sender_pubkey, content_type, block_time DESC);

-- ============================================================================
-- NEW in v2: k_hashtags table for hashtag management
-- ============================================================================

-- Create k_hashtags table
CREATE TABLE IF NOT EXISTS k_hashtags (
    id BIGSERIAL PRIMARY KEY,
    sender_pubkey BYTEA NOT NULL,
    content_id BYTEA NOT NULL,
    block_time BIGINT NOT NULL,
    hashtag VARCHAR(30) NOT NULL
);

-- Index 1: Exact match with cursor pagination
CREATE INDEX IF NOT EXISTS idx_k_hashtags_by_hashtag_time
ON k_hashtags (hashtag, block_time DESC, content_id);

-- Index 2: Pattern matching (prefix and contains)
CREATE INDEX IF NOT EXISTS idx_k_hashtags_pattern
ON k_hashtags (hashtag text_pattern_ops, block_time DESC);

-- Index 3: Trending hashtags calculation
CREATE INDEX IF NOT EXISTS idx_k_hashtags_trending
ON k_hashtags (block_time DESC, hashtag);

-- Index 4: Hashtag by sender with cursor pagination
CREATE INDEX IF NOT EXISTS idx_k_hashtags_by_hashtag_sender
ON k_hashtags (hashtag, sender_pubkey, block_time DESC, content_id);

-- Foreign key constraint
ALTER TABLE k_hashtags
ADD CONSTRAINT fk_k_hashtags_content
FOREIGN KEY (content_id)
REFERENCES k_contents(transaction_id)
ON DELETE CASCADE;
