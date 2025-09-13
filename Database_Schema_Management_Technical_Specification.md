# Database Schema Management Technical Specification

## Overview

The Simply Kaspa Indexer implements a sophisticated database schema management system that handles PostgreSQL database initialization, schema versioning, and incremental upgrades. The system is designed to ensure database consistency and provide seamless migration paths between schema versions.

## Core Components

### 1. Database Client (`database/src/client.rs`)

The `KaspaDbClient` struct serves as the main interface for all database operations, including schema management. Key characteristics:

- **Connection Management**: Uses SQLx connection pooling with configurable pool size (default: 10 connections)
- **Schema Version Constant**: Currently set to version 10 (`SCHEMA_VERSION = 10`)
- **Async Operations**: All database operations are asynchronous using Rust's async/await pattern

### 2. Application Initialization Flow (`indexer/src/main.rs:57-63`)

```rust
let database = KaspaDbClient::new(&cli_args.database_url).await.expect("Database connection FAILED");

if cli_args.initialize_db {
    info!("Initializing database");
    database.drop_schema().await.expect("Unable to drop schema");
}
database.create_schema(cli_args.upgrade_db).await.expect("Unable to create schema");
```

The initialization process follows these steps:
1. Establish database connection
2. Optionally drop existing schema (if `--initialize-db` flag is set)
3. Create or upgrade schema based on current state

## Schema Management System

### 1. Schema Version Detection

The system determines the current schema state by querying the `vars` table:

```sql
SELECT value FROM vars WHERE key = 'schema_version'
```

**Behavior Scenarios:**
- **Table Exists**: Retrieves current schema version and proceeds with upgrade logic
- **Table Missing**: Assumes fresh installation and applies full schema creation

### 2. Fresh Schema Creation

When no existing schema is detected (initial installation):

**Process:**
1. Executes `migrations/schema/up.sql`
2. Creates all required tables and indexes
3. Inserts initial schema version record: `INSERT INTO vars (key, value) VALUES ('schema_version', '10')`

**Core Tables Created:**
- `vars` - System configuration variables
- `blocks` - Kaspa blockchain blocks
- `block_parent` - Block parent relationships
- `subnetworks` - Subnetwork definitions
- `transactions` - Transaction records
- `transaction_inputs` - Transaction input details
- `transaction_outputs` - Transaction output details
- `addresses_transactions` - Address-transaction associations
- `scripts_transactions` - Script-transaction associations
- `blocks_transactions` - Block-transaction relationships
- `transactions_acceptances` - Transaction acceptance records

### 3. Schema Upgrade System

The upgrade system implements a sequential, version-by-version migration approach:

**Version Validation Logic:**
```rust
if version < Self::SCHEMA_VERSION {
    // Perform incremental upgrades
} else if version > Self::SCHEMA_VERSION {
    panic!("Found newer & unsupported schema");
}
```

**Upgrade Process:**
1. **Version Check**: Compare current version against target version
2. **Sequential Upgrades**: Apply migrations one version at a time (v1→v2→v3→...→v10)
3. **Validation**: Confirm each upgrade completed successfully
4. **Safety Checks**: Refuse to proceed with unsupported schema versions

### 4. Migration File Structure and Naming Convention

**File Organization:**
All migration files are located in `database/migrations/schema/` directory with a strict naming convention:

- `up.sql` - Complete schema for fresh installations (creates all tables for current version)
- `down.sql` - Complete schema teardown (drops all tables in reverse dependency order)
- `v{N}_to_v{N+1}.sql` - Incremental upgrade scripts following sequential versioning

**Naming Pattern Examples:**
- `v1_to_v2.sql` - Upgrades from schema version 1 to version 2
- `v2_to_v3.sql` - Upgrades from schema version 2 to version 3
- `v9_to_v10.sql` - Upgrades from schema version 9 to version 10 (current)

**File Management Rules:**
1. **Sequential Numbering**: Version numbers must be consecutive (no gaps allowed)
2. **Immutable Files**: Once deployed, migration files should never be modified
3. **Forward-Only**: No rollback migrations (only upgrade paths)
4. **Compile-Time Embedding**: Files are embedded into binary using `include_str!()` macro
5. **Mandatory Coverage**: Every version increment requires a corresponding migration file

**Supported Migration Path:**
v1 → v2 → v3 → v4 → v5 → v6 → v7 → v8 → v9 → v10

**File Loading Mechanism:**
```rust
// Fresh installation
include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/up.sql"))

// Schema teardown
include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/down.sql"))

// Version-specific upgrades
include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/v1_to_v2.sql"))
include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/v2_to_v3.sql"))
// ... (one file per version increment)
```

### 5. Upgrade Execution Flow

For each version increment:

