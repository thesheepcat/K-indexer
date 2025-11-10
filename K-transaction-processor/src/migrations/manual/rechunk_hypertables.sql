-- Manual Migration Script: Rebuild Hypertable Chunks
-- Purpose: Fix oversized chunks (1000 days) by recreating them as 1-day chunks
--
-- IMPORTANT: This script should be run manually during maintenance window
-- Expected downtime: 5-10 minutes (depending on data size)
--
-- How to run:
-- PGPASSWORD='your_password' psql -h host -p port -U user -d k-db -f rechunk_hypertables.sql

\timing on
\set ON_ERROR_STOP on

BEGIN;

-- ============================================================================
-- Step 1: Create backup tables with all data
-- ============================================================================

\echo 'Creating backup tables...'

CREATE TABLE IF NOT EXISTS k_votes_backup (LIKE k_votes INCLUDING ALL);
CREATE TABLE IF NOT EXISTS k_mentions_backup (LIKE k_mentions INCLUDING ALL);
CREATE TABLE IF NOT EXISTS k_contents_backup (LIKE k_contents INCLUDING ALL);
CREATE TABLE IF NOT EXISTS k_broadcasts_backup (LIKE k_broadcasts INCLUDING ALL);
CREATE TABLE IF NOT EXISTS k_follows_backup (LIKE k_follows INCLUDING ALL);
CREATE TABLE IF NOT EXISTS k_blocks_backup (LIKE k_blocks INCLUDING ALL);

\echo 'Copying data to backup tables...'

INSERT INTO k_votes_backup SELECT * FROM k_votes;
INSERT INTO k_mentions_backup SELECT * FROM k_mentions;
INSERT INTO k_contents_backup SELECT * FROM k_contents;
INSERT INTO k_broadcasts_backup SELECT * FROM k_broadcasts;
INSERT INTO k_follows_backup SELECT * FROM k_follows;
INSERT INTO k_blocks_backup SELECT * FROM k_blocks;

-- Verify backup counts match
\echo 'Verifying backup counts...'
SELECT
    'k_votes' as table_name,
    (SELECT COUNT(*) FROM k_votes) as original_count,
    (SELECT COUNT(*) FROM k_votes_backup) as backup_count,
    (SELECT COUNT(*) FROM k_votes) = (SELECT COUNT(*) FROM k_votes_backup) as match
UNION ALL
SELECT
    'k_mentions',
    (SELECT COUNT(*) FROM k_mentions),
    (SELECT COUNT(*) FROM k_mentions_backup),
    (SELECT COUNT(*) FROM k_mentions) = (SELECT COUNT(*) FROM k_mentions_backup)
UNION ALL
SELECT
    'k_contents',
    (SELECT COUNT(*) FROM k_contents),
    (SELECT COUNT(*) FROM k_contents_backup),
    (SELECT COUNT(*) FROM k_contents) = (SELECT COUNT(*) FROM k_contents_backup)
UNION ALL
SELECT
    'k_broadcasts',
    (SELECT COUNT(*) FROM k_broadcasts),
    (SELECT COUNT(*) FROM k_broadcasts_backup),
    (SELECT COUNT(*) FROM k_broadcasts) = (SELECT COUNT(*) FROM k_broadcasts_backup)
UNION ALL
SELECT
    'k_follows',
    (SELECT COUNT(*) FROM k_follows),
    (SELECT COUNT(*) FROM k_follows_backup),
    (SELECT COUNT(*) FROM k_follows) = (SELECT COUNT(*) FROM k_follows_backup)
UNION ALL
SELECT
    'k_blocks',
    (SELECT COUNT(*) FROM k_blocks),
    (SELECT COUNT(*) FROM k_blocks_backup),
    (SELECT COUNT(*) FROM k_blocks) = (SELECT COUNT(*) FROM k_blocks_backup);

-- ============================================================================
-- Step 2: Truncate hypertables (this drops all chunks)
-- ============================================================================

\echo 'Truncating hypertables (this will drop all oversized chunks)...'

TRUNCATE k_votes;
TRUNCATE k_mentions;
TRUNCATE k_contents;
TRUNCATE k_broadcasts;
TRUNCATE k_follows;
TRUNCATE k_blocks;

