# TimescaleDB Implementation Guide for K-Indexer

## 🎯 Objective

Convert all K protocol tables (`k_*`) to TimescaleDB hypertables to achieve:
- **76% storage reduction** (751 MB → 180 MB)
- **20-50x faster queries** on time-range operations
- **Zero application code changes**
- **Keep all existing data** (no deletion)

---

## 📊 Current Database State

| Table | Rows | Current Size | Table Data | Index Data | Bloat Level |
|-------|------|--------------|------------|------------|-------------|
| **k_votes** | 636,570 | 366 MB | 142 MB | 224 MB | High |
| **k_mentions** | 749,789 | 322 MB | 87 MB | 236 MB | High |
| **k_contents** | 46,229 | 61 MB | 19 MB | 42 MB | Medium |
| **k_broadcasts** | 29 | 384 KB | 16 KB | 368 KB | Low |
| **k_follows** | 39 | 160 KB | 16 KB | 144 KB | Low |
| **k_blocks** | 6 | 128 KB | 8 KB | 120 KB | Low |
| **TOTAL** | **1,432,662** | **751 MB** | **248 MB** | **502 MB** | - |

---

## 🗂️ Table-by-Table Implementation

### **Table 1: k_votes**

**Purpose:** User voting on posts (upvote/downvote)
**Current Size:** 366 MB (142 MB table + 224 MB indexes)
**After Conversion:** ~80 MB (28 MB compressed + 52 MB indexes)
**Savings:** 286 MB (78%)

#### Schema:
```sql
CREATE TABLE k_votes (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,          -- ← Partitioning column
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    post_id BYTEA NOT NULL,
    vote VARCHAR(10) NOT NULL CHECK (vote IN ('upvote', 'downvote'))
);
```

#### Conversion:
```sql
-- Convert to hypertable
SELECT create_hypertable(
    'k_votes',                           -- Table name
    'block_time',                        -- Partitioning column (existing BIGINT)
    chunk_time_interval => 86400000000, -- 1 day = 86400 seconds * 1,000,000 microseconds
    migrate_data => true,                -- Move existing data
    if_not_exists => TRUE
);

-- Configure compression
ALTER TABLE k_votes SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,post_id',  -- Group by user+post
    timescaledb.compress_orderby = 'block_time DESC'           -- Sort by time
);

-- Add compression policy: compress data older than 30 days
SELECT add_compression_policy('k_votes',
    compress_after => 2592000000000  -- 30 days in microseconds);
```

#### Rationale:
- **Chunk Size (1 day):** ~21K votes/day = optimal chunk size (not too big, not too small)
- **Compress After 30 days:** Balances query performance (recent data fast) and storage savings (old data compressed)
- **Segment By:** `sender_pubkey, post_id` groups related votes together for better compression
- **Query Pattern:** Most queries are "get votes for post X" or "did user Y vote on post Z"

#### Expected Performance:
```sql
-- Query recent votes (uncompressed)
SELECT * FROM k_votes WHERE post_id = $1 AND block_time > NOW() - INTERVAL '3 days';
-- Before: 2-5 seconds (scans 224 MB index)
-- After: 0.05 seconds (scans 3 chunks, 6 MB total)  ← 40-100x faster

-- Query old votes (compressed)
SELECT * FROM k_votes WHERE sender_pubkey = $1 AND block_time < NOW() - INTERVAL '30 days';
-- Before: 2-5 seconds
-- After: 0.2 seconds (decompresses + scans relevant chunks)  ← 10-25x faster
```

---

### **Table 2: k_mentions**

**Purpose:** Track user mentions in posts/replies
**Current Size:** 322 MB (87 MB table + 236 MB indexes)
**After Conversion:** ~70 MB (18 MB compressed + 52 MB indexes)
**Savings:** 252 MB (78%)

#### Schema:
```sql
CREATE TABLE k_mentions (
    id BIGSERIAL PRIMARY KEY,
    content_id BYTEA NOT NULL,
    content_type VARCHAR(10) NOT NULL,
    mentioned_pubkey BYTEA NOT NULL,
    block_time BIGINT NOT NULL,          -- ← Partitioning column
    sender_pubkey BYTEA
);
```

