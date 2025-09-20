-- K-transaction-processor Schema v1 to v2 Migration
-- Add unique constraints on sender_signature for signature-based deduplication across k_posts, k_replies, and k_votes tables
-- Add k_blocks table for user blocking functionality

-- Create unique indexes on sender_signature for signature-based deduplication
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_posts_sender_signature_unique ON k_posts(sender_signature);
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_replies_sender_signature_unique ON k_replies(sender_signature);
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_votes_sender_signature_unique ON k_votes(sender_signature);

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

-- Create unique index on sender_signature for signature-based deduplication
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_blocks_sender_signature_unique ON k_blocks(sender_signature);

-- Create indexes for efficient blocking queries
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_blocks_sender_blocked_user_unique ON k_blocks(sender_pubkey, blocked_user_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_blocks_sender_pubkey ON k_blocks(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_blocks_blocked_user_pubkey ON k_blocks(blocked_user_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_blocks_block_time ON k_blocks(block_time);

-- Update schema version to v2
UPDATE k_vars SET value = '2' WHERE key = 'schema_version';