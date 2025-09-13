-- K-transaction-processor Schema Teardown
-- Drops all tables and objects in reverse dependency order

-- Drop trigger and function
DROP TRIGGER IF EXISTS transaction_notify_trigger ON transactions;
DROP FUNCTION IF EXISTS notify_transaction();

-- Drop K protocol tables (reverse dependency order)
DROP TABLE IF EXISTS k_mentions;
DROP TABLE IF EXISTS k_votes;
DROP TABLE IF EXISTS k_broadcasts;
DROP TABLE IF EXISTS k_replies;
DROP TABLE IF EXISTS k_posts;

-- Drop system variables table last
DROP TABLE IF EXISTS k_vars;