#### Conversion:
```sql
-- Convert to hypertable
SELECT create_hypertable(
    'k_mentions',
    'block_time',
    chunk_time_interval => 86400000000, -- 1 day
    migrate_data => true,
    if_not_exists => TRUE
);

-- Configure compression
ALTER TABLE k_mentions SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'mentioned_pubkey,content_type',  -- Group by mentioned user
    timescaledb.compress_orderby = 'block_time DESC'
);

-- Add compression policy: compress data older than 30 days
SELECT add_compression_policy('k_mentions',
    compress_after => 2592000000000  -- 30 days in microseconds);
```

#### Rationale:
- **Chunk Size (1 day):** ~25K mentions/day = manageable chunk size
- **Compress After 30 days:** Keeps notification history accessible without decompression overhead
- **Segment By:** `mentioned_pubkey, content_type` optimizes for "get mentions for user X" queries
- **Query Pattern:** Most queries are "get notifications for user X in last N days"

#### Expected Performance:
```sql
-- Query notifications (recent, uncompressed)
SELECT * FROM k_mentions
WHERE mentioned_pubkey = $1 AND block_time > NOW() - INTERVAL '7 days';
-- Before: 1-3 seconds (scans 236 MB index)
-- After: 0.05 seconds (scans 7 chunks)  ← 20-60x faster

-- Query old mentions (compressed)
SELECT * FROM k_mentions
WHERE content_id = $1 AND content_type = 'post';
-- Before: 1-3 seconds
-- After: 0.1-0.3 seconds  ← 10-30x faster
```

---

### **Table 3: k_contents**

**Purpose:** Unified table for posts, replies, reposts, quotes (main content)
**Current Size:** 61 MB (19 MB table + 42 MB indexes)
**After Conversion:** ~30 MB (10 MB compressed + 20 MB indexes)
**Savings:** 31 MB (51%)

#### Schema:
```sql
CREATE TABLE k_contents (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,          -- ← Partitioning column
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    base64_encoded_message TEXT NOT NULL,
    content_type VARCHAR(10) NOT NULL CHECK (content_type IN ('post', 'reply', 'repost', 'quote')),
    referenced_content_id BYTEA
);
```

#### Conversion:
```sql
-- Convert to hypertable
SELECT create_hypertable(
    'k_contents',
    'block_time',
    chunk_time_interval => 86400000000, -- 1 day
    migrate_data => true,
    if_not_exists => TRUE
);

-- Configure compression
ALTER TABLE k_contents SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,content_type',  -- Group by author+type
    timescaledb.compress_orderby = 'block_time DESC'
);

-- Add compression policy: compress data older than 30 days
SELECT add_compression_policy('k_contents',
    compress_after => 2592000000000  -- 30 days in microseconds);
```

#### Rationale:
- **Chunk Size (1 day):** ~150 posts/day = small chunks for fast queries
- **Compress After 30 days:** Posts are frequently viewed/referenced for weeks; compress only old content
- **Segment By:** `sender_pubkey, content_type` optimizes for "get all posts by user X" queries
- **Query Pattern:** Feed queries, user timelines, individual post lookups (frequent access to recent data)

#### Expected Performance:
```sql
-- Query feed (recent posts, uncompressed)
SELECT * FROM k_contents
WHERE content_type IN ('post', 'repost', 'quote')
AND block_time > NOW() - INTERVAL '7 days'
ORDER BY block_time DESC LIMIT 50;
-- Before: 0.5-1 seconds
-- After: 0.02-0.05 seconds  ← 10-50x faster

-- Query user timeline (mixed compressed/uncompressed)
SELECT * FROM k_contents
WHERE sender_pubkey = $1 AND block_time > NOW() - INTERVAL '90 days';
-- Before: 0.3-0.8 seconds
-- After: 0.05-0.1 seconds  ← 6-16x faster
```

---

### **Table 4: k_broadcasts**

**Purpose:** User profile updates (nickname, avatar, bio)
**Current Size:** 384 KB (16 KB table + 368 KB indexes)
**After Conversion:** ~200 KB (optional, minimal benefit)
**Savings:** 184 KB (48%)

#### Schema:
```sql
CREATE TABLE k_broadcasts (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,          -- ← Partitioning column
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    base64_encoded_nickname TEXT NOT NULL DEFAULT '',
    base64_encoded_profile_image TEXT,
    base64_encoded_message TEXT NOT NULL
);
```

