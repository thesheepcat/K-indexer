-- Migration: v1_to_v2
-- Description: Add hashtag support with pattern matching capabilities
-- Date: 2026-02-14

-- Drop problematic covering index that causes btree size errors
DROP INDEX IF EXISTS idx_k_contents_feed_covering;

-- Replace with optimized index without base64_encoded_message
-- This avoids btree size limit errors when messages contain many hashtags
CREATE INDEX IF NOT EXISTS idx_k_contents_feed_optimized ON k_contents(block_time DESC, id DESC)
    INCLUDE (transaction_id, sender_pubkey, sender_signature, content_type, referenced_content_id)
    WHERE content_type IN ('post', 'repost', 'quote');

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

-- Update schema version
UPDATE k_vars SET value = '2' WHERE key = 'schema_version';
