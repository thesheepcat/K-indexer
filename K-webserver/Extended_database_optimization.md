# Extended Database Optimization Analysis

## Current Index Status Analysis

### Existing Indexes (from new-transaction-processor/migrations.rs)

```sql
-- k_posts
CREATE INDEX IF NOT EXISTS idx_k_posts_sender_pubkey ON k_posts(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_posts_block_time ON k_posts(block_time);

-- k_replies  
CREATE INDEX IF NOT EXISTS idx_k_replies_sender_pubkey ON k_replies(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_replies_post_id ON k_replies(post_id);
CREATE INDEX IF NOT EXISTS idx_k_replies_block_time ON k_replies(block_time);
CREATE INDEX IF NOT EXISTS idx_k_replies_post_id_block_time ON k_replies(post_id, block_time DESC);

-- k_broadcasts
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_sender_pubkey ON k_broadcasts(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_block_time ON k_broadcasts(block_time);

-- k_votes
CREATE INDEX IF NOT EXISTS idx_k_votes_sender_pubkey ON k_votes(sender_pubkey);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id ON k_votes(post_id);
CREATE INDEX IF NOT EXISTS idx_k_votes_vote ON k_votes(vote);
CREATE INDEX IF NOT EXISTS idx_k_votes_block_time ON k_votes(block_time);
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id_sender ON k_votes(post_id, sender_pubkey);
```

## Query Pattern Analysis from database_postgres_impl.rs

### k_posts Table Query Patterns

1. **Get Posts by User** (Lines 111-156)
   - WHERE: `sender_pubkey = $1`
   - Optional: `block_time < $1`, `block_time > $1`
   - ORDER BY: `block_time DESC/ASC`

2. **Get All Posts** (Lines 198-243)
   - WHERE: Time-based filtering only
   - Optional: `block_time < $1`, `block_time > $1`
   - ORDER BY: `block_time DESC/ASC`

3. **Get Post by ID** (Lines 281-292)
   - WHERE: `transaction_id = $1` (Primary Key)

4. **Posts Mentioning User** (Lines 324-354)
   - WHERE: `mentioned_pubkeys @> $1` (JSONB containment)
   - ORDER BY: `block_time DESC/ASC` (on UNION result)

### k_replies Table Query Patterns

5. **Get Replies by Post ID** (Lines 439-484)
   - WHERE: `post_id = $1`
   - Optional: `block_time < $1`, `block_time > $1`
   - ORDER BY: `block_time DESC/ASC`

6. **Get Replies by User** (Lines 530-575)
   - WHERE: `sender_pubkey = $1`
   - Optional: `block_time < $1`, `block_time > $1`
   - ORDER BY: `block_time DESC/ASC`

7. **Get Reply by ID** (Lines 615-626)
   - WHERE: `transaction_id = $1` (Primary Key)

8. **Replies Mentioning User** (Lines 659-705)
   - WHERE: `mentioned_pubkeys @> $1` (JSONB containment)
   - Optional: `block_time < $1`, `block_time > $1`
   - ORDER BY: `block_time DESC/ASC`

9. **Count Replies for Post** (Lines 735-742)
   - WHERE: `post_id = $1`

### k_broadcasts Table Query Patterns

10. **Get All Broadcasts** (Lines 753-798)
    - WHERE: Time-based filtering only
    - Optional: `block_time < $1`, `block_time > $1`
    - ORDER BY: `block_time DESC/ASC`

11. **Get Latest Broadcast by User** (Lines 839-852)
    - WHERE: `sender_pubkey = $1`
    - ORDER BY: `block_time DESC`
    - LIMIT: 1

### k_votes Table Query Patterns

12. **Get Votes for Post** (Lines 880-892)
    - WHERE: `post_id = $1`
    - ORDER BY: `block_time DESC`

13. **Get Vote Counts** (Lines 920-932)
    - WHERE: `post_id = $1`
    - Aggregation: `COUNT(*) FILTER (WHERE vote = 'upvote/downvote')`

14. **Get User Vote for Post** (Lines 948-962)
    - WHERE: `post_id = $1 AND sender_pubkey = $2`
    - ORDER BY: `block_time DESC`
    - LIMIT: 1

## Missing Critical Indexes

### HIGH PRIORITY - JSONB Indexes

❌ **Missing GIN indexes for JSONB containment queries**

The `get-mentions` API performs `mentioned_pubkeys @> $1` queries on both k_posts and k_replies tables. Without GIN indexes, these queries will perform full table scans and be extremely slow on large datasets.

```sql
CREATE INDEX IF NOT EXISTS idx_k_posts_mentioned_pubkeys_gin 
    ON k_posts USING GIN (mentioned_pubkeys);

CREATE INDEX IF NOT EXISTS idx_k_replies_mentioned_pubkeys_gin 
    ON k_replies USING GIN (mentioned_pubkeys);
```

### MEDIUM PRIORITY - Composite Indexes

❌ **Missing composite indexes for common query patterns**

Current single-column indexes are insufficient for queries that filter by one column and order by another.

