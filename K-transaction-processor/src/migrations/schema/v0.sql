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