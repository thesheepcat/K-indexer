-- Migration 002: Create K Protocol Tables
-- This migration creates PostgreSQL tables equivalent to the NoSQL collections used in the main K-indexer

-- Create k_posts table for K protocol posts
CREATE TABLE IF NOT EXISTS k_posts (
    transaction_id BYTEA PRIMARY KEY,       -- Binary transaction ID (32 bytes)
    block_time BIGINT NOT NULL,             -- Unix timestamp
    sender_pubkey BYTEA NOT NULL,           -- Binary public key (32 or 33 bytes)
    sender_signature BYTEA NOT NULL,        -- Binary signature (64 bytes)
    base64_encoded_message TEXT NOT NULL,   -- Base64 encoded message content
    mentioned_pubkeys JSONB DEFAULT '[]'::jsonb -- Array of mentioned public keys as JSON
);

-- Create k_replies table for K protocol replies
CREATE TABLE IF NOT EXISTS k_replies (
    transaction_id BYTEA PRIMARY KEY,       -- Binary transaction ID (32 bytes)
    block_time BIGINT NOT NULL,             -- Unix timestamp
    sender_pubkey BYTEA NOT NULL,           -- Binary public key (32 or 33 bytes)
    sender_signature BYTEA NOT NULL,        -- Binary signature (64 bytes)
    post_id BYTEA NOT NULL,                 -- Binary transaction ID of the post being replied to
    base64_encoded_message TEXT NOT NULL,   -- Base64 encoded message content
    mentioned_pubkeys JSONB DEFAULT '[]'::jsonb -- Array of mentioned public keys as JSON
);

-- Create k_broadcasts table for K protocol broadcasts (user profile updates)
CREATE TABLE IF NOT EXISTS k_broadcasts (
    transaction_id BYTEA PRIMARY KEY,       -- Binary transaction ID (32 bytes)
    block_time BIGINT NOT NULL,             -- Unix timestamp
    sender_pubkey BYTEA NOT NULL,           -- Binary public key (32 or 33 bytes)
    sender_signature BYTEA NOT NULL,        -- Binary signature (64 bytes)
    base64_encoded_nickname TEXT NOT NULL DEFAULT '', -- Base64 encoded user nickname
    base64_encoded_profile_image TEXT,      -- Base64 encoded profile image (optional)
    base64_encoded_message TEXT NOT NULL    -- Base64 encoded message content
);

-- Create k_votes table for K protocol votes (upvotes/downvotes)
CREATE TABLE IF NOT EXISTS k_votes (
    transaction_id BYTEA PRIMARY KEY,       -- Binary transaction ID (32 bytes)
    block_time BIGINT NOT NULL,             -- Unix timestamp
    sender_pubkey BYTEA NOT NULL,           -- Binary public key (32 or 33 bytes)
    sender_signature BYTEA NOT NULL,        -- Binary signature (64 bytes)
    post_id BYTEA NOT NULL,                 -- Binary transaction ID of the post being voted on
    vote VARCHAR(10) NOT NULL CHECK (vote IN ('upvote', 'downvote')), -- Vote type constraint
    author_pubkey BYTEA DEFAULT decode('', 'hex') -- Binary public key of the original post author
);

-- Create indexes for better query performance

-- Indexes on k_posts
CREATE INDEX IF NOT EXISTS idx_k_posts_sender_pubkey ON k_posts(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_posts_block_time ON k_posts(block_time);

-- Indexes on k_replies  
CREATE INDEX IF NOT EXISTS idx_k_replies_sender_pubkey ON k_replies(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_replies_post_id ON k_replies(post_id);
CREATE INDEX IF NOT EXISTS idx_k_replies_block_time ON k_replies(block_time);

-- Indexes on k_broadcasts
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_sender_pubkey ON k_broadcasts(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_block_time ON k_broadcasts(block_time);

-- Indexes on k_votes
CREATE INDEX IF NOT EXISTS idx_k_votes_sender_pubkey ON k_votes(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id ON k_votes(post_id);
CREATE INDEX IF NOT EXISTS idx_k_votes_vote ON k_votes(vote);
CREATE INDEX IF NOT EXISTS idx_k_votes_block_time ON k_votes(block_time);

-- Composite indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id_sender ON k_votes(post_id, sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_replies_post_id_block_time ON k_replies(post_id, block_time DESC);

COMMENT ON TABLE k_posts IS 'K protocol posts data parsed from Kaspa blockchain transactions';
COMMENT ON TABLE k_replies IS 'K protocol replies data parsed from Kaspa blockchain transactions';
COMMENT ON TABLE k_broadcasts IS 'K protocol broadcasts (user profile updates) parsed from Kaspa blockchain transactions';
COMMENT ON TABLE k_votes IS 'K protocol votes (upvotes/downvotes) parsed from Kaspa blockchain transactions';