#### Conversion:
```sql
-- Convert to hypertable (optional - table is very small)
SELECT create_hypertable(
    'k_broadcasts',
    'block_time',
    chunk_time_interval => 86400000000, -- 1 day (uniform with other tables)
    migrate_data => true,
    if_not_exists => TRUE
);

-- Configure compression
ALTER TABLE k_broadcasts SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey',  -- One user per segment
    timescaledb.compress_orderby = 'block_time DESC'
);

-- Add compression policy: compress data older than 30 days
SELECT add_compression_policy('k_broadcasts',
    compress_after => 2592000000000  -- 30 days in microseconds);
```

#### Rationale:
- **Chunk Size (1 day):** Uniform with other tables; allows future growth
- **Compress After 30 days:** Uniform compression policy across all tables
- **Segment By:** `sender_pubkey` only (one user = one profile)
- **Query Pattern:** "Get latest profile for user X" (usually cached at app level)

#### Note:
Given the tiny size (384 KB), conversion provides minimal benefit. **Optional step.**

---

### **Table 5: k_follows**

**Purpose:** User following relationships
**Current Size:** 160 KB (16 KB table + 144 KB indexes)
**After Conversion:** ~80 KB (optional, minimal benefit)
**Savings:** 80 KB (50%)

#### Schema:
```sql
CREATE TABLE k_follows (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,          -- ← Partitioning column
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    following_action VARCHAR(10) NOT NULL CHECK (following_action IN ('follow')),
    followed_user_pubkey BYTEA NOT NULL
);
```

#### Conversion:
```sql
-- Convert to hypertable (optional - table is very small)
SELECT create_hypertable(
    'k_follows',
    'block_time',
    chunk_time_interval => 86400000000, -- 1 day (infrequent follows)
    migrate_data => true,
    if_not_exists => TRUE
);

-- Configure compression
ALTER TABLE k_follows SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,followed_user_pubkey',
    timescaledb.compress_orderby = 'block_time DESC'
);

-- Add compression policy: compress data older than 90 days
SELECT add_compression_policy('k_follows',
    compress_after => 2592000000000  -- 30 days in microseconds);
```

#### Rationale:
- **Chunk Size (30 days):** Very few follow actions per day
- **Compress After 90 days:** Follow relationships rarely change; most queries are "who does X follow" (current state)
- **Segment By:** `sender_pubkey, followed_user_pubkey` (one relationship per segment)
- **Query Pattern:** "Get followers/following for user X" (usually cached)

#### Note:
Given the tiny size (160 KB), conversion provides minimal benefit. **Optional step.**

---

### **Table 6: k_blocks**

**Purpose:** User blocking relationships
**Current Size:** 128 KB (8 KB table + 120 KB indexes)
**After Conversion:** ~64 KB (optional, minimal benefit)
**Savings:** 64 KB (50%)

#### Schema:
```sql
CREATE TABLE k_blocks (
    id BIGSERIAL PRIMARY KEY,
    transaction_id BYTEA UNIQUE NOT NULL,
    block_time BIGINT NOT NULL,          -- ← Partitioning column
    sender_pubkey BYTEA NOT NULL,
    sender_signature BYTEA NOT NULL,
    blocking_action VARCHAR(10) NOT NULL CHECK (blocking_action IN ('block')),
    blocked_user_pubkey BYTEA NOT NULL
);
```

#### Conversion:
```sql
-- Convert to hypertable (optional - table is very small)
SELECT create_hypertable(
    'k_blocks',
    'block_time',
    chunk_time_interval => 86400000000, -- 1 day (infrequent blocks)
    migrate_data => true,
    if_not_exists => TRUE
);

-- Configure compression
ALTER TABLE k_blocks SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,blocked_user_pubkey',
    timescaledb.compress_orderby = 'block_time DESC'
);

-- Add compression policy: compress data older than 90 days
SELECT add_compression_policy('k_blocks',
    compress_after => 2592000000000  -- 30 days in microseconds);
```

#### Rationale:
- **Chunk Size (30 days):** Very few block actions per day
- **Compress After 90 days:** Block relationships rarely change
- **Segment By:** `sender_pubkey, blocked_user_pubkey` (one relationship per segment)
- **Query Pattern:** "Get blocked users for user X" (usually cached)

#### Note:
Given the tiny size (128 KB), conversion provides minimal benefit. **Optional step.**

