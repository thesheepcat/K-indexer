-- K-transaction-processor Schema v1
-- Complete schema for fresh installation

-- Create system variables table first
CREATE TABLE IF NOT EXISTS k_vars (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL
);

-- Insert initial schema version (v1 = complete schema with indexes)
INSERT INTO k_vars (key, value) VALUES ('schema_version', '1') ON CONFLICT (key) DO NOTHING;

-- Create K protocol tables
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

-- Create transaction notification function
CREATE OR REPLACE FUNCTION notify_transaction() RETURNS TRIGGER AS '
BEGIN
    IF substr(encode(NEW.payload, ''hex''), 1, 8) = ''6b3a313a'' THEN
        PERFORM pg_notify(''transaction_channel'', encode(NEW.transaction_id, ''hex''));
    END IF;
    RETURN NEW;
END;
' LANGUAGE plpgsql;

-- Create transaction notification trigger (transactions table existence is verified before this runs)
CREATE TRIGGER transaction_notify_trigger AFTER INSERT ON transactions FOR EACH ROW EXECUTE FUNCTION notify_transaction();