-- Migration from schema version 7 to 8
-- Adds back critical indexes on k_mentions table for performance optimization

-- Add index on content_id for fast lookup of mentions per content
CREATE INDEX IF NOT EXISTS idx_k_mentions_content_id ON k_mentions(content_id);

-- Add index on mentioned_pubkey for fast lookup of contents mentioning a user
CREATE INDEX IF NOT EXISTS idx_k_mentions_mentioned_pubkey ON k_mentions(mentioned_pubkey);

-- Update schema version
UPDATE k_vars SET value = '8' WHERE key = 'schema_version';
