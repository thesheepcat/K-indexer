use axum::{
    Router,
    extract::{ConnectInfo, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
};
use axum_prometheus::PrometheusMetricLayer;
use serde::Deserialize;
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::RwLock, time::Instant};
use tower_http::{
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    timeout::TimeoutLayer,
};
use tracing::{error as log_error, info as log_info};

use crate::api_handlers::ApiHandlers;
use crate::config::ServerConfig;
use crate::database_trait::DatabaseInterface;
use crate::models::{
    ApiError, PaginatedNotificationsResponse, PaginatedPostsResponse, PaginatedRepliesResponse,
    PaginatedUsersResponse, PostDetailsResponse, ServerUserPost, TrendingHashtagsResponse,
};

#[derive(Debug, Clone)]
pub(crate) struct RateLimitEntry {
    count: u32,
    window_start: Instant,
}

type RateLimitMap = Arc<RwLock<HashMap<SocketAddr, RateLimitEntry>>>;

pub struct AppState {
    pub api_handlers: ApiHandlers,
    pub rate_limit_map: RateLimitMap,
    pub server_config: ServerConfig,
    pub db: Arc<dyn DatabaseInterface>,
}

pub struct WebServer {
    pub app_state: Arc<AppState>,
}

#[derive(Debug, Deserialize)]
struct GetPostsQuery {
    user: Option<String>,
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetRepliesQuery {
    post: Option<String>,
    user: Option<String>,
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>, // Changed to String to support compound cursors
    after: Option<String>,  // Changed to String to support compound cursors
}

#[derive(Debug, Deserialize)]
struct GetPostsWatchingQuery {
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetContentsFollowingQuery {
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetUsersQuery {
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetMostActiveUsersQuery {
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    #[serde(rename = "timeWindow")]
    time_window: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchUsersQuery {
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    #[serde(rename = "searchedUserPubkey")]
    searched_user_pubkey: Option<String>,
    #[serde(rename = "searchedUserNickname")]
    searched_user_nickname: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetMentionsQuery {
    user: Option<String>,
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetNotificationsQuery {
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetHashtagContentQuery {
    hashtag: Option<String>,
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetTrendingHashtagsQuery {
    #[serde(rename = "timeWindow")]
    time_window: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct GetPostDetailsQuery {
    id: Option<String>,
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetUserDetailsQuery {
    user: Option<String>,
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetBlockedUsersQuery {
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetFollowedUsersQuery {
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetUsersFollowingQuery {
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    #[serde(rename = "userPubkey")]
    user_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetUsersFollowersQuery {
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    #[serde(rename = "userPubkey")]
    user_pubkey: Option<String>,
    limit: Option<u32>,
    before: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetNotificationsCountQuery {
    #[serde(rename = "requesterPubkey")]
    requester_pubkey: Option<String>,
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetUsersCountQuery {}

impl WebServer {
    pub async fn new(db: Arc<dyn DatabaseInterface>, server_config: ServerConfig) -> Self {
        let api_handlers = ApiHandlers::new(db.clone());
        let rate_limit_map = Arc::new(RwLock::new(HashMap::new()));

        let app_state = Arc::new(AppState {
            api_handlers,
            rate_limit_map,
            server_config,
            db,
        });

        Self { app_state }
    }

    pub fn create_router(&self) -> Router {
        let timeout_duration = Duration::from_secs(self.app_state.server_config.request_timeout);
        let (prometheus_layer, metric_handle) = PrometheusMetricLayer::pair();

        Router::new()
            .route("/", get(handle_root))
            .route("/health", get(handle_health))
            .route("/stats", get(handle_stats))
            .route(
                "/metrics",
                get(move || async move { metric_handle.render() }),
            )
            .route("/get-posts", get(handle_get_posts))
            .route("/get-post-details", get(handle_get_post_details))
            .route("/get-posts-watching", get(handle_get_posts_watching))
            .route(
                "/get-contents-following",
                get(handle_get_contents_following),
            )
            .route("/get-replies", get(handle_get_replies))
            .route("/get-mentions", get(handle_get_mentions))
            .route("/get-users", get(handle_get_users))
            .route("/get-most-active-users", get(handle_get_most_active_users))
            .route("/get-users-count", get(handle_get_users_count))
            .route("/search-users", get(handle_search_users))
            .route("/get-user-details", get(handle_get_user_details))
            .route("/get-followed-users", get(handle_get_followed_users))
            .route("/get-users-following", get(handle_get_users_following))
            .route("/get-users-followers", get(handle_get_users_followers))
            .route("/get-blocked-users", get(handle_get_blocked_users))
            .route(
                "/get-notifications-count",
                get(handle_get_notifications_count),
            )
            .route("/get-notifications", get(handle_get_notifications))
            .route("/get-hashtag-content", get(handle_get_hashtag_content))
            .route("/get-trending-hashtags", get(handle_get_trending_hashtags))
            .layer(prometheus_layer)
            .layer(TimeoutLayer::new(timeout_duration))
            .layer(RequestBodyLimitLayer::new(1024 * 1024)) // 1MB limit
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(self.app_state.clone())
    }

    pub async fn serve(&self, bind_address: &str) -> Result<(), Box<dyn std::error::Error>> {
        let router = self.create_router();
        let listener = TcpListener::bind(bind_address).await?;

        log_info!("Web server starting on {}", bind_address);
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;

        Ok(())
    }
}

// Rate limiting middleware
async fn check_rate_limit(
    state: &AppState,
    client_addr: SocketAddr,
) -> Result<(), (StatusCode, Json<ApiError>)> {
    let now = Instant::now();
    let mut rate_limits = state.rate_limit_map.write().await;

    let entry = rate_limits.entry(client_addr).or_insert(RateLimitEntry {
        count: 0,
        window_start: now,
    });

    // Reset window if 1 minute has passed
    if now.duration_since(entry.window_start) >= Duration::from_secs(60) {
        entry.count = 0;
        entry.window_start = now;
    }

    entry.count += 1;

    if entry.count > state.server_config.rate_limit {
        let error = ApiError {
            error: "Rate limit exceeded. Too many requests per minute.".to_string(),
            code: "RATE_LIMIT_EXCEEDED".to_string(),
        };
        return Err((StatusCode::TOO_MANY_REQUESTS, Json(error)));
    }

    Ok(())
}

// API Handler Functions

async fn handle_root() -> &'static str {
    "K-indexer API Server - Posts API v1.0"
}

async fn handle_health(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Query database for network on every health check
    let network = app_state
        .db
        .get_network()
        .await
        .unwrap_or_else(|_| "unknown".to_string());

    Ok(Json(serde_json::json!({
        "status": "healthy",
        "service": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "network": network
    })))
}

async fn handle_stats(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    match app_state.db.get_stats().await {
        Ok(stats) => Ok(Json(serde_json::json!({
            "broadcasts": stats.broadcasts_count,
            "posts": stats.posts_count,
            "replies": stats.replies_count,
            "quotes": stats.quotes_count,
            "votes": stats.votes_count,
            "follows": stats.follows_count,
            "blocks": stats.blocks_count
        }))),
        Err(e) => {
            log_error!("Failed to get database stats: {}", e);
            let error = ApiError {
                error: "Failed to retrieve database statistics".to_string(),
                code: "INTERNAL_ERROR".to_string(),
            };
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
        }
    }
}

async fn handle_get_posts(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetPostsQuery>,
) -> Result<Json<PaginatedPostsResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;
    // Check if user parameter is provided
    let user_public_key = match params.user {
        Some(user) => user,
        None => {
            let error = ApiError {
                error: "Missing required parameter: user".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated posts for the user with voting status
    match app_state
        .api_handlers
        .get_posts_paginated(
            &user_public_key,
            &requester_pubkey,
            limit,
            params.before,
            params.after,
        )
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedPostsResponse
            match serde_json::from_str::<PaginatedPostsResponse>(&response_json) {
                Ok(posts_response) => Ok(Json(posts_response)),
                Err(err) => {
                    log_error!("Failed to parse paginated posts response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" | "INVALID_LIMIT" => {
                            StatusCode::BAD_REQUEST
                        }
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_post_details(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetPostDetailsQuery>,
) -> Result<Json<PostDetailsResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;
    // Check if id parameter is provided
    let post_id = match params.id {
        Some(id) => id,
        None => {
            let error = ApiError {
                error: "Missing required parameter: id".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get post details with voting information and blocking status
    match app_state
        .api_handlers
        .get_post_details(&post_id, &requester_pubkey)
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PostDetailsResponse
            match serde_json::from_str::<PostDetailsResponse>(&response_json) {
                Ok(post_details_response) => Ok(Json(post_details_response)),
                Err(err) => {
                    log_error!("Failed to parse post details response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_POST_ID" => StatusCode::BAD_REQUEST,
                        "NOT_FOUND" => StatusCode::NOT_FOUND,
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_mentions(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetMentionsQuery>,
) -> Result<Json<PaginatedPostsResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;
    // Check if user parameter is provided
    let user_public_key = match params.user {
        Some(user) => user,
        None => {
            let error = ApiError {
                error: "Missing required parameter: user".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated mentions for the user with voting status
    match app_state
        .api_handlers
        .get_mentions_paginated(
            &user_public_key,
            &requester_pubkey,
            limit,
            params.before,
            params.after,
        )
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedPostsResponse
            match serde_json::from_str::<PaginatedPostsResponse>(&response_json) {
                Ok(mentions_response) => Ok(Json(mentions_response)),
                Err(err) => {
                    log_error!("Failed to parse paginated mentions response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" | "INVALID_LIMIT" => {
                            StatusCode::BAD_REQUEST
                        }
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_notifications(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetNotificationsQuery>,
) -> Result<Json<PaginatedNotificationsResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated notifications for the user
    match app_state
        .api_handlers
        .get_notifications_paginated(&requester_pubkey, limit, params.before, params.after)
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedNotificationsResponse
            match serde_json::from_str::<PaginatedNotificationsResponse>(&response_json) {
                Ok(notifications_response) => Ok(Json(notifications_response)),
                Err(err) => {
                    log_error!("Failed to parse paginated notifications response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" | "INVALID_LIMIT" => {
                            StatusCode::BAD_REQUEST
                        }
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_hashtag_content(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetHashtagContentQuery>,
) -> Result<Json<PaginatedPostsResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Check if hashtag parameter is provided
    let hashtag = match params.hashtag {
        Some(tag) => tag.to_lowercase(), // Normalize to lowercase
        None => {
            let error = ApiError {
                error: "Missing required parameter: hashtag".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate hashtag length (max 30 characters, without #)
    if hashtag.is_empty() {
        let error = ApiError {
            error: "Hashtag parameter cannot be empty".to_string(),
            code: "INVALID_PARAMETER".to_string(),
        };
        return Err((StatusCode::BAD_REQUEST, Json(error)));
    }

    if hashtag.len() > 30 {
        let error = ApiError {
            error: "Hashtag parameter cannot exceed 30 characters".to_string(),
            code: "INVALID_PARAMETER".to_string(),
        };
        return Err((StatusCode::BAD_REQUEST, Json(error)));
    }

    // Limit defaults to 20 if not provided
    let limit = params.limit.unwrap_or(20);

    // Validate limit parameter
    if limit < 1 || limit > 100 {
        let error = ApiError {
            error: "Limit parameter must be between 1 and 100".to_string(),
            code: "INVALID_LIMIT".to_string(),
        };
        return Err((StatusCode::BAD_REQUEST, Json(error)));
    }

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated hashtag content
    match app_state
        .api_handlers
        .get_hashtag_content_paginated(
            &hashtag,
            &requester_pubkey,
            limit,
            params.before,
            params.after,
        )
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedPostsResponse
            match serde_json::from_str::<PaginatedPostsResponse>(&response_json) {
                Ok(posts_response) => Ok(Json(posts_response)),
                Err(err) => {
                    log_error!(
                        "Failed to parse paginated hashtag content response: {}",
                        err
                    );
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_PARAMETER" | "INVALID_LIMIT" => {
                            StatusCode::BAD_REQUEST
                        }
                        "NOT_FOUND" => StatusCode::NOT_FOUND,
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_users(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetUsersQuery>,
) -> Result<Json<PaginatedUsersResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;
    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated user introduction posts with block status
    match app_state
        .api_handlers
        .get_users_paginated(limit, &requester_pubkey, params.before, params.after)
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedUsersResponse
            match serde_json::from_str::<PaginatedUsersResponse>(&response_json) {
                Ok(users_response) => Ok(Json(users_response)),
                Err(err) => {
                    log_error!("Failed to parse paginated users response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "DATABASE_ERROR" | "SERIALIZATION_ERROR" => {
                            StatusCode::INTERNAL_SERVER_ERROR
                        }
                        "MISSING_PARAMETER" | "INVALID_LIMIT" => StatusCode::BAD_REQUEST,
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_most_active_users(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetMostActiveUsersQuery>,
) -> Result<Json<PaginatedUsersResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Parse and validate time_window parameter (required)
    let time_window = match params.time_window {
        Some(tw) => tw,
        None => {
            let error = ApiError {
                error: "Missing required parameter: timeWindow".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    let valid_windows = ["1h", "6h", "24h", "7d", "30d"];
    if !valid_windows.contains(&time_window.as_str()) {
        let error = ApiError {
            error: format!(
                "Invalid timeWindow parameter. Must be one of: {}",
                valid_windows.join(", ")
            ),
            code: "INVALID_PARAMETER".to_string(),
        };
        return Err((StatusCode::BAD_REQUEST, Json(error)));
    }

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get most active users ranked by content count
    match app_state
        .api_handlers
        .get_most_active_users_paginated(
            limit,
            &requester_pubkey,
            &time_window,
            params.before,
            params.after,
        )
        .await
    {
        Ok(response_json) => match serde_json::from_str::<PaginatedUsersResponse>(&response_json) {
            Ok(users_response) => Ok(Json(users_response)),
            Err(err) => {
                log_error!("Failed to parse most active users response: {}", err);
                let error = ApiError {
                    error: "Internal server error".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                };
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
            }
        },
        Err(error_json) => match serde_json::from_str::<ApiError>(&error_json) {
            Ok(api_error) => {
                let status_code = match api_error.code.as_str() {
                    "DATABASE_ERROR" | "SERIALIZATION_ERROR" => StatusCode::INTERNAL_SERVER_ERROR,
                    "MISSING_PARAMETER" | "INVALID_LIMIT" | "INVALID_PARAMETER" => {
                        StatusCode::BAD_REQUEST
                    }
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                Err((status_code, Json(api_error)))
            }
            Err(_) => {
                let error = ApiError {
                    error: "Internal server error".to_string(),
                    code: "INTERNAL_ERROR".to_string(),
                };
                Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
            }
        },
    }
}

async fn handle_search_users(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<SearchUsersQuery>,
) -> Result<Json<PaginatedUsersResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to search users
    match app_state
        .api_handlers
        .search_users_paginated(
            limit,
            &requester_pubkey,
            params.before,
            params.after,
            params.searched_user_pubkey,
            params.searched_user_nickname,
        )
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedUsersResponse
            match serde_json::from_str::<PaginatedUsersResponse>(&response_json) {
                Ok(users_response) => Ok(Json(users_response)),
                Err(err) => {
                    log_error!("Failed to parse search users response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "DATABASE_ERROR" | "SERIALIZATION_ERROR" => {
                            StatusCode::INTERNAL_SERVER_ERROR
                        }
                        "MISSING_PARAMETER" | "INVALID_LIMIT" | "INVALID_USER_KEY" => {
                            StatusCode::BAD_REQUEST
                        }
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_posts_watching(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetPostsWatchingQuery>,
) -> Result<Json<PaginatedPostsResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;
    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated posts for watching with voting status
    match app_state
        .api_handlers
        .get_posts_watching_paginated(&requester_pubkey, limit, params.before, params.after)
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedPostsResponse
            match serde_json::from_str::<PaginatedPostsResponse>(&response_json) {
                Ok(posts_response) => Ok(Json(posts_response)),
                Err(err) => {
                    log_error!("Failed to parse paginated posts response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "DATABASE_ERROR" | "SERIALIZATION_ERROR" => {
                            StatusCode::INTERNAL_SERVER_ERROR
                        }
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" | "INVALID_LIMIT" => {
                            StatusCode::BAD_REQUEST
                        }
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_contents_following(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetContentsFollowingQuery>,
) -> Result<Json<PaginatedPostsResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;
    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated content from followed users
    match app_state
        .api_handlers
        .get_content_following_paginated(&requester_pubkey, limit, params.before, params.after)
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedPostsResponse
            match serde_json::from_str::<PaginatedPostsResponse>(&response_json) {
                Ok(posts_response) => Ok(Json(posts_response)),
                Err(err) => {
                    log_error!("Failed to parse paginated content response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "DATABASE_ERROR" | "SERIALIZATION_ERROR" => {
                            StatusCode::INTERNAL_SERVER_ERROR
                        }
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" | "INVALID_LIMIT" => {
                            StatusCode::BAD_REQUEST
                        }
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_replies(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetRepliesQuery>,
) -> Result<Json<PaginatedRepliesResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;
    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Check if exactly one of post or user parameter is provided
    match (params.post.as_ref(), params.user.as_ref()) {
        (Some(post_id), None) => {
            // Post replies mode: get replies to a specific post
            match app_state
                .api_handlers
                .get_replies_paginated(
                    post_id,
                    &requester_pubkey,
                    limit,
                    params.before,
                    params.after,
                )
                .await
            {
                Ok(response_json) => {
                    match serde_json::from_str::<PaginatedRepliesResponse>(&response_json) {
                        Ok(replies_response) => Ok(Json(replies_response)),
                        Err(err) => {
                            log_error!("Failed to parse paginated replies response: {}", err);
                            let error = ApiError {
                                error: "Internal server error".to_string(),
                                code: "INTERNAL_ERROR".to_string(),
                            };
                            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                        }
                    }
                }
                Err(error_json) => match serde_json::from_str::<ApiError>(&error_json) {
                    Ok(api_error) => {
                        let status_code = match api_error.code.as_str() {
                            "MISSING_PARAMETER" | "INVALID_POST_ID" | "INVALID_USER_KEY"
                            | "INVALID_LIMIT" => StatusCode::BAD_REQUEST,
                            _ => StatusCode::INTERNAL_SERVER_ERROR,
                        };
                        Err((status_code, Json(api_error)))
                    }
                    Err(_) => {
                        let error = ApiError {
                            error: "Internal server error".to_string(),
                            code: "INTERNAL_ERROR".to_string(),
                        };
                        Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                    }
                },
            }
        }
        (None, Some(user_public_key)) => {
            // User replies mode: get all replies made by a specific user
            match app_state
                .api_handlers
                .get_user_replies_paginated(
                    user_public_key,
                    &requester_pubkey,
                    limit,
                    params.before,
                    params.after,
                )
                .await
            {
                Ok(response_json) => {
                    match serde_json::from_str::<PaginatedRepliesResponse>(&response_json) {
                        Ok(replies_response) => Ok(Json(replies_response)),
                        Err(err) => {
                            log_error!("Failed to parse paginated user replies response: {}", err);
                            let error = ApiError {
                                error: "Internal server error".to_string(),
                                code: "INTERNAL_ERROR".to_string(),
                            };
                            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                        }
                    }
                }
                Err(error_json) => match serde_json::from_str::<ApiError>(&error_json) {
                    Ok(api_error) => {
                        let status_code = match api_error.code.as_str() {
                            "MISSING_PARAMETER" | "INVALID_USER_KEY" | "INVALID_LIMIT" => {
                                StatusCode::BAD_REQUEST
                            }
                            _ => StatusCode::INTERNAL_SERVER_ERROR,
                        };
                        Err((status_code, Json(api_error)))
                    }
                    Err(_) => {
                        let error = ApiError {
                            error: "Internal server error".to_string(),
                            code: "INTERNAL_ERROR".to_string(),
                        };
                        Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                    }
                },
            }
        }
        (Some(_), Some(_)) => {
            // Both parameters provided - not allowed
            let error = ApiError {
                error: "Cannot provide both 'post' and 'user' parameters. Use 'post' for post replies or 'user' for user replies.".to_string(),
                code: "INVALID_PARAMETERS".to_string(),
            };
            Err((StatusCode::BAD_REQUEST, Json(error)))
        }
        (None, None) => {
            // Neither parameter provided - not allowed
            let error = ApiError {
                error: "Missing required parameter: either 'post' or 'user' must be provided"
                    .to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            Err((StatusCode::BAD_REQUEST, Json(error)))
        }
    }
}

async fn handle_get_user_details(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetUserDetailsQuery>,
) -> Result<Json<ServerUserPost>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Check if user parameter is provided
    let user_public_key = match params.user {
        Some(user) => user,
        None => {
            let error = ApiError {
                error: "Missing required parameter: user".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get user details
    match app_state
        .api_handlers
        .get_user_details(&user_public_key, &requester_pubkey)
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to ServerUserPost
            match serde_json::from_str::<ServerUserPost>(&response_json) {
                Ok(user_details_response) => Ok(Json(user_details_response)),
                Err(err) => {
                    log_error!("Failed to parse user details response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" => StatusCode::BAD_REQUEST,
                        "USER_NOT_FOUND" => StatusCode::NOT_FOUND,
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_blocked_users(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetBlockedUsersQuery>,
) -> Result<Json<PaginatedUsersResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated blocked users
    match app_state
        .api_handlers
        .get_blocked_users_paginated(&requester_pubkey, limit, params.before, params.after)
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedUsersResponse
            match serde_json::from_str::<PaginatedUsersResponse>(&response_json) {
                Ok(users_response) => Ok(Json(users_response)),
                Err(err) => {
                    log_error!("Failed to parse paginated blocked users response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" | "INVALID_LIMIT" => {
                            StatusCode::BAD_REQUEST
                        }
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_followed_users(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetFollowedUsersQuery>,
) -> Result<Json<PaginatedUsersResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated followed users
    match app_state
        .api_handlers
        .get_followed_users_paginated(&requester_pubkey, limit, params.before, params.after)
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedUsersResponse
            match serde_json::from_str::<PaginatedUsersResponse>(&response_json) {
                Ok(users_response) => Ok(Json(users_response)),
                Err(err) => {
                    log_error!("Failed to parse paginated followed users response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" | "INVALID_LIMIT" => {
                            StatusCode::BAD_REQUEST
                        }
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_users_following(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetUsersFollowingQuery>,
) -> Result<Json<PaginatedUsersResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Check if userPubkey parameter is provided
    let user_pubkey = match params.user_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: userPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated users following
    match app_state
        .api_handlers
        .get_users_following_paginated(
            &requester_pubkey,
            &user_pubkey,
            limit,
            params.before,
            params.after,
        )
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedUsersResponse
            match serde_json::from_str::<PaginatedUsersResponse>(&response_json) {
                Ok(users_response) => Ok(Json(users_response)),
                Err(err) => {
                    log_error!(
                        "Failed to parse paginated users following response: {}",
                        err
                    );
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" | "INVALID_LIMIT" => {
                            StatusCode::BAD_REQUEST
                        }
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_users_followers(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetUsersFollowersQuery>,
) -> Result<Json<PaginatedUsersResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Check if userPubkey parameter is provided
    let user_pubkey = match params.user_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: userPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Validate required limit parameter
    let limit = match params.limit {
        Some(limit) => {
            if limit < 1 || limit > 100 {
                let error = ApiError {
                    error: "Limit parameter must be between 1 and 100".to_string(),
                    code: "INVALID_LIMIT".to_string(),
                };
                return Err((StatusCode::BAD_REQUEST, Json(error)));
            }
            limit
        }
        None => {
            let error = ApiError {
                error: "Missing required parameter: limit".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get paginated users followers
    match app_state
        .api_handlers
        .get_users_followers_paginated(
            &requester_pubkey,
            &user_pubkey,
            limit,
            params.before,
            params.after,
        )
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to PaginatedUsersResponse
            match serde_json::from_str::<PaginatedUsersResponse>(&response_json) {
                Ok(users_response) => Ok(Json(users_response)),
                Err(err) => {
                    log_error!(
                        "Failed to parse paginated users followers response: {}",
                        err
                    );
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" | "INVALID_LIMIT" => {
                            StatusCode::BAD_REQUEST
                        }
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_notifications_count(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetNotificationsCountQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Check if requesterPubkey parameter is provided
    let requester_pubkey = match params.requester_pubkey {
        Some(pubkey) => pubkey,
        None => {
            let error = ApiError {
                error: "Missing required parameter: requesterPubkey".to_string(),
                code: "MISSING_PARAMETER".to_string(),
            };
            return Err((StatusCode::BAD_REQUEST, Json(error)));
        }
    };

    // Use the API handler to get notification count
    match app_state
        .api_handlers
        .get_notification_count(&requester_pubkey, params.after)
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to a generic JSON value
            match serde_json::from_str::<serde_json::Value>(&response_json) {
                Ok(response) => Ok(Json(response)),
                Err(err) => {
                    log_error!("Failed to parse notification count response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "MISSING_PARAMETER" | "INVALID_USER_KEY" => StatusCode::BAD_REQUEST,
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_users_count(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(_params): Query<GetUsersCountQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Use the API handler to get users count
    match app_state.api_handlers.get_users_count().await {
        Ok(response_json) => {
            // Parse the JSON response back to a generic JSON value
            match serde_json::from_str::<serde_json::Value>(&response_json) {
                Ok(response) => Ok(Json(response)),
                Err(err) => {
                    log_error!("Failed to parse users count response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}

async fn handle_get_trending_hashtags(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(app_state): State<Arc<AppState>>,
    Query(params): Query<GetTrendingHashtagsQuery>,
) -> Result<Json<TrendingHashtagsResponse>, (StatusCode, Json<ApiError>)> {
    // Check rate limit first
    check_rate_limit(&app_state, addr).await?;

    // Parse and validate time_window parameter (default: "24h")
    let time_window = params.time_window.unwrap_or_else(|| "24h".to_string());

    // Validate time_window values
    let valid_windows = ["1h", "6h", "24h", "7d", "30d"];
    if !valid_windows.contains(&time_window.as_str()) {
        let error = ApiError {
            error: format!(
                "Invalid timeWindow parameter. Must be one of: {}",
                valid_windows.join(", ")
            ),
            code: "INVALID_PARAMETER".to_string(),
        };
        return Err((StatusCode::BAD_REQUEST, Json(error)));
    }

    // Limit defaults to 20 if not provided
    let limit = params.limit.unwrap_or(20);

    // Validate limit parameter
    if limit < 1 || limit > 100 {
        let error = ApiError {
            error: "Limit parameter must be between 1 and 100".to_string(),
            code: "INVALID_LIMIT".to_string(),
        };
        return Err((StatusCode::BAD_REQUEST, Json(error)));
    }

    // Use the API handler to get trending hashtags
    match app_state
        .api_handlers
        .get_trending_hashtags(&time_window, limit)
        .await
    {
        Ok(response_json) => {
            // Parse the JSON response back to TrendingHashtagsResponse
            match serde_json::from_str::<TrendingHashtagsResponse>(&response_json) {
                Ok(response) => Ok(Json(response)),
                Err(err) => {
                    log_error!("Failed to parse trending hashtags response: {}", err);
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
        Err(error_json) => {
            // Parse the error response
            match serde_json::from_str::<ApiError>(&error_json) {
                Ok(api_error) => {
                    let status_code = match api_error.code.as_str() {
                        "INVALID_PARAMETER" | "INVALID_LIMIT" => StatusCode::BAD_REQUEST,
                        _ => StatusCode::INTERNAL_SERVER_ERROR,
                    };
                    Err((status_code, Json(api_error)))
                }
                Err(_) => {
                    let error = ApiError {
                        error: "Internal server error".to_string(),
                        code: "INTERNAL_ERROR".to_string(),
                    };
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error)))
                }
            }
        }
    }
}