-- ============================================================================
-- Step 3: Re-insert data (TimescaleDB will create new 1-day chunks)
-- ============================================================================

\echo 'Re-inserting data (this will create new 1-day chunks)...'
\echo 'This may take a few minutes...'

INSERT INTO k_votes SELECT * FROM k_votes_backup ORDER BY block_time;
\echo 'k_votes: done'

INSERT INTO k_mentions SELECT * FROM k_mentions_backup ORDER BY block_time;
\echo 'k_mentions: done'

INSERT INTO k_contents SELECT * FROM k_contents_backup ORDER BY block_time;
\echo 'k_contents: done'

INSERT INTO k_broadcasts SELECT * FROM k_broadcasts_backup ORDER BY block_time;
\echo 'k_broadcasts: done'

INSERT INTO k_follows SELECT * FROM k_follows_backup ORDER BY block_time;
\echo 'k_follows: done'

INSERT INTO k_blocks SELECT * FROM k_blocks_backup ORDER BY block_time;
\echo 'k_blocks: done'

-- ============================================================================
-- Step 4: Verify data integrity
-- ============================================================================

\echo 'Verifying data integrity after re-insertion...'

SELECT
    'k_votes' as table_name,
    (SELECT COUNT(*) FROM k_votes) as current_count,
    (SELECT COUNT(*) FROM k_votes_backup) as backup_count,
    (SELECT COUNT(*) FROM k_votes) = (SELECT COUNT(*) FROM k_votes_backup) as match
UNION ALL
SELECT
    'k_mentions',
    (SELECT COUNT(*) FROM k_mentions),
    (SELECT COUNT(*) FROM k_mentions_backup),
    (SELECT COUNT(*) FROM k_mentions) = (SELECT COUNT(*) FROM k_mentions_backup)
UNION ALL
SELECT
    'k_contents',
    (SELECT COUNT(*) FROM k_contents),
    (SELECT COUNT(*) FROM k_contents_backup),
    (SELECT COUNT(*) FROM k_contents) = (SELECT COUNT(*) FROM k_contents_backup)
UNION ALL
SELECT
    'k_broadcasts',
    (SELECT COUNT(*) FROM k_broadcasts),
    (SELECT COUNT(*) FROM k_broadcasts_backup),
    (SELECT COUNT(*) FROM k_broadcasts) = (SELECT COUNT(*) FROM k_broadcasts_backup)
UNION ALL
SELECT
    'k_follows',
    (SELECT COUNT(*) FROM k_follows),
    (SELECT COUNT(*) FROM k_follows_backup),
    (SELECT COUNT(*) FROM k_follows) = (SELECT COUNT(*) FROM k_follows_backup)
UNION ALL
SELECT
    'k_blocks',
    (SELECT COUNT(*) FROM k_blocks),
    (SELECT COUNT(*) FROM k_blocks_backup),
    (SELECT COUNT(*) FROM k_blocks) = (SELECT COUNT(*) FROM k_blocks_backup);

-- ============================================================================
-- Step 5: Show new chunk structure
-- ============================================================================

\echo 'New chunk structure:'

SELECT
    hypertable_name,
    COUNT(*) as total_chunks,
    to_timestamp(MIN(range_start_integer)/1000) as oldest_chunk_start,
    to_timestamp(MAX(range_end_integer)/1000) as newest_chunk_end
FROM timescaledb_information.chunks
WHERE hypertable_schema = 'public'
GROUP BY hypertable_name
ORDER BY hypertable_name;

COMMIT;

\echo ''
\echo '============================================================================'
\echo 'Migration completed successfully!'
\echo ''
\echo 'Next steps:'
\echo '1. Verify the chunk counts and date ranges above'
\echo '2. Monitor compression jobs over the next few days'
\echo '3. After confirming everything works, drop backup tables:'
\echo '   DROP TABLE k_votes_backup, k_mentions_backup, k_contents_backup,'
\echo '             k_broadcasts_backup, k_follows_backup, k_blocks_backup;'
\echo '============================================================================'
