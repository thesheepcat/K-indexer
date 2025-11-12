-- K-transaction-processor Schema v0
-- Creates all K protocol tables (no indexes, no extensions)

-- Create system variables table first
CREATE TABLE IF NOT EXISTS k_vars (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL
);

-- Insert initial schema version
INSERT INTO k_vars (key, value) VALUES ('schema_version', '0') ON CONFLICT (key) DO NOTHING;

-- Create k_broadcasts table
CREATE TABLE IF NOT EXISTS k_broadcasts (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    base64_encoded_nickname TEXT NOT NULL DEFAULT '',
    base64_encoded_profile_image TEXT,
    base64_encoded_message TEXT NOT NULL
);

-- Create k_votes table
CREATE TABLE IF NOT EXISTS k_votes (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    post_id BYTEA NOT NULL,
    vote VARCHAR(10) NOT NULL CHECK (vote IN ('upvote', 'downvote'))
);

-- Create k_mentions table
CREATE TABLE IF NOT EXISTS k_mentions (
    id BIGSERIAL PRIMARY KEY,
    content_id BYTEA NOT NULL,
    content_type VARCHAR(10) NOT NULL,
    mentioned_pubkey BYTEA NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA
);

-- Create k_blocks table for blocking/unblocking users
CREATE TABLE IF NOT EXISTS k_blocks (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    blocking_action VARCHAR(10) NOT NULL CHECK (blocking_action IN ('block')),
    blocked_user_pubkey BYTEA NOT NULL
);

-- Create k_follows table for following/unfollowing users
CREATE TABLE IF NOT EXISTS k_follows (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    following_action VARCHAR(10) NOT NULL CHECK (following_action IN ('follow')),
    followed_user_pubkey BYTEA NOT NULL
);

-- Create unified k_contents table (posts, replies, reposts, quotes)
CREATE TABLE IF NOT EXISTS k_contents (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    base64_encoded_message TEXT NOT NULL,
    -- Content type discriminator: 'post', 'reply', 'repost', 'quote'
    content_type VARCHAR(10) NOT NULL CHECK (content_type IN ('post', 'reply', 'repost', 'quote')),
    -- Optional reference to parent content (NULL for posts, NOT NULL for replies/reposts/quotes)
    referenced_content_id BYTEA
);
