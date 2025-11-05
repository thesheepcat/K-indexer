-- Migration from Schema v7 to v8
-- Converts all k_ prefixed tables to TimescaleDB hypertables with compression

-- ============================================================================
-- Step 0: Drop unique constraints that don't include partitioning column
-- TimescaleDB requires all unique indexes to include the partitioning column (block_time)
-- We drop these constraints and rely on application-level deduplication
-- ============================================================================

-- Drop UNIQUE constraints on transaction_id (inline column constraint)
ALTER TABLE k_votes ALTER COLUMN transaction_id DROP NOT NULL;
ALTER TABLE k_votes DROP CONSTRAINT IF EXISTS k_votes_transaction_id_key;
ALTER TABLE k_votes ALTER COLUMN transaction_id SET NOT NULL;

ALTER TABLE k_mentions ALTER COLUMN content_id DROP NOT NULL;
ALTER TABLE k_mentions DROP CONSTRAINT IF EXISTS k_mentions_content_id_key;
-- Note: content_id doesn't have NOT NULL constraint, just drop if exists

ALTER TABLE k_contents ALTER COLUMN transaction_id DROP NOT NULL;
ALTER TABLE k_contents DROP CONSTRAINT IF EXISTS k_contents_transaction_id_key;
ALTER TABLE k_contents ALTER COLUMN transaction_id SET NOT NULL;

ALTER TABLE k_broadcasts ALTER COLUMN transaction_id DROP NOT NULL;
ALTER TABLE k_broadcasts DROP CONSTRAINT IF EXISTS k_broadcasts_transaction_id_key;
ALTER TABLE k_broadcasts ALTER COLUMN transaction_id SET NOT NULL;

ALTER TABLE k_follows ALTER COLUMN transaction_id DROP NOT NULL;
ALTER TABLE k_follows DROP CONSTRAINT IF EXISTS k_follows_transaction_id_key;
ALTER TABLE k_follows ALTER COLUMN transaction_id SET NOT NULL;

ALTER TABLE k_blocks ALTER COLUMN transaction_id DROP NOT NULL;
ALTER TABLE k_blocks DROP CONSTRAINT IF EXISTS k_blocks_transaction_id_key;
ALTER TABLE k_blocks ALTER COLUMN transaction_id SET NOT NULL;

-- Drop unique indexes
DROP INDEX IF EXISTS idx_k_votes_sender_signature_unique;
DROP INDEX IF EXISTS idx_k_blocks_sender_signature_unique;
DROP INDEX IF EXISTS idx_k_blocks_sender_blocked_user_unique;
DROP INDEX IF EXISTS idx_k_follows_sender_signature_unique;
DROP INDEX IF EXISTS idx_k_follows_sender_followed_user_unique;
DROP INDEX IF EXISTS idx_k_contents_transaction_id;
DROP INDEX IF EXISTS idx_k_contents_sender_signature_unique;

-- ============================================================================
-- Step 1: Convert k_votes to hypertable
-- ============================================================================

SELECT create_hypertable('k_votes', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_votes SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,post_id',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_votes',
    compress_after => '30 days'::interval);

-- ============================================================================
-- Step 2: Convert k_mentions to hypertable
-- ============================================================================

SELECT create_hypertable('k_mentions', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_mentions SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'mentioned_pubkey,content_type',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_mentions',
    compress_after => '30 days'::interval);

-- ============================================================================
-- Step 3: Convert k_contents to hypertable
-- ============================================================================

SELECT create_hypertable('k_contents', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_contents SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,content_type',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_contents',
    compress_after => '30 days'::interval);

-- ============================================================================
-- Step 4: Convert k_broadcasts to hypertable
-- ============================================================================

SELECT create_hypertable('k_broadcasts', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_broadcasts SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_broadcasts',
    compress_after => '30 days'::interval);

-- ============================================================================
-- Step 5: Convert k_follows to hypertable
-- ============================================================================

SELECT create_hypertable('k_follows', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_follows SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,followed_user_pubkey',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_follows',
    compress_after => '30 days'::interval);

-- ============================================================================
-- Step 6: Convert k_blocks to hypertable
-- ============================================================================

SELECT create_hypertable('k_blocks', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_blocks SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,blocked_user_pubkey',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_blocks',
    compress_after => '30 days'::interval);

-- ============================================================================
-- Step 7: Recreate indexes as non-unique (deduplication handled by application)
-- ============================================================================

-- Recreate transaction_id indexes (non-unique)
CREATE INDEX IF NOT EXISTS idx_k_votes_transaction_id ON k_votes(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_contents_transaction_id ON k_contents(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_transaction_id ON k_broadcasts(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_follows_transaction_id ON k_follows(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_blocks_transaction_id ON k_blocks(transaction_id);

-- Recreate signature indexes (non-unique)
CREATE INDEX IF NOT EXISTS idx_k_votes_sender_signature ON k_votes(sender_signature);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_sender_signature ON k_broadcasts(sender_signature);
CREATE INDEX IF NOT EXISTS idx_k_blocks_sender_signature ON k_blocks(sender_signature);
CREATE INDEX IF NOT EXISTS idx_k_follows_sender_signature ON k_follows(sender_signature);
CREATE INDEX IF NOT EXISTS idx_k_contents_sender_signature ON k_contents(sender_signature);

-- Recreate composite indexes (non-unique)
CREATE INDEX IF NOT EXISTS idx_k_blocks_sender_blocked_user ON k_blocks(sender_pubkey, blocked_user_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_follows_sender_followed_user ON k_follows(sender_pubkey, followed_user_pubkey);

-- ============================================================================
-- Step 8: Update schema version to v8
-- ============================================================================

UPDATE k_vars SET value = '8' WHERE key = 'schema_version';

-- ============================================================================
-- Migration complete!
-- All k_ prefixed tables are now TimescaleDB hypertables with:
-- - 1 day chunk intervals
-- - 30 day compression policies
-- - Optimized segmentby and orderby for each table
-- - Non-unique indexes (deduplication handled by application logic)
-- ============================================================================
