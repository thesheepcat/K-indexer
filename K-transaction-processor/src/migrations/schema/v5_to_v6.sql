-- Migration from Schema v5 to v6
-- Removes k_posts and k_replies tables (replaced by k_contents table in v4)

-- ============================================================================
-- Step 1: Drop k_posts table and all its indexes
-- ============================================================================

-- Note: Indexes will be automatically dropped when the table is dropped with CASCADE
DROP TABLE IF EXISTS k_posts CASCADE;

-- ============================================================================
-- Step 2: Drop k_replies table and all its indexes
-- ============================================================================

-- Note: Indexes will be automatically dropped when the table is dropped with CASCADE
DROP TABLE IF EXISTS k_replies CASCADE;

-- ============================================================================
-- Step 3: Update schema version to v6
-- ============================================================================

UPDATE k_vars SET value = '6' WHERE key = 'schema_version';

-- ============================================================================
-- Migration complete!
-- ============================================================================
