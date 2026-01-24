use axum::{
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
    Router,
};
use rust_embed::RustEmbed;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use super::{auth, events, incidents, ingest, metrics, patterns, projects, rate_limit, settings, webhooks, AppState};

#[derive(RustEmbed)]
#[folder = "ui/build"]
struct UiAssets;

/// Build the main application router
pub fn build(state: AppState) -> Router {
    // Create rate limiter for ingest endpoint
    let ingest_limiter = rate_limit::create_rate_limiter(state.config.ingest.rate_limit_per_second);
    tracing::info!(
        "Ingest rate limit configured: {} requests/second",
        state.config.ingest.rate_limit_per_second
    );

    // Ingest endpoint with rate limiting
    let ingest_route = Router::new()
        .route("/ingest", post(ingest::http_ingest))
        .layer(rate_limit::RateLimitLayer::new(ingest_limiter.clone()))
        .with_state(state.clone());

    // Webhook endpoints with rate limiting
    let webhook_routes = Router::new()
        .route("/webhooks/sentry", post(webhooks::sentry_webhook))
        .route("/webhooks/datadog", post(webhooks::datadog_webhook))
        .route("/webhooks/grafana", post(webhooks::grafana_webhook))
        .layer(rate_limit::RateLimitLayer::new(ingest_limiter))
        .with_state(state.clone());

    let api = Router::new()
        // Health checks
        .route("/health", get(health))
        .route("/ready", get(ready))
        // Metrics
        .route("/metrics", get(metrics::handler))
        // Ingest endpoint (merged separately with rate limiting)
        // Auth endpoints
        .route("/api/v1/auth/status", get(auth::status))
        .route("/api/v1/auth/github", get(auth::github_redirect).delete(auth::github_disconnect))
        .route("/api/v1/auth/github/callback", get(auth::github_callback))
        .route("/api/v1/auth/slack", get(auth::slack_redirect).delete(auth::slack_disconnect))
        .route("/api/v1/auth/slack/callback", get(auth::slack_callback))
        .route("/api/v1/auth/slack/channels", get(auth::slack_channels))
        .route("/api/v1/auth/slack/channel", post(auth::slack_set_channel))
        .route("/api/v1/auth/slack/test", post(auth::slack_test))
        // Projects endpoints
        .route("/api/v1/projects", get(projects::list).post(projects::create))
        .route(
            "/api/v1/projects/:id",
            get(projects::get)
                .patch(projects::update)
                .delete(projects::delete),
        )
        .route("/api/v1/projects/:id/stats", get(projects::stats))
        // Incidents endpoints
        .route("/api/v1/incidents", get(incidents::list))
        .route("/api/v1/incidents/stats", get(incidents::stats))
        .route("/api/v1/incidents/:id", get(incidents::get))
        .route("/api/v1/incidents/:id/retry", post(incidents::retry))
        .route("/api/v1/incidents/:id/ignore", post(incidents::ignore))
        // Generated tests endpoints
        .route("/api/v1/incidents/:id/tests", get(incidents::list_tests))
        .route(
            "/api/v1/incidents/:id/tests/:test_id",
            get(incidents::get_test).patch(incidents::update_test),
        )
        // Pattern matches for incidents
        .route("/api/v1/incidents/:id/pattern-matches", get(patterns::incident_matches))
        // Patterns endpoints
        .route("/api/v1/patterns", get(patterns::list).post(patterns::create))
        .route("/api/v1/patterns/summary", get(patterns::summary))
        .route("/api/v1/patterns/categories", get(patterns::list_categories))
        .route("/api/v1/patterns/test", post(patterns::test_pattern))
        .route("/api/v1/patterns/reload", post(patterns::reload))
        .route(
            "/api/v1/patterns/:id",
            get(patterns::get)
                .patch(patterns::update)
                .delete(patterns::delete),
        )
        .route("/api/v1/patterns/:id/stats", get(patterns::stats))
        // SSE events endpoint
        .route("/api/v1/events", get(events::handler))
        // Settings endpoints
        .route(
            "/api/v1/settings/llm",
            get(settings::get_llm).put(settings::update_llm),
        )
        .route("/api/v1/settings/llm/verify", post(settings::verify_llm))
        .route("/api/v1/settings/llm/models", post(settings::list_models))
        // Usage endpoints
        .route("/api/v1/usage/summary", get(settings::usage_summary))
        .route("/api/v1/usage/details", get(settings::usage_details))
        // Webhook settings
        .route(
            "/api/v1/settings/webhooks",
            get(settings::get_webhooks).put(settings::update_webhooks),
        )
        // Status endpoint
        .route("/api/v1/status", get(api_status))
        .with_state(state.clone());

    // Static UI assets with SPA fallback
    let ui = Router::new().fallback(serve_ui);

    Router::new()
        .merge(api)
        .merge(ingest_route)
        .merge(webhook_routes)
        .merge(ui)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

/// Health check - always returns OK if server is running
async fn health() -> &'static str {
    "OK"
}

/// Readiness check - returns OK if database is connected
async fn ready(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<&'static str, StatusCode> {
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    Ok("OK")
}

/// API status endpoint
async fn api_status(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "environment": state.config.environment,
        "display_name": state.config.display_name,
        "llm_configured": state.llm.is_configured().await,
        "github_connected": state.github.is_connected().await,
    }))
}

/// Serve embedded UI assets with SPA fallback
async fn serve_ui(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match UiAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => {
            // SPA fallback - serve index.html for all unknown routes
            match UiAssets::get("index.html") {
                Some(content) => (
                    [(header::CONTENT_TYPE, "text/html")],
                    content.data.into_owned(),
                )
                    .into_response(),
                None => {
                    // No UI built yet - return a simple dashboard
                    (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "text/html")],
                        include_str!("../../templates/placeholder.html"),
                    )
                        .into_response()
                }
            }
        }
    }
}
