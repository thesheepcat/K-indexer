-- Migration from Schema v4 to v5
-- Adds k_follows table for following/unfollowing users

-- ============================================================================
-- Step 1: Create k_follows table for following/unfollowing users
-- ============================================================================

CREATE TABLE IF NOT EXISTS k_follows (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    following_action VARCHAR(10) NOT NULL CHECK (following_action IN ('follow')),
    followed_user_pubkey BYTEA NOT NULL
);

-- ============================================================================
-- Step 2: Create indexes for efficient following queries
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
-- Step 3: Update schema version to v5
-- ============================================================================

UPDATE k_vars SET value = '5' WHERE key = 'schema_version';

-- ============================================================================
-- Migration complete!
-- ============================================================================
--
-- k_follows table created with deduplication logic:
-- - follow: inserts/updates record in k_follows
-- - unfollow: deletes record from k_follows (if present)
-- ============================================================================
