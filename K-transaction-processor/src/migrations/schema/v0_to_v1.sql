-- K-transaction-processor Schema v0 to v1 Migration
-- Adds all indexes to existing tables

-- Create indexes for K protocol tables
CREATE INDEX IF NOT EXISTS idx_k_posts_transaction_id ON k_posts(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_posts_sender_pubkey ON k_posts(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_posts_block_time ON k_posts(block_time);
CREATE INDEX IF NOT EXISTS idx_k_replies_transaction_id ON k_replies(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_replies_sender_pubkey ON k_replies(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_replies_post_id ON k_replies(post_id);
CREATE INDEX IF NOT EXISTS idx_k_replies_block_time ON k_replies(block_time);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_transaction_id ON k_broadcasts(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_sender_pubkey ON k_broadcasts(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_block_time ON k_broadcasts(block_time);
CREATE INDEX IF NOT EXISTS idx_k_votes_transaction_id ON k_votes(transaction_id);
CREATE INDEX IF NOT EXISTS idx_k_votes_sender_pubkey ON k_votes(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id ON k_votes(post_id);
CREATE INDEX IF NOT EXISTS idx_k_votes_vote ON k_votes(vote);
CREATE INDEX IF NOT EXISTS idx_k_votes_block_time ON k_votes(block_time);
CREATE INDEX IF NOT EXISTS idx_k_mentions_content_id ON k_mentions(content_id);
CREATE INDEX IF NOT EXISTS idx_k_mentions_mentioned_pubkey ON k_mentions(mentioned_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_mentions_content_type ON k_mentions(content_type);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id_sender ON k_votes(post_id, sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_replies_post_id_block_time ON k_replies(post_id, block_time DESC);
CREATE INDEX IF NOT EXISTS idx_k_posts_block_time_id_covering ON k_posts(block_time DESC, id DESC) INCLUDE (transaction_id, sender_pubkey, sender_signature, base64_encoded_message);
CREATE INDEX IF NOT EXISTS idx_k_mentions_content_type_id ON k_mentions(content_type, content_id);

-- Update schema version to 1
UPDATE k_vars SET value = '1' WHERE key = 'schema_version';