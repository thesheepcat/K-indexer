# K-content-remover

A utility for removing all content created by a specific user from the K-indexer database.

## Overview

K-content-remover is a standalone tool that removes all records associated with a given public key from the K-indexer database. This tool is useful for data cleanup, user account deletion, or removing spam/malicious content.

## Features

- **Complete Removal**: Deletes all content created by the specified user across all K protocol tables
- **Dry-Run Mode**: Preview what would be deleted without making any changes
- **Confirmation Prompt**: Requires explicit confirmation before deletion (can be skipped with `--yes` flag)
- **Atomic Operation**: All deletions happen in a single database transaction (all or nothing)
- **Detailed Reporting**: Shows exactly what was deleted from each table
- **Safe Execution**: Can run while K-transaction-processor and K-webserver are active

## What Gets Removed

K-content-remover deletes records from the following tables where `sender_pubkey` matches the target user:

1. **k_contents** - All posts, replies, reposts, and quotes created by the user
2. **k_votes** - All votes (upvotes/downvotes) created by the user
3. **k_broadcasts** - User's profile information (nickname, profile image, bio)
4. **k_blocks** - Block relationships where the user is the blocker
5. **k_follows** - Follow relationships where the user is the follower
6. **k_mentions** - Mentions where the user is the sender (sender_pubkey matches)

**Important:** This tool deletes records **ONLY** where `sender_pubkey` matches the target user. It does NOT delete mentions that reference the user's content via `content_id` - only mentions where the user is the sender.

## CLI Parameters

### Required

- `-t, --target-user <PUBKEY>`: Public key (hex string) of the user whose content should be removed

### Database Connection (Optional)

- `-H, --db-host <HOST>`: Database host (default: localhost)
- `-P, --db-port <PORT>`: Database port (default: 5432)
- `-d, --db-name <NAME>`: Database name (default: kaspa)
- `-U, --db-user <USER>`: Database username (default: postgres)
- `-p, --db-password <PASSWORD>`: Database password (default: postgres)
- `-m, --db-max-connections <NUM>`: Maximum database connections (default: 2)

### Operation Mode (Optional)

- `--dry-run`: Preview what would be deleted without actually deleting anything
- `-y, --yes`: Skip confirmation prompt and proceed with deletion automatically

## Usage Examples

### Preview Mode (Dry Run)

Check what would be deleted without making any changes:

```bash
cargo run -- --target-user 1234567890abcdef --dry-run
```

### Basic Usage (with confirmation prompt)

```bash
cargo run -- --target-user 1234567890abcdef
```

This will:
1. Show a preview of what will be deleted
2. Ask for confirmation (type "DELETE" to proceed)
3. Delete all content created by the user

### Skip Confirmation Prompt

For automated scripts or when you're absolutely sure:

```bash
cargo run -- --target-user 1234567890abcdef --yes
```

**⚠️ WARNING**: This will delete immediately without asking for confirmation!

### Using DEV Environment (from docker/DEV/.env)

```bash
cd K-content-remover && cargo run -- \
  --target-user 1234567890abcdef \
  --db-host localhost \
  --db-port 5433 \
  --db-name k-db-DEV \
  --db-user username_DEV \
  --db-password password_DEV \
  --db-max-connections 10
```

### Custom Database Connection

```bash
cargo run -- \
  --target-user 1234567890abcdef \
  --db-host localhost \
  --db-port 5432 \
  --db-name kaspa \
  --db-user postgres \
  --db-password mypassword
```

### Production Build

```bash
cargo build --release
./target/release/K-content-remover \
  --target-user 1234567890abcdef \
  --db-host localhost \
  --db-port <DB_PORT> \
  --db-name <DB_NAME> \
  --db-user <DB_USER> \
  --db-password <DB_PASSWORD>
```

## Building

```bash
cd K-content-remover
cargo build --release
```

## Requirements

- Rust 2024 edition
- PostgreSQL database with K-indexer schema
- K-transaction-processor schema must be initialized (tables must exist)

## Architecture

The application is structured as follows:

```
K-content-remover/
├── src/
│   ├── main.rs                # Application entry point and user interaction
│   ├── config.rs              # CLI argument parsing and configuration
│   ├── database.rs            # Database connection pool management
│   └── removal_operation.rs   # Preview and execution of deletion operations
├── Cargo.toml                 # Rust dependencies
└── README.md                  # This file
```

## Logging

The application uses structured logging with the `tracing` crate. Set the `RUST_LOG` environment variable to control log levels:

```bash
RUST_LOG=debug cargo run -- --target-user 1234567890abcdef
```

The application logs:
- Target user public key
- Database connection details
- Preview of records to be deleted (broken down by table)
- Confirmation prompts (when applicable)
- Deletion results (number of records deleted per table)
- Total records deleted

## Safety Features

1. **Preview First**: Always shows what will be deleted before proceeding
2. **Confirmation Required**: Requires typing "DELETE" (all caps) to confirm deletion (unless `--yes` flag is used)
3. **Dry-Run Mode**: Test the operation without making any changes
4. **Atomic Transaction**: All deletions happen in a single transaction - if any part fails, nothing is deleted
5. **No Data Found**: Gracefully exits if no content found for the specified user
6. **Detailed Reporting**: Shows exactly what was deleted for audit purposes

## Error Handling

- Database connection issues trigger automatic retry with 10-second delays
- Invalid public key hex strings are rejected with clear error messages
- Transaction failures result in complete rollback (no partial deletions)
- All errors are logged with detailed context

## Important Notes

- **IRREVERSIBLE**: Once content is deleted, it CANNOT be recovered
- **No Backup**: This tool does not create backups - ensure you have backups if needed
- **Cascade Effect**: Deleting content may affect other users' timelines if they interacted with the deleted content
- **Concurrent Execution**: Safe to run while K-transaction-processor is active (uses proper transaction isolation)
- **k_broadcasts Exception**: Each user can only have one broadcast (profile) record, so deleting it removes their profile entirely

## Example Output

```
Starting K-content-remover v0.1.0
Target user pubkey: 1234567890abcdef
Database connection: localhost:5432/kaspa
Database connection pool created with 2 max connections
========== Analyzing content to remove ==========
Preview results:
  - k_mentions:   12 records
  - k_contents:   23 records
  - k_votes:      8 records
  - k_broadcasts: 1 records
  - k_blocks:     3 records
  - k_follows:    5 records
  Total records to be deleted: 52
========== CONFIRMATION REQUIRED ==========
You are about to DELETE 52 records from the database!
This operation CANNOT be undone!
Target user: 1234567890abcdef

Type 'DELETE' (all caps) to confirm, or anything else to cancel:
> DELETE
========== Executing content removal ==========
✓ Content removal completed successfully:
  - Deleted 12 records from k_mentions
  - Deleted 23 records from k_contents
  - Deleted 8 records from k_votes
  - Deleted 1 records from k_broadcasts
  - Deleted 3 records from k_blocks
  - Deleted 5 records from k_follows
  Total records deleted: 52
========== Content removal completed successfully ==========
Removed 52 total records for user 1234567890abcdef
```

## Comparison with K-database-cleaner

| Feature | K-content-remover | K-database-cleaner |
|---------|------------------|-------------------|
| Execution | One-time | Continuous loop |
| Target | Any specified user | Configured indexer user's preferences |
| Scope | Complete removal by pubkey | Selective cleanup based on follows/blocks |
| Use Case | User deletion, spam removal | Database maintenance, storage optimization |
| Confirmation | Required (unless --yes) | Not required (automated) |
| Dry-Run | Supported | Not supported |