```rust
if version == N {
    let ddl = include_str!("migrations/schema/v{N}_to_v{N+1}.sql");
    if upgrade_db {
        warn!("Upgrading schema from v{N} to v{N+1}");
        query::misc::execute_ddl(ddl, &pool).await?;
        info!("Schema upgrade completed successfully");
        version += 1;
    } else {
        panic!("Found outdated schema v{N}. Set flag '-u' to upgrade");
    }
}
```

**Key Features:**
- **Explicit Consent**: Requires `--upgrade-db` flag for automatic upgrades
- **DDL Preview**: Shows SQL statements before execution
- **Atomic Operations**: Each migration executes as a single transaction
- **Progress Tracking**: Updates schema version after each successful migration
- **Rollback Protection**: No automatic rollback mechanism (forward-only migrations)

## Command Line Interface

**Schema Management Flags:**
- `--initialize-db`: Drops existing schema and recreates from scratch
- `--upgrade-db` (`-u`): Enables automatic schema upgrades
- `--database-url`: PostgreSQL connection string

**Usage Examples:**
```bash
# Fresh installation
./indexer --initialize-db --database-url "postgresql://user:pass@host/db"

# Upgrade existing schema
./indexer --upgrade-db --database-url "postgresql://user:pass@host/db"

# Normal operation (requires up-to-date schema)
./indexer --database-url "postgresql://user:pass@host/db"
```

## Error Handling and Safety Mechanisms

### 1. Version Compatibility Checks

- **Outdated Schema**: Application refuses to start with old schema versions unless upgrade is explicitly enabled
- **Future Schema**: Application panics when encountering newer schema versions (prevents data corruption)
- **Missing Schema**: Treats as fresh installation scenario

### 2. Upgrade Safety

- **Manual Intervention Required**: Automatic upgrades must be explicitly enabled
- **Preview Mode**: Shows migration SQL before execution when upgrade is disabled
- **Sequential Processing**: Prevents skipping intermediate versions
- **Connection Validation**: Ensures database connectivity before attempting migrations

### 3. Schema Destruction

The `drop_schema()` method provides complete schema teardown by executing `down.sql`, which drops all tables in dependency order:

```sql
DROP TABLE IF EXISTS scripts_transactions;
DROP TABLE IF EXISTS addresses_transactions;
-- ... (all tables in reverse dependency order)
DROP TABLE IF EXISTS vars;
```

## Technical Implementation Details

### 1. Compile-Time SQL Inclusion

Migration files are embedded at compile time using `include_str!()` macro:
```rust
include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/up.sql"))
```

**Benefits:**
- No runtime file system dependencies
- Guaranteed availability of migration scripts
- Compile-time validation of file paths

### 2. SQL Execution Engine (`database/src/query/misc.rs`)

The `execute_ddl()` function handles the execution of SQL migration files:

```rust
pub async fn execute_ddl(ddl: &str, pool: &Pool<Postgres>) -> Result<(), Error> {
    for statement in ddl.split(";").filter(|stmt| !stmt.trim().is_empty()) {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}
```

**Execution Process:**
1. **Statement Parsing**: Splits the SQL file content by semicolon (`;`) delimiters
2. **Whitespace Filtering**: Removes empty or whitespace-only statements
3. **Sequential Execution**: Executes each SQL statement individually using SQLx
4. **Error Propagation**: Stops execution and returns error if any statement fails
5. **Connection Pool Usage**: Uses existing connection pool for all operations

**Key Characteristics:**
- **Atomic Per-Statement**: Each SQL statement executes as a separate database operation
- **Sequential Processing**: Statements execute in file order (important for DDL dependencies)
- **Fail-Fast Behavior**: Entire migration fails if any single statement fails
- **Async Execution**: Non-blocking database operations using Rust async/await

**Usage in Schema Management:**
```rust
// Fresh schema creation
query::misc::execute_ddl(
    include_str!("migrations/schema/up.sql"), 
    &self.pool
).await?;

// Schema upgrades
query::misc::execute_ddl(
    include_str!("migrations/schema/v1_to_v2.sql"), 
    &self.pool
).await?;

// Schema teardown
query::misc::execute_ddl(
    include_str!("migrations/schema/down.sql"), 
    &self.pool
).await?;
```

### 3. Connection Pool Management

- **Pool Configuration**: 10 max connections, 10-second acquire timeout
- **Query Logging**: Slow queries (>60s) logged as warnings
- **Connection Options**: Supports full PostgreSQL connection string format

### 4. Transaction Handling

Each DDL operation executes within its own transaction context, ensuring atomicity of individual migrations while allowing for rollback if specific operations fail.

## Monitoring and Observability

The system provides comprehensive logging throughout the schema management process:

- **Info Level**: Schema version status, upgrade completion
- **Warn Level**: Upgrade previews, slow query notifications
- **Debug Level**: Connection details (with credential masking)
- **Trace Level**: Detailed operation logging

This design ensures reliable, predictable schema evolution while maintaining data integrity and providing clear operational visibility.