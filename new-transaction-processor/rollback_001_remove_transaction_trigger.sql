-- Rollback: Remove transaction notification trigger
-- Use this script to remove the trigger and function if needed

BEGIN;

-- Drop the trigger
DROP TRIGGER IF EXISTS transaction_notify_trigger ON transactions;

-- Drop the function
DROP FUNCTION IF EXISTS notify_transaction();

COMMIT;