-- K-transaction-processor Schema v1 to v2 Migration
-- Add unique constraints on sender_signature for signature-based deduplication across k_posts, k_replies, and k_votes tables

-- Create unique indexes on sender_signature for signature-based deduplication
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_posts_sender_signature_unique ON k_posts(sender_signature);
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_replies_sender_signature_unique ON k_replies(sender_signature);
CREATE UNIQUE INDEX IF NOT EXISTS idx_k_votes_sender_signature_unique ON k_votes(sender_signature);

-- Update schema version to v2
UPDATE k_vars SET value = '2' WHERE key = 'schema_version';