-- K-transaction-processor Schema v0
-- Basic tables and triggers (no indexes)

-- Create system variables table first
CREATE TABLE IF NOT EXISTS k_vars (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL
);

-- Insert initial schema version
INSERT INTO k_vars (key, value) VALUES ('schema_version', '0') ON CONFLICT (key) DO NOTHING;

-- Create K protocol tables (no indexes)
CREATE TABLE IF NOT EXISTS k_posts (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    base64_encoded_message TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS k_replies (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    post_id BYTEA NOT NULL,
    base64_encoded_message TEXT NOT NULL
);

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

CREATE TABLE IF NOT EXISTS k_votes (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    post_id BYTEA NOT NULL,
    vote VARCHAR(10) NOT NULL CHECK (vote IN ('upvote', 'downvote'))
);

CREATE TABLE IF NOT EXISTS k_mentions (
    id BIGSERIAL PRIMARY KEY,
    content_id BYTEA NOT NULL,
    content_type VARCHAR(10) NOT NULL,
    mentioned_pubkey BYTEA NOT NULL,
    block_time BIGINT NOT NULL
);