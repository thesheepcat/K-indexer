# Hashtag API Endpoints - Technical Specifications

This document provides technical specifications for the hashtag-related REST API endpoints for the K-indexer system.

---

## Overview

The hashtag API provides endpoints for discovering and querying content by hashtags, autocomplete suggestions, trending hashtags, and user activity around specific hashtags.

**Pattern Consistency with Existing API:**
- All endpoints follow kebab-case naming convention (e.g., `get-hashtag-content`)
- Pagination uses the same `PaginationMetadata` structure as existing endpoints
- Response field names use camelCase (matching `ServerPost` model)
- Error responses follow the same structure as existing endpoints
- Query parameters use camelCase (e.g., `requesterPubkey`)
- All endpoints return JSON responses

---

## General Pagination Rules

The hashtag endpoints use cursor-based pagination consistent with the existing K-indexer API.

### Pagination Parameters

- `limit` (optional): Number of items to return
  - Default: 20
  - Maximum: 100
  - Minimum: 1

- `before` (optional): Unix timestamp cursor
  - Returns items created before this timestamp
  - Used for paginating to older content
  - Example: `before=1703185000`

- `after` (optional): Unix timestamp cursor
  - Returns items created after this timestamp
  - Used for fetching newer content
  - Example: `after=1703190000`

### Pagination Response Format

All paginated endpoints include a `pagination` object in the response:

```json
{
  "pagination": {
    "hasMore": true,
    "nextCursor": "1703184000",
    "prevCursor": "1703186000"
  }
}
```

- `hasMore`: Boolean indicating if more older items are available
- `nextCursor`: Timestamp for the next page of older items (use with `before`)
- `prevCursor`: Timestamp for newer items (use with `after`)
- Cursors are `null` when no more content is available in that direction

---

## API Endpoint Details

### 1. Get Content by Hashtag

Find all content (posts, replies, quotes) with a specific hashtag, newest first, with cursor pagination.

**Endpoint:** `GET /get-hashtag-content`

