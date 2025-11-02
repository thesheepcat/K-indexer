-- Migration from Schema v5 to v6
-- Adds TimescaleDB and pg_cron extensions with automatic REINDEX scheduling
-- Prevents index bloat on transactions table caused by hourly DELETE operations

-- ============================================================================
-- Step 1: Enable TimescaleDB extension
-- ============================================================================
-- Makes hypertable features available (not activated on tables yet)

CREATE EXTENSION IF NOT EXISTS timescaledb;

-- ============================================================================
-- Step 2: Enable pg_cron extension for scheduled maintenance
-- ============================================================================
-- Allows automatic database maintenance tasks (REINDEX, VACUUM, etc.)

CREATE EXTENSION IF NOT EXISTS pg_cron;

-- ============================================================================
-- Step 3: Schedule automatic REINDEX for transactions_pkey
-- ============================================================================
-- Background: The transactions table experiences severe index bloat due to
-- hourly DELETE operations (--retention=1h in simply-kaspa-indexer).
-- Without REINDEX, the primary key index grows to 1,222 MB for 200K rows
-- (should be ~6 MB). This job runs every 2 hours to maintain optimal size.

-- Remove existing job if present (for idempotency)
DELETE FROM cron.job WHERE jobname = 'reindex-transactions-pkey';

-- Schedule REINDEX for primary key index every 2 hours at :00
SELECT cron.schedule(
    'reindex-transactions-pkey',
    '0 */2 * * *',
    'REINDEX INDEX CONCURRENTLY transactions_pkey'
);

-- ============================================================================
-- Step 4: Schedule automatic REINDEX for transactions_block_time_idx
-- ============================================================================
-- Offset by 5 minutes to avoid concurrent REINDEX operations

-- Remove existing job if present (for idempotency)
DELETE FROM cron.job WHERE jobname = 'reindex-transactions-block-time';

-- Schedule REINDEX for block_time index every 2 hours at :05
SELECT cron.schedule(
    'reindex-transactions-block-time',
    '5 */2 * * *',
    'REINDEX INDEX CONCURRENTLY transactions_block_time_idx'
);

-- ============================================================================
-- Step 5: Update schema version to v6
-- ============================================================================

UPDATE k_vars SET value = '6' WHERE key = 'schema_version';

-- ============================================================================
-- Migration complete!
-- ============================================================================
--
-- TimescaleDB extension enabled (hypertables available but not activated)
-- pg_cron extension enabled with 2 scheduled jobs:
-- - reindex-transactions-pkey: Every 2 hours at :00 (00:00, 02:00, 04:00, ...)
-- - reindex-transactions-block-time: Every 2 hours at :05 (00:05, 02:05, ...)
--
-- Monitoring commands:
-- - View scheduled jobs: SELECT * FROM cron.job;
-- - View job history: SELECT * FROM cron.job_run_details ORDER BY start_time DESC LIMIT 10;
-- - Check index sizes: SELECT pg_size_pretty(pg_indexes_size('transactions'));
--
-- Recommended: Run immediate REINDEX to fix current bloat:
--   REINDEX INDEX CONCURRENTLY transactions_pkey;
--   REINDEX INDEX CONCURRENTLY transactions_block_time_idx;
-- ============================================================================
