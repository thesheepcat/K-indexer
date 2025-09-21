# Query Performance Monitoring with pg_stat_statements

## Setup

Enable pg_stat_statements when starting PostgreSQL:

```bash
postgres -c shared_preload_libraries=pg_stat_statements \
         -c pg_stat_statements.max=10000 \
         -c pg_stat_statements.track=all \
         -c pg_stat_statements.save=on
```

Create the extension:
```sql
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
```

## Common Performance Analysis Queries

### 1. Top 10 Slowest Queries (by average time)
```sql
SELECT
    query,
    calls,
    round(mean_exec_time::numeric, 2) as avg_time_ms,
    round(total_exec_time::numeric, 2) as total_time_ms,
    round((100.0 * total_exec_time / sum(total_exec_time) OVER())::numeric, 2) as percentage
FROM pg_stat_statements
WHERE calls > 1
ORDER BY mean_exec_time DESC
LIMIT 10;
```

### 2. Most Frequently Called Queries
```sql
SELECT
    query,
    calls,
    round(mean_exec_time::numeric, 2) as avg_time_ms,
    round((calls * mean_exec_time)::numeric, 2) as total_impact_ms
FROM pg_stat_statements
ORDER BY calls DESC
LIMIT 10;
```

### 3. Queries with Highest Total Time Impact
```sql
SELECT
    query,
    calls,
    round(total_exec_time::numeric, 2) as total_time_ms,
    round(mean_exec_time::numeric, 2) as avg_time_ms,
    round((100.0 * total_exec_time / sum(total_exec_time) OVER())::numeric, 2) as percentage
FROM pg_stat_statements
ORDER BY total_exec_time DESC
LIMIT 10;
```

### 4. Queries with Poor Cache Hit Ratio
```sql
SELECT
    query,
    calls,
    shared_blks_hit,
    shared_blks_read,
    round((100.0 * shared_blks_hit / NULLIF(shared_blks_hit + shared_blks_read, 0))::numeric, 2) as hit_ratio_pct
FROM pg_stat_statements
WHERE shared_blks_read > 0
ORDER BY hit_ratio_pct ASC
LIMIT 10;
```

### 5. Queries Using Temporary Files
```sql
SELECT
    query,
    calls,
    temp_blks_written,
    temp_blks_read,
    round((temp_blks_written * 8192 / 1024.0 / 1024.0)::numeric, 2) as temp_mb_written
FROM pg_stat_statements
WHERE temp_blks_written > 0
ORDER BY temp_blks_written DESC
LIMIT 10;
```

### 6. K-indexer Specific Queries Analysis
```sql
-- Analyze blocking-related queries
SELECT
    query,
    calls,
    round(mean_exec_time::numeric, 2) as avg_time_ms,
    round(total_exec_time::numeric, 2) as total_time_ms
FROM pg_stat_statements
WHERE query ILIKE '%k_blocks%'
ORDER BY total_exec_time DESC;

-- Analyze broadcast queries
SELECT
    query,
    calls,
    round(mean_exec_time::numeric, 2) as avg_time_ms
FROM pg_stat_statements
WHERE query ILIKE '%k_broadcasts%'
ORDER BY mean_exec_time DESC;

-- Analyze post/reply queries
SELECT
    query,
    calls,
    round(mean_exec_time::numeric, 2) as avg_time_ms
FROM pg_stat_statements
WHERE query ILIKE '%k_posts%' OR query ILIKE '%k_replies%'
ORDER BY calls DESC;

-- Top 3 slowest queries on k_ tables with execution/planning breakdown
SELECT
    LEFT(query, 100) as query_type,
    calls,
    -- Execution times
    ROUND(min_exec_time::numeric, 2) as min_exec_ms,
    ROUND(mean_exec_time::numeric, 2) as avg_exec_ms,
    ROUND(max_exec_time::numeric, 2) as max_exec_ms,
    -- Planning times
    ROUND(min_plan_time::numeric, 2) as min_plan_ms,
    ROUND(mean_plan_time::numeric, 2) as avg_plan_ms,
    ROUND(max_plan_time::numeric, 2) as max_plan_ms,
    ROUND(total_exec_time::numeric, 2) as total_exec_ms,
    ROUND(total_plan_time::numeric, 2) as total_plan_ms
FROM pg_stat_statements
WHERE query ILIKE '%k_blocks%' OR query ILIKE '%k_broadcasts%' OR query ILIKE '%k_mentions%'
   OR query ILIKE '%k_posts%' OR query ILIKE '%k_replies%' OR query ILIKE '%k_vars%' OR query ILIKE '%k_votes%'
ORDER BY total_exec_time DESC
LIMIT 3;
```

### 7. Overall Database Performance Summary
```sql
SELECT
    sum(calls) as total_queries,
    round(sum(total_exec_time)::numeric, 2) as total_time_ms,
    round(avg(mean_exec_time)::numeric, 2) as overall_avg_time_ms,
    count(*) as unique_queries
FROM pg_stat_statements;
```

## Maintenance Commands

### Reset Statistics
```sql
-- Reset all statistics
SELECT pg_stat_statements_reset();

-- Reset statistics for specific query (by queryid)
SELECT pg_stat_statements_reset(queryid);
```

### Monitor Current Activity
```sql
-- Check currently running queries
SELECT
    pid,
    state,
    query_start,
    now() - query_start as duration,
    left(query, 100) as query_preview
FROM pg_stat_activity
WHERE state != 'idle'
ORDER BY query_start;
```

## Performance Tuning Tips

1. **Focus on queries with high total_exec_time** - These have the biggest performance impact
2. **Optimize frequently called queries** - Even small improvements have big cumulative effects
3. **Check cache hit ratios** - Low ratios may indicate missing indexes or inefficient queries
4. **Monitor temp file usage** - Indicates queries that need more work_mem or better indexing
5. **Regular analysis** - Run these queries periodically to track performance trends

## Alerting Thresholds

- **Average query time > 100ms** - Investigate for optimization
- **Cache hit ratio < 95%** - Consider indexing improvements
- **Temp file usage > 100MB** - Query may need optimization or more memory
- **Total time impact > 10%** - High-priority optimization candidate