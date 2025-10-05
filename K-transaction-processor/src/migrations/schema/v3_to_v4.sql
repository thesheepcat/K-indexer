-- Migration from Schema v3 to v4
-- Adds unified k_contents table for posts, replies, reposts, and quotes
-- IMPORTANT: This migration does NOT remove k_posts and k_replies tables
-- Both old and new tables will coexist during transition period

-- ============================================================================
-- Step 1: Create unified k_contents table
-- ============================================================================

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

-- ============================================================================
-- Step 2: Create indexes for k_contents table
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

-- ============================================================================
-- Step 3: Update schema version to v4
-- ============================================================================

UPDATE k_vars SET value = '4' WHERE key = 'schema_version';

-- ============================================================================
-- Migration complete!
-- ============================================================================
--
-- Next steps (manual):
-- 1. Deploy application code that supports k_contents table
-- 2. Start dual-writing to both k_posts/k_replies AND k_contents
-- 3. Backfill historical data: INSERT INTO k_contents SELECT FROM k_posts/k_replies
-- 4. Verify data integrity
-- 5. Switch reads to k_contents
-- 6. Stop dual-writing to k_posts/k_replies
-- 7. (Optional) Drop k_posts and k_replies tables in future migration
-- ============================================================================
