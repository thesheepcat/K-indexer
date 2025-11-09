-- Migration from Schema v9 to v10
-- Adds integer_now function for TimescaleDB compression to work with integer timestamps

-- Create function that returns current time in milliseconds (matching block_time format)
CREATE OR REPLACE FUNCTION public.integer_now_ms()
RETURNS bigint LANGUAGE SQL STABLE AS
'SELECT (EXTRACT(EPOCH FROM NOW()) * 1000)::bigint';

-- Set the integer_now function for all hypertables
SELECT set_integer_now_func('k_votes', 'integer_now_ms');
SELECT set_integer_now_func('k_mentions', 'integer_now_ms');
SELECT set_integer_now_func('k_contents', 'integer_now_ms');
SELECT set_integer_now_func('k_broadcasts', 'integer_now_ms');
SELECT set_integer_now_func('k_follows', 'integer_now_ms');
SELECT set_integer_now_func('k_blocks', 'integer_now_ms');

-- Update schema version
UPDATE k_vars SET value = '10' WHERE key = 'schema_version';