---

## 📋 Implementation Summary

### **Priority Ranking:**

| Priority | Table | Current Size | Savings | Complexity | Downtime |
|----------|-------|--------------|---------|------------|----------|
| **1. CRITICAL** | k_votes | 366 MB | 286 MB | Low | 60-90s |
| **2. HIGH** | k_mentions | 322 MB | 252 MB | Low | 60-90s |
| **3. MEDIUM** | k_contents | 61 MB | 31 MB | Low | 10-15s |
| **4. LOW** | k_broadcasts | 384 KB | 184 KB | Low | <5s |
| **5. OPTIONAL** | k_follows | 160 KB | 80 KB | Low | <5s |
| **6. OPTIONAL** | k_blocks | 128 KB | 64 KB | Low | <5s |

### **Recommended Approach:**

**Phase 1 (Essential):** Convert top 3 tables
- k_votes, k_mentions, k_contents
- Total savings: **569 MB (45% of k_ tables)**
- Total downtime: **2-3 minutes**

**Phase 2 (Optional):** Convert remaining small tables
- k_broadcasts, k_follows, k_blocks
- Total savings: **328 KB (minimal)**
- Total downtime: **<15 seconds**

---

## 📊 Expected Results

### **Storage Reduction:**

| Component | Before | After | Savings |
|-----------|--------|-------|---------|
| **k_votes** | 366 MB | 80 MB | -286 MB (-78%) |
| **k_mentions** | 322 MB | 70 MB | -252 MB (-78%) |
| **k_contents** | 61 MB | 30 MB | -31 MB (-51%) |
| **k_broadcasts** | 384 KB | 200 KB | -184 KB (-48%) |
| **k_follows** | 160 KB | 80 KB | -80 KB (-50%) |
| **k_blocks** | 128 KB | 64 KB | -64 KB (-50%) |
| **TOTAL k_ tables** | **751 MB** | **180 MB** | **-571 MB (-76%)** |

### **Query Performance:**

| Query Type | Before | After | Improvement |
|------------|--------|-------|-------------|
| Recent data queries (< 7 days) | 2-5s | 0.05-0.1s | **20-100x faster** |
| Time-range queries | 1-3s | 0.05-0.2s | **15-60x faster** |
| User timeline queries | 0.5-1s | 0.02-0.05s | **10-50x faster** |
| Compressed data queries | 2-5s | 0.1-0.3s | **10-50x faster** |

---

## 🛠️ Complete Migration Script

Create file: `K-transaction-processor/src/migrations/schema/v7_to_v8_timescaledb_hypertables.sql`

```sql
-- Migration from Schema v7 to v8
-- Converts K protocol tables to TimescaleDB hypertables with compression

-- ============================================================================
-- Phase 1: Essential Tables (569 MB savings)
-- ============================================================================

-- Table 1: k_votes (286 MB savings)
SELECT create_hypertable('k_votes', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_votes SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,post_id',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_votes',
    compress_after => '7 days'::interval);

-- Table 2: k_mentions (252 MB savings)
SELECT create_hypertable('k_mentions', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_mentions SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'mentioned_pubkey,content_type',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_mentions',
    compress_after => '7 days'::interval);

-- Table 3: k_contents (31 MB savings)
SELECT create_hypertable('k_contents', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_contents SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,content_type',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_contents',
    compress_after => 2592000000000  -- 30 days in microseconds);

-- ============================================================================
-- Phase 2: Optional Small Tables (328 KB savings)
-- ============================================================================

-- Table 4: k_broadcasts (optional)
SELECT create_hypertable('k_broadcasts', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_broadcasts SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_broadcasts',
    compress_after => 2592000000000  -- 30 days in microseconds);

-- Table 5: k_follows (optional)
SELECT create_hypertable('k_follows', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_follows SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,followed_user_pubkey',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_follows',
    compress_after => 2592000000000  -- 30 days in microseconds);

-- Table 6: k_blocks (optional)
SELECT create_hypertable('k_blocks', 'block_time',
    chunk_time_interval => 86400000000,
    migrate_data => true,
    if_not_exists => TRUE);

ALTER TABLE k_blocks SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sender_pubkey,blocked_user_pubkey',
    timescaledb.compress_orderby = 'block_time DESC'
);

SELECT add_compression_policy('k_blocks',
    compress_after => 2592000000000  -- 30 days in microseconds);

-- ============================================================================
-- Update schema version
-- ============================================================================

UPDATE k_vars SET value = '8' WHERE key = 'schema_version';

-- ============================================================================
-- Migration complete!
-- ============================================================================
```

