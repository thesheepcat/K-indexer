# Database Setup for Transaction Processor

## Overview
The transaction processor requires a PostgreSQL trigger to send notifications when new transactions are inserted. This trigger filters transactions based on payload content and sends notifications to a PostgreSQL channel.

## Prerequisites
- PostgreSQL 10+ installed and running
- Access to the database with CREATE TRIGGER and CREATE FUNCTION privileges
- Existing `transactions` table with the following structure:
  ```sql
  transactions table:
  - transaction_id (bytea, primary key) - stored as binary data in hex format
  - subnetwork_id (integer)
  - hash (bytea) - stored as binary data in hex format  
  - mass (integer)
  - payload (bytea) - stored as binary data in hex format
  - block_time (bigint)
  ```

## Installation Steps

### 1. Deploy the Trigger
Execute the migration script to create the notification trigger:

```bash
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -f migration_001_add_transaction_trigger.sql
```

**Example:**
```bash
psql -h localhost -d kaspa_db -U postgres -f migration_001_add_transaction_trigger.sql
```

### 2. Create K Protocol Tables
Execute the migration script to create the K protocol tables for storing parsed K protocol data:

```bash
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -f migration_002_create_k_protocol_tables.sql
```

**Example:**
```bash
psql -h localhost -d kaspa_db -U postgres -f migration_002_create_k_protocol_tables.sql
```

This will create the following tables:
- `k_posts` - K protocol posts
- `k_replies` - K protocol replies
- `k_broadcasts` - K protocol broadcasts (user profile updates)
- `k_votes` - K protocol votes (upvotes/downvotes)

### 3. Verify Installation

#### 3.1 Verify Transaction Trigger
Check that the trigger was created successfully:

```bash
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -c "
SELECT trigger_name, event_manipulation, event_object_table 
FROM information_schema.triggers 
WHERE trigger_name = 'transaction_notify_trigger';"
```

You should see output similar to:
```
        trigger_name        | event_manipulation | event_object_table 
----------------------------+--------------------+--------------------
 transaction_notify_trigger | INSERT             | transactions
```

Also verify that the function was created:

```bash
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -c "
SELECT proname FROM pg_proc WHERE proname = 'notify_transaction';"
```

You should see:
```
      proname       
--------------------
 notify_transaction
```

#### 3.2 Verify K Protocol Tables
Check that all K protocol tables were created:

```bash
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -c "
SELECT table_name 
FROM information_schema.tables 
WHERE table_schema = 'public' 
AND table_name LIKE 'k_%'
ORDER BY table_name;"
```

You should see:
```
 table_name  
-------------
 k_broadcasts
 k_posts
 k_replies
 k_votes
```

You can also check the table structures:

```bash
# Check k_posts table structure
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -c "\d k_posts"

# Check k_replies table structure  
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -c "\d k_replies"

# Check k_broadcasts table structure
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -c "\d k_broadcasts"

# Check k_votes table structure
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -c "\d k_votes"
```

### 4. Test the Trigger
To test if the trigger works correctly, you can manually insert a test transaction with bytea data:

```sql
-- Insert test transaction with hex data (payload starts with 6b3a313a)
INSERT INTO transactions (transaction_id, subnetwork_id, mass, payload, block_time) 
VALUES (
    decode('1234567890abcdef', 'hex'),  -- transaction_id as bytea
    1,                                 -- subnetwork_id as integer
    100000,                           -- mass as integer
    decode('6b3a313a0123456789abcdef', 'hex'), -- payload as bytea starting with 6b3a313a
    1640995200                        -- block_time as bigint (unix timestamp)
);
```

If you have a client listening on the `transaction_channel`, it should receive a notification with the hex-encoded transaction_id `1234567890abcdef`.

## K Protocol Tables Schema

The K protocol tables store parsed data from Kaspa transactions that contain K protocol payloads. All tables follow the protocol specifications defined in `PROTOCOL_SPECIFICATIONS.md`.

### k_posts
Stores K protocol post messages:
```sql
transaction_id BYTEA PRIMARY KEY       -- Binary transaction ID (32 bytes)
block_time BIGINT NOT NULL             -- Unix timestamp from blockchain
sender_pubkey BYTEA NOT NULL           -- Binary sender public key (32 or 33 bytes)
sender_signature BYTEA NOT NULL        -- Binary message signature (64 bytes)
base64_encoded_message TEXT NOT NULL   -- Base64 encoded message content
mentioned_pubkeys JSONB DEFAULT '[]'   -- Array of mentioned user public keys
```

