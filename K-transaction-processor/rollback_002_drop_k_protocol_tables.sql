-- Rollback 002: Drop K Protocol Tables
-- This rollback script removes all K protocol tables created by migration_002

-- Drop indexes first (they will be dropped automatically with tables, but explicit for clarity)
DROP INDEX IF EXISTS idx_k_votes_post_id_created;
DROP INDEX IF EXISTS idx_k_replies_post_id_created;
DROP INDEX IF EXISTS idx_k_votes_post_id_sender;

DROP INDEX IF EXISTS idx_k_votes_created_at;
DROP INDEX IF EXISTS idx_k_votes_block_time;
DROP INDEX IF EXISTS idx_k_votes_vote;
DROP INDEX IF EXISTS idx_k_votes_post_id;
DROP INDEX IF EXISTS idx_k_votes_sender_pubkey;

DROP INDEX IF EXISTS idx_k_broadcasts_created_at;
DROP INDEX IF EXISTS idx_k_broadcasts_block_time;
DROP INDEX IF EXISTS idx_k_broadcasts_sender_pubkey;

DROP INDEX IF EXISTS idx_k_replies_created_at;
DROP INDEX IF EXISTS idx_k_replies_block_time;
DROP INDEX IF EXISTS idx_k_replies_post_id;
DROP INDEX IF EXISTS idx_k_replies_sender_pubkey;

DROP INDEX IF EXISTS idx_k_posts_created_at;
DROP INDEX IF EXISTS idx_k_posts_block_time;
DROP INDEX IF EXISTS idx_k_posts_sender_pubkey;

-- Drop tables
DROP TABLE IF EXISTS k_votes;
DROP TABLE IF EXISTS k_broadcasts;
DROP TABLE IF EXISTS k_replies;
DROP TABLE IF EXISTS k_posts;