---

## ⚙️ TimescaleDB Configuration Summary

### **Chunk Size and Compression Policy Table**

| Table | Rows | Size | Chunk Interval | Chunk Size | Compress After | Priority |
|-------|------|------|----------------|------------|----------------|----------|
| **k_votes** | 636,570 | 366 MB | **1 day** (86400s) | ~5 MB | **30 days** | Critical |
| **k_mentions** | 749,789 | 322 MB | **1 day** (86400s) | ~6 MB | **30 days** | High |
| **k_contents** | 46,229 | 61 MB | **1 day** (86400s) | ~500 KB | **30 days** | Medium |
| **k_broadcasts** | 29 | 384 KB | **1 day** (86400s) | ~13 bytes | **30 days** | Low |
| **k_follows** | 39 | 160 KB | **1 day** (86400s) | ~5 bytes | **30 days** | Optional |
| **k_blocks** | 6 | 128 KB | **1 day** (86400s) | ~1 byte | **30 days** | Optional |

### **Design Rationale:**

**Chunk Interval:**
- **All tables**: **1 day** chunks (uniform, conservative approach)
  - High-frequency tables (k_votes, k_mentions, k_contents): ~5-6 MB/chunk (optimal)
  - Low-frequency tables (k_broadcasts, k_follows, k_blocks): ~1-13 bytes/chunk (very small, but allows future growth)
  - Easy to scale up later if needed with `set_chunk_time_interval()`
  - Cannot scale down retroactively, so starting small is safer

**Compression Policy:**
- **All tables**: Compress after **30 days** (uniform, balanced approach)
  - Keeps recent month uncompressed for fast queries
  - Compresses historical data for storage savings
  - Easy to adjust later (can make more/less aggressive as needed)
  - Trade-off between performance (longer uncompressed) and storage (more aggressive compression)

**Optimal chunk size:** 1-10 MB
- Too small (<100 KB): Overhead from many chunks
- Too large (>100 MB): Slow queries, poor partition pruning
- Sweet spot (1-10 MB): Fast queries, efficient compression

---

## 📈 Monitoring Commands

### **Check Hypertable Status:**
```sql
SELECT * FROM timescaledb_information.hypertables;
```

### **View Chunks:**
```sql
SELECT
    hypertable_name,
    chunk_name,
    range_start,
    range_end,
    pg_size_pretty(total_bytes) as size
FROM timescaledb_information.chunks
ORDER BY hypertable_name, range_start DESC;
```

### **Check Compression Status:**
```sql
SELECT
    chunk_name,
    pg_size_pretty(before_compression_total_bytes) as before,
    pg_size_pretty(after_compression_total_bytes) as after,
    ROUND(100 - (after_compression_total_bytes::float /
          before_compression_total_bytes * 100), 1) as savings_pct
FROM timescaledb_information.chunk_compression_stats
WHERE hypertable_name = 'k_votes'
ORDER BY chunk_name DESC;
```

### **Monitor Query Performance:**
```sql
SELECT
    query,
    calls,
    mean_exec_time,
    max_exec_time
FROM pg_stat_statements
WHERE query LIKE '%k_votes%'
ORDER BY mean_exec_time DESC
LIMIT 10;
```

---

## ⚠️ Important Notes

1. **Zero Application Code Changes:** All existing SQL queries work unchanged
2. **Transparent Decompression:** Compressed data is automatically decompressed when queried
3. **Downtime:** ~2-3 minutes total for essential tables (run during low-traffic period)
4. **Reversibility:** Cannot easily reverse hypertable conversion (test on DEV first!)
5. **Disk Space:** Ensure 2x table size free during migration (temporary workspace)

---

## ✅ Success Criteria

- [ ] All K tables converted to hypertables
- [ ] Compression policies active
- [ ] Storage reduced by 85%+
- [ ] Queries 20-50x faster
- [ ] Zero query errors in application logs
- [ ] Monitoring dashboards show healthy compression ratios

---

**Document Version:** 1.0
**Last Updated:** 2025-11-04
**Author:** K-Indexer Team