```sql
-- k_posts: User posts with time filtering and ordering
CREATE INDEX IF NOT EXISTS idx_k_posts_sender_pubkey_block_time 
    ON k_posts(sender_pubkey, block_time DESC);

-- k_replies: User replies with time filtering and ordering  
CREATE INDEX IF NOT EXISTS idx_k_replies_sender_pubkey_block_time 
    ON k_replies(sender_pubkey, block_time DESC);

-- k_broadcasts: Latest user broadcast optimization
CREATE INDEX IF NOT EXISTS idx_k_broadcasts_sender_pubkey_block_time 
    ON k_broadcasts(sender_pubkey, block_time DESC);

-- k_votes: Vote aggregation optimization
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id_vote 
    ON k_votes(post_id, vote);

-- k_votes: User vote lookup with time ordering
CREATE INDEX IF NOT EXISTS idx_k_votes_post_id_sender_pubkey_block_time 
    ON k_votes(post_id, sender_pubkey, block_time DESC);
```

## Performance Impact Analysis

### Without GIN Indexes (HIGH IMPACT)
- **get-mentions API**: O(n) full table scans on k_posts and k_replies
- **Scaling**: Performance degrades linearly with table size
- **User experience**: Slow response times for mention queries

### Without Composite Indexes (MEDIUM IMPACT)
- **User-specific queries**: Inefficient index usage requiring sorts
- **Pagination**: Suboptimal performance for paginated results
- **Vote queries**: Multiple index lookups instead of single composite scan

## Recommended Implementation

### Migration 003: Add Missing Indexes

```sql
-- Migration 003: Optimize indexes for new-webserver query patterns

-- CRITICAL: GIN indexes for JSONB containment queries (get-mentions API)
CREATE INDEX IF NOT EXISTS idx_k_posts_mentioned_pubkeys_gin 
    ON k_posts USING GIN (mentioned_pubkeys);

CREATE INDEX IF NOT EXISTS idx_k_replies_mentioned_pubkeys_gin 
    ON k_replies USING GIN (mentioned_pubkeys);

-- OPTIMIZATION: Composite indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_k_posts_sender_pubkey_block_time 
    ON k_posts(sender_pubkey, block_time DESC);

CREATE INDEX IF NOT EXISTS idx_k_replies_sender_pubkey_block_time 
    ON k_replies(sender_pubkey, block_time DESC);

CREATE INDEX IF NOT EXISTS idx_k_broadcasts_sender_pubkey_block_time 
    ON k_broadcasts(sender_pubkey, block_time DESC);

CREATE INDEX IF NOT EXISTS idx_k_votes_post_id_vote 
    ON k_votes(post_id, vote);

CREATE INDEX IF NOT EXISTS idx_k_votes_post_id_sender_pubkey_block_time 
    ON k_votes(post_id, sender_pubkey, block_time DESC);

-- Documentation comments
COMMENT ON INDEX idx_k_posts_mentioned_pubkeys_gin IS 'GIN index for fast JSONB containment queries in get-mentions API';
COMMENT ON INDEX idx_k_replies_mentioned_pubkeys_gin IS 'GIN index for fast JSONB containment queries in get-mentions API';
COMMENT ON INDEX idx_k_posts_sender_pubkey_block_time IS 'Composite index for user posts with time-based pagination';
COMMENT ON INDEX idx_k_replies_sender_pubkey_block_time IS 'Composite index for user replies with time-based pagination';
COMMENT ON INDEX idx_k_broadcasts_sender_pubkey_block_time IS 'Composite index for latest user broadcast lookup';
COMMENT ON INDEX idx_k_votes_post_id_vote IS 'Composite index for vote count aggregation';
COMMENT ON INDEX idx_k_votes_post_id_sender_pubkey_block_time IS 'Composite index for user-specific vote lookup';
```

## Query-to-Index Mapping

| API Endpoint | Query Pattern | Required Index |
|--------------|---------------|----------------|
| get-mentions | `mentioned_pubkeys @> $1` | GIN(mentioned_pubkeys) |
| get-posts | `sender_pubkey = $1 ORDER BY block_time` | (sender_pubkey, block_time) |
| get-replies | `post_id = $1 ORDER BY block_time` | (post_id, block_time) ✅ |
| get-users | `sender_pubkey = $1 ORDER BY block_time DESC LIMIT 1` | (sender_pubkey, block_time DESC) |
| Vote counts | `post_id = $1 AND vote = $2` | (post_id, vote) |
| User votes | `post_id = $1 AND sender_pubkey = $2` | (post_id, sender_pubkey, block_time) |

## Implementation Priority

1. **IMMEDIATE**: Add GIN indexes for mentioned_pubkeys (critical for get-mentions performance)
2. **NEXT**: Add composite indexes for user-specific queries with time ordering
3. **LATER**: Monitor query performance and add additional indexes as needed

## Monitoring Recommendations

After implementing indexes, monitor:
- Query execution times for get-mentions API
- Index usage statistics (`pg_stat_user_indexes`)
- Query plans for major endpoints (`EXPLAIN ANALYZE`)
- Index size growth over time