**Query Parameters:**
- `hashtag` (string, required) - The hashtag to search for (without # symbol, case-insensitive)
- `requesterPubkey` (string, optional) - Public key of the requester for voting status (66-character hex string with 02/03 prefix)
- `limit` (integer, optional, default: 20, max: 100) - Number of results to return
- `before` (integer, optional) - Return content created before this timestamp (for pagination to older content)
- `after` (integer, optional) - Return content created after this timestamp (for fetching newer content)

**Example Requests:**
```bash
# First page (latest 20 posts with #rust)
curl "http://localhost:3001/get-hashtag-content?hashtag=rust&limit=20"

# With requester for voting status
curl "http://localhost:3001/get-hashtag-content?hashtag=rust&requesterPubkey=02218b3732df2353978154ec5323b745bce9520a5ed506a96de4f4e3dad20dc44f&limit=20"

# Next page (older content)
curl "http://localhost:3001/get-hashtag-content?hashtag=rust&limit=20&before=1703185000"

# Check for newer content
curl "http://localhost:3001/get-hashtag-content?hashtag=rust&limit=20&after=1703190000"
```

**Response:**
```json
{
  "posts": [
    {
      "id": "f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2",
      "userPublicKey": "021234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12",
      "postContent": "TGVhcm5pbmcgI3J1c3QgaXMgYW1hemluZyEgI3Byb2dyYW1taW5n",
      "signature": "3045022100f1f2f3f4f5f6f7f8f9f0f1f2f3f4f5f6f7f8f9f0f1f2f3f4f5f6f7f8f9f0f1f2022071f2f3f4f5f6f7f8f9f0f1f2f3f4f5f6f7f8f9f0f1f2f3f4f5f6f7f8f9f0f1f2",
      "timestamp": 1703186000,
      "repliesCount": 5,
      "upVotesCount": 12,
      "downVotesCount": 1,
      "repostsCount": 3,
      "parentPostId": null,
      "mentionedPubkeys": [],
      "isUpvoted": false,
      "isDownvoted": false,
      "userNickname": "TWFyeQ==",
      "userProfileImage": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg=="
    }
  ],
  "pagination": {
    "hasMore": true,
    "nextCursor": "1703185000",
    "prevCursor": "1703187000"
  }
}
```

**Response Fields:**
- `posts`: Array of post objects (same structure as `get-posts-following`)
- `pagination`: Standard pagination metadata

**Notes:**
- Returns posts, replies, and quotes containing the specified hashtag
- If `requesterPubkey` is provided, includes voting status (`isUpvoted`, `isDownvoted`)
- Hashtag matching is case-insensitive
- User profile fields (`userNickname`, `userProfileImage`) are included when available

---

### 2. Count Hashtag Usage

Get the total number of times a hashtag has been used across all content.

**Endpoint:** `GET /get-hashtag-count`

**Query Parameters:**
- `hashtag` (string, required) - The hashtag to count (without # symbol, case-insensitive)

**Example Request:**
```bash
curl "http://localhost:3001/get-hashtag-count?hashtag=programming"
```

**Response:**
```json
{
  "hashtag": "programming",
  "count": 1523
}
```

**Response Fields:**
- `hashtag`: The queried hashtag (normalized to lowercase)
- `count`: Total number of times this hashtag has been used

---

### 3. Autocomplete Hashtags

Find hashtags starting with a given prefix (for search-as-you-type features).

**Endpoint:** `GET /autocomplete-hashtags`

**Query Parameters:**
- `prefix` (string, required) - The prefix to search for (minimum 1 character)
- `limit` (integer, optional, default: 20, max: 100) - Number of results to return

**Example Requests:**
```bash
# Search for hashtags starting with "rus"
curl "http://localhost:3001/autocomplete-hashtags?prefix=rus&limit=10"

# Get more results
curl "http://localhost:3001/autocomplete-hashtags?prefix=r&limit=50"
```

**Response:**
```json
{
  "prefix": "rus",
  "hashtags": [
    {
      "hashtag": "rust",
      "usageCount": 542
    },
    {
      "hashtag": "russia",
      "usageCount": 234
    },
    {
      "hashtag": "rust_lang",
      "usageCount": 156
    }
  ]
}
```

**Response Fields:**
- `prefix`: The search prefix
- `hashtags`: Array of matching hashtags
  - `hashtag`: The hashtag text
  - `usageCount`: Number of times this hashtag has been used
- Results are ordered by usage count (descending)

**Notes:**
- Very fast query using `idx_k_hashtags_pattern` with `text_pattern_ops`
- Prefix matching is case-insensitive
- Only returns hashtags that have been used at least once

---

### 4. Search Hashtags

Find hashtags containing a specific substring anywhere in the tag.

**Endpoint:** `GET /search-hashtags`

**Query Parameters:**
- `query` (string, required) - The substring to search for (minimum 1 character)
- `limit` (integer, optional, default: 20, max: 100) - Number of results to return

**Example Request:**
```bash
curl "http://localhost:3001/search-hashtags?query=script&limit=20"
```

**Response:**
```json
{
  "query": "script",
  "hashtags": [
    {
      "hashtag": "javascript",
      "usageCount": 892
    },
    {
      "hashtag": "typescript",
      "usageCount": 445
    },
    {
      "hashtag": "scripting",
      "usageCount": 123
    }
  ]
}
```

**Response Fields:**
- `query`: The search query
- `hashtags`: Array of matching hashtags
  - `hashtag`: The hashtag text
  - `usageCount`: Number of times this hashtag has been used
- Results are ordered by usage count (descending)

**Notes:**
- Slower than prefix search but still indexed
- Substring matching is case-insensitive
- For better performance, use `autocomplete-hashtags` when possible

---

### 5. Trending Hashtags

Get the most used hashtags within a time window.

**Endpoint:** `GET /get-trending-hashtags`

**Query Parameters:**
- `timeWindow` (string, optional, default: "24h") - Time window: "1h", "6h", "24h", "7d", "30d"
- `limit` (integer, optional, default: 20, max: 100) - Number of results to return

**Example Requests:**
```bash
# Get top 20 trending hashtags in the last 24 hours
curl "http://localhost:3001/get-trending-hashtags?timeWindow=24h&limit=20"

# Get top 10 trending hashtags in the last 7 days
curl "http://localhost:3001/get-trending-hashtags?timeWindow=7d&limit=10"
```

**Response:**
```json
{
  "timeWindow": "24h",
  "fromTime": 1703100000,
  "toTime": 1703186400,
  "hashtags": [
    {
      "hashtag": "kaspa",
      "usageCount": 1234,
      "rank": 1
    },
    {
      "hashtag": "crypto",
      "usageCount": 987,
      "rank": 2
    },
    {
      "hashtag": "defi",
      "usageCount": 654,
      "rank": 3
    }
  ]
}
```

**Response Fields:**
- `timeWindow`: The requested time window
- `fromTime`: Start of time window (Unix timestamp)
- `toTime`: End of time window (Unix timestamp)
- `hashtags`: Array of trending hashtags
  - `hashtag`: The hashtag text
  - `usageCount`: Number of uses within the time window
  - `rank`: Ranking position (1 = most used)

**Time Window Mappings:**
- `1h`: Last 1 hour
- `6h`: Last 6 hours
- `24h`: Last 24 hours (default)
- `7d`: Last 7 days
- `30d`: Last 30 days

---

### 6. Get Content by Hashtag and User

Find all content with a specific hashtag by a specific user, newest first, with cursor pagination.

**Endpoint:** `GET /get-hashtag-content-by-user`

**Query Parameters:**
- `hashtag` (string, required) - The hashtag to search for (without # symbol, case-insensitive)
- `userPubkey` (string, required) - The user's public key (66-character hex string with 02/03 prefix)
- `requesterPubkey` (string, optional) - Public key of the requester for voting status
- `limit` (integer, optional, default: 20, max: 100) - Number of results to return
- `before` (integer, optional) - Return content created before this timestamp
- `after` (integer, optional) - Return content created after this timestamp

**Example Requests:**
```bash
# First page
curl "http://localhost:3001/get-hashtag-content-by-user?hashtag=rust&userPubkey=021234567890abcdef&limit=20"

# With requester for voting status
curl "http://localhost:3001/get-hashtag-content-by-user?hashtag=rust&userPubkey=021234567890abcdef&requesterPubkey=02218b3732df2353978154ec5323b745bce9520a5ed506a96de4f4e3dad20dc44f&limit=20"

# Next page
curl "http://localhost:3001/get-hashtag-content-by-user?hashtag=rust&userPubkey=021234567890abcdef&limit=20&before=1703185000"
```

**Response:**
```json
{
  "posts": [
    {
      "id": "def456...",
      "userPublicKey": "021234567890abcdef...",
      "postContent": "TXkgdGhvdWdodHMgb24gI3J1c3Q=",
      "signature": "3045...",
      "timestamp": 1703186000,
      "repliesCount": 2,
      "upVotesCount": 8,
      "downVotesCount": 0,
      "repostsCount": 1,
      "parentPostId": null,
      "mentionedPubkeys": [],
      "isUpvoted": true,
      "isDownvoted": false,
      "userNickname": "Sm9obg==",
      "userProfileImage": "iVBORw0KG..."
    }
  ],
  "pagination": {
    "hasMore": true,
    "nextCursor": "1703185000",
    "prevCursor": "1703187000"
  }
}
```

**Response Fields:**
- Same structure as `get-hashtag-content`
- `posts`: Array of post objects from the specified user
- `pagination`: Standard pagination metadata

---

### 7. Top Users by Hashtag

Get the most active users posting about a specific hashtag within a time window.

**Endpoint:** `GET /get-hashtag-top-users`

**Query Parameters:**
- `hashtag` (string, required) - The hashtag to analyze (without # symbol, case-insensitive)
- `timeWindow` (string, optional, default: "24h") - Time window: "1h", "6h", "24h", "7d", "30d"
- `limit` (integer, optional, default: 20, max: 100) - Number of results to return

**Example Requests:**
```bash
# Get top 20 users posting about #programming in the last 24 hours
curl "http://localhost:3001/get-hashtag-top-users?hashtag=programming&timeWindow=24h&limit=20"

# Get top 10 users in the last 7 days
curl "http://localhost:3001/get-hashtag-top-users?hashtag=kaspa&timeWindow=7d&limit=10"
```

**Response:**
```json
{
  "hashtag": "programming",
  "timeWindow": "7d",
  "fromTime": 1702581690,
  "toTime": 1703186490,
  "topUsers": [
    {
      "userPublicKey": "021234567890abcdef...",
      "postCount": 45,
      "rank": 1,
      "userNickname": "QWxpY2U=",
      "userProfileImage": "iVBORw0KG..."
    },
    {
      "userPublicKey": "03456789abcdef012...",
      "postCount": 32,
      "rank": 2,
      "userNickname": "Qm9i",
      "userProfileImage": null
    }
  ]
}
```

**Response Fields:**
- `hashtag`: The analyzed hashtag
- `timeWindow`: The requested time window
- `fromTime`: Start of time window (Unix timestamp)
- `toTime`: End of time window (Unix timestamp)
- `topUsers`: Array of user activity data
  - `userPublicKey`: User's public key
  - `postCount`: Number of posts using this hashtag in the time window
  - `rank`: Ranking position (1 = most active)
  - `userNickname`: Base64 encoded nickname (optional)
  - `userProfileImage`: Base64 encoded profile image (optional)

**Notes:**
- User profile fields are enriched from `k_broadcasts` table
- Results are ordered by post count (descending)

---

## Error Responses

All endpoints follow the same error response structure as existing K-indexer APIs.

**Status Code:** `400 BAD_REQUEST`

Missing or invalid parameters:
```json
{
  "error": "Invalid parameter",
  "code": "INVALID_PARAMETER"
}
```

**Status Code:** `404 NOT_FOUND`

Hashtag not found:
```json
{
  "error": "Hashtag not found",
  "code": "NOT_FOUND"
}
```

**Status Code:** `429 TOO_MANY_REQUESTS`

Rate limit exceeded:
```json
{
  "error": "Too many requests",
  "code": "RATE_LIMIT_EXCEEDED"
}
```

**Status Code:** `500 INTERNAL_SERVER_ERROR`

Database or server error:
```json
{
  "error": "Internal server error during database query",
  "code": "DATABASE_ERROR"
}
```

---

## Implementation Notes

### Database Queries

All endpoints leverage the specialized hashtag indexes:

1. **get-hashtag-content** - Uses `idx_k_hashtags_by_hashtag_time`
2. **get-hashtag-count** - Uses `idx_k_hashtags_by_hashtag_time`
3. **autocomplete-hashtags** - Uses `idx_k_hashtags_pattern` (prefix)
4. **search-hashtags** - Uses `idx_k_hashtags_pattern` (contains)
5. **get-trending-hashtags** - Uses `idx_k_hashtags_trending`
6. **get-hashtag-content-by-user** - Uses `idx_k_hashtags_by_hashtag_sender`
7. **get-hashtag-top-users** - Uses `idx_k_hashtags_by_hashtag_sender`

### General Notes

- All hashtag parameters should be provided **without** the `#` symbol
- Hashtag matching is **case-insensitive** (stored and queried in lowercase)
- Maximum hashtag length is **30 characters**
- All timestamps are in Unix epoch format (seconds)
- Rate limiting applies to all endpoints (same as existing API)
- All endpoints are public (no authentication required)
- Content from blocked users is not filtered (client-side responsibility)
