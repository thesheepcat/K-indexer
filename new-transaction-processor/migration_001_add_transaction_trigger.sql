-- Migration: Add transaction notification trigger
-- This trigger will send notifications on the 'transaction_channel' whenever
-- a new transaction is inserted with a payload starting with '6b3a313a' (in hex format)
-- Note: transaction_id and payload are stored as bytea (binary data) in hex format

BEGIN;

-- Create the notification function
CREATE OR REPLACE FUNCTION notify_transaction()
RETURNS TRIGGER AS $$
BEGIN
    -- Check if the payload (bytea) starts with '6b3a313a' when converted to hex
    -- encode(NEW.payload, 'hex') converts bytea to hex string
    IF substr(encode(NEW.payload, 'hex'), 1, 8) = '6b3a313a' THEN
        -- Send notification with transaction_id as hex string
        PERFORM pg_notify('transaction_channel', encode(NEW.transaction_id, 'hex'));
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create the trigger that fires after each INSERT on transactions table
CREATE TRIGGER transaction_notify_trigger
    AFTER INSERT ON transactions
    FOR EACH ROW
    EXECUTE FUNCTION notify_transaction();

COMMIT;