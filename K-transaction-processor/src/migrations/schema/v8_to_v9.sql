-- Migration from Schema v8 to v9
-- Drops redundant idx_k_mentions_comprehensive index

-- ============================================================================
-- Step 1: Drop redundant comprehensive index on k_mentions
-- ============================================================================

DROP INDEX IF EXISTS idx_k_mentions_comprehensive;

-- ============================================================================
-- Step 2: Update schema version to v9
-- ============================================================================

UPDATE k_vars SET value = '9' WHERE key = 'schema_version';

-- ============================================================================
-- Migration complete!
-- ============================================================================