### k_replies
Stores K protocol reply messages:
```sql
transaction_id BYTEA PRIMARY KEY       -- Binary transaction ID (32 bytes)
block_time BIGINT NOT NULL             -- Unix timestamp from blockchain
sender_pubkey BYTEA NOT NULL           -- Binary sender public key (32 or 33 bytes)
sender_signature BYTEA NOT NULL        -- Binary message signature (64 bytes)
post_id BYTEA NOT NULL                 -- Binary transaction ID of post being replied to
base64_encoded_message TEXT NOT NULL   -- Base64 encoded message content
mentioned_pubkeys JSONB DEFAULT '[]'   -- Array of mentioned user public keys
```

### k_broadcasts
Stores K protocol broadcast messages (user profile updates):
```sql
transaction_id BYTEA PRIMARY KEY       -- Binary transaction ID (32 bytes)
block_time BIGINT NOT NULL             -- Unix timestamp from blockchain
sender_pubkey BYTEA NOT NULL           -- Binary sender public key (32 or 33 bytes)
sender_signature BYTEA NOT NULL        -- Binary message signature (64 bytes)
base64_encoded_nickname TEXT NOT NULL  -- Base64 encoded user nickname
base64_encoded_profile_image TEXT      -- Base64 encoded profile image (optional)
base64_encoded_message TEXT NOT NULL   -- Base64 encoded message content
```

### k_votes
Stores K protocol vote messages (upvotes/downvotes):
```sql
transaction_id BYTEA PRIMARY KEY       -- Binary transaction ID (32 bytes)
block_time BIGINT NOT NULL             -- Unix timestamp from blockchain
sender_pubkey BYTEA NOT NULL           -- Binary sender public key (32 or 33 bytes)
sender_signature BYTEA NOT NULL        -- Binary message signature (64 bytes)
post_id BYTEA NOT NULL                 -- Binary transaction ID of post being voted on
vote VARCHAR(10) NOT NULL              -- 'upvote' or 'downvote'
author_pubkey BYTEA DEFAULT decode('', 'hex') -- Binary original post author public key (future)
```

### Indexes
The migration creates comprehensive indexes for optimal query performance:
- Primary keys on all `transaction_id` fields
- Indexes on `sender_pubkey` for user-based queries
- Indexes on `block_time` for temporal queries
- Indexes on `post_id` for reply/vote relationships
- Composite indexes for common query patterns

## How It Works

1. **Trigger Activation**: The trigger fires after every INSERT operation on the `transactions` table
2. **Payload Filtering**: Only transactions with payloads starting with `'6b3a313a'` (in hex format) trigger notifications
3. **Data Conversion**: The trigger uses `encode(NEW.payload, 'hex')` to convert bytea to hex string for pattern matching
4. **Notification**: A notification is sent to the `transaction_channel` with the hex-encoded `transaction_id` as the payload
5. **Application Processing**: The Rust application receives hex transaction IDs, converts them back to bytea for database queries, and displays all data in hex format

## Configuration

The trigger uses these default settings:
- **Channel Name**: `transaction_channel`
- **Payload Filter**: Transactions starting with `6b3a313a`

To modify these settings, edit the `migration_001_add_transaction_trigger.sql` file before deployment.

## Rollback

If you need to remove the trigger and function:

```bash
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -f rollback_001_remove_transaction_trigger.sql
```

If you need to remove the K protocol tables:

```bash
psql -h $DB_HOST -d $DB_NAME -U $DB_USER -f rollback_002_drop_k_protocol_tables.sql
```

## Security Considerations

- Use a dedicated database user with minimal required permissions
- Consider using SSL/TLS for database connections
- Ensure proper network security between application and database

## Performance Impact

The trigger has minimal performance impact:
- Simple hex encoding and string comparison operations (`encode` + `substr`)
- No additional I/O operations
- Estimated overhead: <1% on INSERT operations
- The `encode()` function is efficient for small bytea payloads

## Data Format Notes

- **Transaction IDs**: Stored as `bytea` in all tables, transmitted as hex strings in notifications
- **Public Keys**: Stored as `bytea` (32 or 33 bytes) for space efficiency, converted from hex strings
- **Signatures**: Stored as `bytea` (64 bytes) for space efficiency, converted from hex strings
- **Payloads**: Stored as `bytea`, checked in hex format (trigger looks for hex pattern `6b3a313a`)
- **Hash Values**: Stored as `bytea`, displayed as hex strings in the application
- **Hex Encoding**: All bytea fields are automatically converted to lowercase hex strings by the application
- **Storage Efficiency**: Using bytea reduces storage by ~50% for binary data compared to hex VARCHAR