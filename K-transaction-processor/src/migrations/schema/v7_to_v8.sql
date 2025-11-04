-- Migration from Schema v7 to v8
-- Converts all k_ prefixed tables to TimescaleDB hypertables with compression

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
-- Step 7: Update schema version to v8
-- ============================================================================

UPDATE k_vars SET value = '8' WHERE key = 'schema_version';

-- ============================================================================
-- Migration complete!
-- All k_ prefixed tables are now TimescaleDB hypertables with:
-- - 1 day chunk intervals
-- - 30 day compression policies
-- - Optimized segmentby and orderby for each table
-- ============================================================================
