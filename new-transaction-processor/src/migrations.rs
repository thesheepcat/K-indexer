pub const MIGRATION_001_ADD_TRANSACTION_TRIGGER: &str = r#"BEGIN;
CREATE OR REPLACE FUNCTION notify_transaction() RETURNS TRIGGER AS $$ BEGIN IF substr(encode(NEW.payload, 'hex'), 1, 4) = '6b3a' THEN PERFORM pg_notify('transaction_channel', encode(NEW.transaction_id, 'hex')); END IF; RETURN NEW; END; $$ LANGUAGE plpgsql;
CREATE TRIGGER transaction_notify_trigger AFTER INSERT ON transactions FOR EACH ROW EXECUTE FUNCTION notify_transaction();
COMMIT;"#;

pub const MIGRATION_002_CREATE_K_PROTOCOL_TABLES: &str = r#"CREATE TABLE IF NOT EXISTS k_posts (
    transaction_id VARCHAR(64) PRIMARY KEY,
    block_time BIGINT NOT NULL,
    sender_pubkey VARCHAR(66) NOT NULL,
    sender_signature VARCHAR(128) NOT NULL,
    base64_encoded_message TEXT NOT NULL,
    mentioned_pubkeys JSONB DEFAULT '[]'::jsonb
);

CREATE TABLE IF NOT EXISTS k_replies (
    transaction_id VARCHAR(64) PRIMARY KEY,
    block_time BIGINT NOT NULL,
    sender_pubkey VARCHAR(66) NOT NULL,
    sender_signature VARCHAR(128) NOT NULL,
    post_id VARCHAR(64) NOT NULL,
    base64_encoded_message TEXT NOT NULL,
    mentioned_pubkeys JSONB DEFAULT '[]'::jsonb
);

CREATE TABLE IF NOT EXISTS k_broadcasts (
    transaction_id VARCHAR(64) PRIMARY KEY,
    block_time BIGINT NOT NULL,
    sender_pubkey VARCHAR(66) NOT NULL,
    sender_signature VARCHAR(128) NOT NULL,
    base64_encoded_nickname TEXT NOT NULL DEFAULT '',
    base64_encoded_profile_image TEXT,
    base64_encoded_message TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS k_votes (
    transaction_id VARCHAR(64) PRIMARY KEY,
    block_time BIGINT NOT NULL,
    sender_pubkey VARCHAR(66) NOT NULL,
    sender_signature VARCHAR(128) NOT NULL,
    post_id VARCHAR(64) NOT NULL,
    vote VARCHAR(10) NOT NULL CHECK (vote IN ('upvote', 'downvote')),
    author_pubkey VARCHAR(66) DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_k_posts_sender_pubkey ON k_posts(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_posts_block_time ON k_posts(block_time);
CREATE INDEX IF NOT EXISTS idx_k_replies_sender_pubkey ON k_replies(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_replies_post_id ON k_replies(post_id);
CREATE INDEX IF NOT EXISTS idx_k_replies_block_time ON k_replies(block_time);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_sender_pubkey ON k_broadcasts(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_block_time ON k_broadcasts(block_time);
CREATE INDEX IF NOT EXISTS idx_k_votes_sender_pubkey ON k_votes(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id ON k_votes(post_id);
CREATE INDEX IF NOT EXISTS idx_k_votes_vote ON k_votes(vote);
CREATE INDEX IF NOT EXISTS idx_k_votes_block_time ON k_votes(block_time);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id_sender ON k_votes(post_id, sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_replies_post_id_block_time ON k_replies(post_id, block_time DESC);"#;