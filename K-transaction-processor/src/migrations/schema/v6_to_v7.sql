-- Migration from Schema v6 to v7
-- Removes redundant k_mentions indexes that are covered by idx_k_mentions_comprehensive

-- The comprehensive index (mentioned_pubkey, sender_pubkey, content_type, content_id, block_time DESC, id DESC)
-- covers all query patterns for get-notifications, get-notification-count, and get-mentions.
-- PostgreSQL can use the left-prefix of this index for simpler queries.

-- Drop 4 redundant k_mentions indexes
DROP INDEX IF EXISTS idx_k_mentions_content_id;
DROP INDEX IF EXISTS idx_k_mentions_mentioned_pubkey;
DROP INDEX IF EXISTS idx_k_mentions_content_type;
DROP INDEX IF EXISTS idx_k_mentions_content_type_id;

-- Update schema version
UPDATE k_vars SET value = '7' WHERE key = 'schema_version';
