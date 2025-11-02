# K-database-cleaner

A utility for maintaining a lighter and cleaner K-indexer database by automatically purging unwanted content based on user-defined settings.

## Overview

K-database-cleaner runs periodic purge operations to remove:
- Content from blocked users
- Old content from non-followed users
- Orphaned replies and votes
- Non-user records from block and follow tables

## Features

- **Automated Purging**: Runs every X seconds/minutes based on user preferences
- **Sequential Operations**: Five separate purge operations executed in order
- **Data Retention**: Configurable retention period for non-followed users' content
- **Detailed Logging**: Reports how many records were deleted in each operation
- **Safe Execution**: Skips remaining operations if an error occurs

## Purge Operations

### Operation 1: Clean Block and Follow Tables
Removes all records where `sender_pubkey` is not the specified user from `k_blocks` and `k_follows` tables.

### Operation 2: Remove Blocked Users' Content
Removes all content (posts, quotes, replies, votes) from blocked users, including related data from the `k_mentions` table.

### Operation 3: Remove Old Content from Non-Followed Users
Removes posts and quotes from non-followed users older than the specified retention period, including related mentions.

### Operation 4: Remove Orphaned Replies
Removes all replies that reference content that no longer exists in the database, including related mentions.

### Operation 5: Remove Orphaned Votes
Removes all votes that reference posts that no longer exist in the database, including related mentions.

## Protected Tables

K-database-cleaner does **not** remove anything from:
- `k_broadcasts`

## CLI Parameters

### Required
- `-u, --user <PUBKEY>`: Public key (hex string) of the user to whom this indexer is dedicated

### Database Connection (Optional)
- `-H, --db-host <HOST>`: Database host (default: localhost)
- `-P, --db-port <PORT>`: Database port (default: 5432)
- `-d, --db-name <NAME>`: Database name (default: kaspa)
- `-U, --db-user <USER>`: Database username (default: postgres)
- `-p, --db-password <PASSWORD>`: Database password (default: postgres)
- `-m, --db-max-connections <NUM>`: Maximum database connections (default: 2)

### Purge Settings (Optional)
- `-t, --purge-interval <SECONDS>`: Interval between purge operations (default: 600 seconds)
- `-r, --data-retention <HOURS>`: Hours to retain data from non-followed users (default: 72)

## Usage Examples

### Basic Usage (with all defaults)
```bash
cargo run -- --user 1234567890abcdef
```

This uses all defaults:
- Database: `localhost:5432/kaspa` with user `postgres` and password `postgres`
- Purge interval: 600 seconds (10 minutes)
- Data retention: 72 hours (3 days)

### Using DEV Environment (from docker/DEV/.env)
```bash
cd K-database-cleaner && cargo run -- \
  --user 1234567890abcdef \
  --data-retention 24 \
  --purge-interval 600 \
  --db-host localhost \
  --db-port 5433 \
  --db-name k-db-DEV \
  --db-user username_DEV \
  --db-password password_DEV \
  --db-max-connections 10
```

**Note:** Replace `1234567890abcdef` with your actual user public key in hex format.

### Custom Database Connection
```bash
cargo run -- \
  --user 1234567890abcdef \
  --db-host localhost \
  --db-port 5432 \
  --db-name kaspa \
  --db-user postgres \
  --db-password mypassword \
  --data-retention 48 \
  --purge-interval 300
```

### Production Build
```bash
cargo build --release
./target/release/K-database-cleaner \
  --user 1234567890abcdef \
  --data-retention 72 \
  --purge-interval 1800 \
  --db-host localhost \
  --db-port <DB_PORT> \
  --db-name <DB_NAME> \
  --db-user <DB_USER> \
  --db-password <DB_PASSWORD>
```

## Building

```bash
cd K-database-cleaner
cargo build --release
```

## Requirements

- Rust 2024 edition
- PostgreSQL database with K-indexer schema
- K-transaction-processor running (for database schema)

## Architecture

The application is structured as follows:

```
K-database-cleaner/
├── src/
│   ├── main.rs                    # Application entry point and purge loop
│   ├── config.rs                  # CLI argument parsing and configuration
│   ├── database.rs                # Database connection pool management
│   └── purge_operations/
│       ├── mod.rs                 # Module exports
│       ├── operation_1.rs         # Clean k_blocks and k_follows
│       ├── operation_2.rs         # Remove blocked users' content
│       ├── operation_3.rs         # Remove old non-followed content
│       ├── operation_4.rs         # Remove orphaned replies
│       └── operation_5.rs         # Remove orphaned votes
├── Cargo.toml                     # Rust dependencies
└── README.md                      # This file
```

## Logging

The application uses structured logging with the `tracing` crate. Set the `RUST_LOG` environment variable to control log levels:

```bash
RUST_LOG=debug cargo run -- --user 1234567890abcdef
```

Each purge cycle logs:
- Start and completion times
- Total duration of the purge cycle
- Number of records deleted per table in each operation
- Any errors or warnings encountered

## Error Handling

- If any purge operation fails, the error is logged and remaining operations in that cycle are skipped
- The application waits for the next purge interval before retrying
- Database connection issues trigger automatic retry with 10-second delays

## Shutdown

Send a `SIGINT` signal (Ctrl+C) to gracefully shut down the application.
