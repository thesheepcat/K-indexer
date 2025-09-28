-- K-transaction-processor Schema v2 to v3 Migration
-- Add sender_pubkey column to k_mentions table for optimized get-notifications query performance
-- This enables filtering by blocked users at the mention level before expensive JOINs

-- Add sender_pubkey column to k_mentions table
ALTER TABLE k_mentions ADD COLUMN IF NOT EXISTS sender_pubkey BYTEA;

-- Create comprehensive index that efficiently serves both get-notifications and get-mentions queries
-- This index supports:
-- 1. get-notifications: WHERE mentioned_pubkey = ? AND sender_pubkey NOT IN (blocked_users) ORDER BY block_time DESC, id DESC
-- 2. get-mentions: WHERE mentioned_pubkey = ? AND content_type = ? AND content_id = ?
CREATE INDEX IF NOT EXISTS idx_k_mentions_comprehensive ON k_mentions(mentioned_pubkey, sender_pubkey, content_type, content_id, block_time DESC, id DESC);

-- Drop the old optimal index as it's replaced by the new comprehensive one
DROP INDEX IF EXISTS idx_k_mentions_optimal;

-- Update schema version to v3
UPDATE k_vars SET value = '3' WHERE key = 'schema_version';