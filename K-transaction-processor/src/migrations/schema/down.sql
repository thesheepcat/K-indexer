-- K-transaction-processor Schema Teardown
-- Drops all tables and objects in reverse dependency order

-- Drop trigger and function (created dynamically in database.rs)
DROP TRIGGER IF EXISTS transaction_notify_trigger ON transactions;
DROP FUNCTION IF EXISTS notify_transaction();

-- Drop K protocol tables (reverse dependency order)
DROP TABLE IF EXISTS k_hashtags CASCADE;
DROP TABLE IF EXISTS k_contents CASCADE;
DROP TABLE IF EXISTS k_follows CASCADE;
DROP TABLE IF EXISTS k_blocks CASCADE;
DROP TABLE IF EXISTS k_mentions CASCADE;
DROP TABLE IF EXISTS k_votes CASCADE;
DROP TABLE IF EXISTS k_broadcasts CASCADE;

-- Drop extensions
DROP EXTENSION IF EXISTS pg_stat_statements;

-- Drop system variables table last
DROP TABLE IF EXISTS k_vars;