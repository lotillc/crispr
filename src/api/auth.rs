//! Auth API endpoints
//!
//! GitHub and Slack OAuth flows and connection status.

use axum::{
    extract::{Host, Query, State},
    http::header,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use crate::error::AppError;

// ============================================================================
// Connection Status
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ConnectionStatus {
    pub github: GitHubStatus,
    pub slack: SlackStatus,
    pub llm: LlmStatus,
}

#[derive(Debug, Serialize)]
pub struct GitHubStatus {
    pub connected: bool,
    pub mode: String,
    pub user: Option<GitHubUserInfo>,
    pub oauth_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GitHubUserInfo {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: String,
}

#[derive(Debug, Serialize)]
pub struct SlackStatus {
    pub enabled: bool,
    pub connected: bool,
    pub team_name: Option<String>,
    pub channel_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LlmStatus {
    pub configured: bool,
    pub reader_model: Option<String>,
    pub writer_model: Option<String>,
}

/// GET /api/v1/auth/status - Get connection status for all integrations
pub async fn status(State(state): State<AppState>) -> Result<Json<ConnectionStatus>, AppError> {
    // GitHub status
    let github_connected = state.github.is_connected().await;
    let github_user = if github_connected {
        state.github.get_user().await.ok().map(|u| GitHubUserInfo {
            login: u.login,
            name: u.name,
            avatar_url: u.avatar_url,
        })
    } else {
        None
    };

    // Generate OAuth URL if in oauth mode and not connected
    let oauth_url = if state.config.github.mode == "oauth" && !github_connected {
        state.config.github.client_id.as_ref().map(|_| "/api/v1/auth/github".to_string())
    } else {
        None
    };

    // Slack status
    let slack_info = sqlx::query!(
        "SELECT team_name, channel_name FROM slack_connections ORDER BY created_at DESC LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await?;

    // LLM status
    let llm_config = sqlx::query!(
        "SELECT reader_model, writer_model FROM llm_config ORDER BY created_at DESC LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(ConnectionStatus {
        github: GitHubStatus {
            connected: github_connected,
            mode: state.config.github.mode.clone(),
            user: github_user,
            oauth_url,
        },
        slack: SlackStatus {
            enabled: state.config.slack.enabled,
            connected: slack_info.is_some(),
            team_name: slack_info.as_ref().and_then(|s| s.team_name.clone()),
            channel_name: slack_info.and_then(|s| s.channel_name),
        },
        llm: LlmStatus {
            configured: state.llm.is_configured().await,
            reader_model: llm_config.as_ref().map(|c| c.reader_model.clone()),
            writer_model: llm_config.map(|c| c.writer_model),
        },
    }))
}

// ============================================================================
// GitHub OAuth
// ============================================================================

/// GET /api/v1/auth/github - Redirect to GitHub OAuth
pub async fn github_redirect(
    State(state): State<AppState>,
    Host(host): Host,
) -> Result<Redirect, AppError> {
    let client_id = state
        .config
        .github
        .client_id
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("GitHub OAuth not configured. Set GITHUB_CLIENT_ID and GITHUB_CLIENT_SECRET.".to_string()))?;

    // Generate state parameter for CSRF protection
    let oauth_state = Uuid::new_v4().to_string();

    // Store state in database with expiry
    sqlx::query!(
        r#"
        INSERT INTO oauth_states (state, provider, created_at, expires_at)
        VALUES ($1, 'github', NOW(), NOW() + INTERVAL '10 minutes')
        "#,
        &oauth_state
    )
    .execute(&state.db)
    .await?;

    // Build callback URL - use the host from the request
    let scheme = if host.contains("localhost") || host.contains("127.0.0.1") {
        "http"
    } else {
        "https"
    };
    let callback_url = format!("{}://{}/api/v1/auth/github/callback", scheme, host);
    let encoded_callback = urlencoding::encode(&callback_url);

    // GitHub OAuth URL with all required parameters
    let redirect_uri = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=repo&state={}",
        client_id, encoded_callback, oauth_state
    );

    tracing::info!(callback_url = %callback_url, "Redirecting to GitHub OAuth");

    Ok(Redirect::temporary(&redirect_uri))
}

#[derive(Debug, Deserialize)]
pub struct GitHubCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// GET /api/v1/auth/github/callback - Handle GitHub OAuth callback
pub async fn github_callback(
    State(state): State<AppState>,
    Host(host): Host,
    Query(query): Query<GitHubCallbackQuery>,
) -> Result<Response, AppError> {
    let scheme = if host.contains("localhost") || host.contains("127.0.0.1") {
        "http"
    } else {
        "https"
    };
    let base_url = format!("{}://{}", scheme, host);

    // Check for OAuth errors from GitHub
    if let Some(error) = &query.error {
        let description = query.error_description.as_deref().unwrap_or("Unknown error");
        tracing::warn!(error = %error, description = %description, "GitHub OAuth error");
        return Ok(Redirect::temporary(&format!(
            "{}/settings?github_error={}",
            base_url,
            urlencoding::encode(description)
        )).into_response());
    }

    // Validate required parameters
    let code = query.code.as_ref().ok_or_else(|| {
        AppError::BadRequest("Missing authorization code".to_string())
    })?;
    let oauth_state = query.state.as_ref().ok_or_else(|| {
        AppError::BadRequest("Missing state parameter".to_string())
    })?;

    // Validate state (CSRF protection)
    let valid_state = sqlx::query!(
        r#"
        DELETE FROM oauth_states
        WHERE state = $1 AND provider = 'github' AND expires_at > NOW()
        RETURNING state
        "#,
        oauth_state
    )
    .fetch_optional(&state.db)
    .await?;

    if valid_state.is_none() {
        tracing::warn!(state = %oauth_state, "Invalid or expired OAuth state");
        return Ok(Redirect::temporary(&format!(
            "{}/settings?github_error={}",
            base_url,
            urlencoding::encode("Invalid or expired authorization. Please try again.")
        )).into_response());
    }

    // Get client credentials
    let client_id = state
        .config
        .github
        .client_id
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("GitHub OAuth not configured".to_string()))?;
    let client_secret = state
        .config
        .github
        .client_secret
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("GitHub OAuth not configured".to_string()))?;

    // Exchange code for token
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code.as_str()),
        ])
        .send()
        .await?;

    if !token_response.status().is_success() {
        let error_text = token_response.text().await.unwrap_or_default();
        tracing::error!(error = %error_text, "Failed to exchange code for token");
        return Ok(Redirect::temporary(&format!(
            "{}/settings?github_error={}",
            base_url,
            urlencoding::encode("Failed to complete authorization. Please try again.")
        )).into_response());
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: Option<String>,
        token_type: Option<String>,
        scope: Option<String>,
        error: Option<String>,
        error_description: Option<String>,
    }

    let token: TokenResponse = token_response.json().await?;

    // Check for token error
    if let Some(error) = token.error {
        let description = token.error_description.unwrap_or(error);
        tracing::error!(error = %description, "GitHub token error");
        return Ok(Redirect::temporary(&format!(
            "{}/settings?github_error={}",
            base_url,
            urlencoding::encode(&description)
        )).into_response());
    }

    let access_token = token.access_token.ok_or_else(|| {
        AppError::Internal("No access token in response".to_string())
    })?;
    let scope = token.scope.unwrap_or_default();

    // Get user info
    let user_response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "CRISPR/1.0")
        .send()
        .await?;

    #[derive(Deserialize)]
    struct UserResponse {
        login: String,
        id: i64,
        name: Option<String>,
        avatar_url: String,
    }

    let user: UserResponse = user_response.json().await?;
    let user_login = user.login.clone();

    // Save to database (upsert - replace any existing connection)
    let scopes: Vec<String> = scope.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    // Delete existing connections first
    sqlx::query!("DELETE FROM github_connections")
        .execute(&state.db)
        .await?;

    // Insert new connection
    sqlx::query!(
        r#"
        INSERT INTO github_connections (id, mode, access_token, user_login, user_id, scopes)
        VALUES ($1, 'oauth', $2, $3, $4, $5)
        "#,
        Uuid::new_v4(),
        access_token,
        user.login,
        user.id,
        &scopes,
    )
    .execute(&state.db)
    .await?;

    // Reinitialize GitHub client
    state.github.init().await?;

    tracing::info!(user = %user_login, "GitHub OAuth completed successfully");

    // Redirect to settings page with success message
    Ok(Redirect::temporary(&format!(
        "{}/settings?github_success={}",
        base_url,
        urlencoding::encode(&format!("Connected as {}", user_login))
    )).into_response())
}

#[derive(Debug, Serialize)]
pub struct OAuthResult {
    pub success: bool,
    pub message: String,
}

/// DELETE /api/v1/auth/github - Disconnect GitHub
pub async fn github_disconnect(
    State(state): State<AppState>,
) -> Result<Json<OAuthResult>, AppError> {
    // Delete all GitHub connections
    let result = sqlx::query!("DELETE FROM github_connections")
        .execute(&state.db)
        .await?;

    if result.rows_affected() > 0 {
        // Clear the token from the client
        state.github.init().await?;

        tracing::info!("GitHub disconnected");
        Ok(Json(OAuthResult {
            success: true,
            message: "GitHub disconnected".to_string(),
        }))
    } else {
        Ok(Json(OAuthResult {
            success: false,
            message: "No GitHub connection to disconnect".to_string(),
        }))
    }
}

// ============================================================================
// Slack OAuth
// ============================================================================

/// GET /api/v1/auth/slack - Redirect to Slack OAuth
pub async fn slack_redirect(
    State(state): State<AppState>,
    Host(host): Host,
) -> Result<Redirect, AppError> {
    if !state.config.slack.enabled {
        return Err(AppError::BadRequest("Slack not enabled".to_string()));
    }

    let client_id = state
        .config
        .slack
        .client_id
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Slack OAuth not configured".to_string()))?;

    let oauth_state = Uuid::new_v4().to_string();

    // Store state in database
    sqlx::query!(
        r#"
        INSERT INTO oauth_states (state, provider, created_at, expires_at)
        VALUES ($1, 'slack', NOW(), NOW() + INTERVAL '10 minutes')
        "#,
        &oauth_state
    )
    .execute(&state.db)
    .await?;

    // Build callback URL
    let scheme = if host.contains("localhost") || host.contains("127.0.0.1") {
        "http"
    } else {
        "https"
    };
    let callback_url = format!("{}://{}/api/v1/auth/slack/callback", scheme, host);
    let encoded_callback = urlencoding::encode(&callback_url);

    let redirect_uri = format!(
        "https://slack.com/oauth/v2/authorize?client_id={}&redirect_uri={}&scope=chat:write,channels:read&state={}",
        client_id, encoded_callback, oauth_state
    );

    Ok(Redirect::temporary(&redirect_uri))
}

#[derive(Debug, Deserialize)]
pub struct SlackCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// GET /api/v1/auth/slack/callback - Handle Slack OAuth callback
pub async fn slack_callback(
    State(state): State<AppState>,
    Host(host): Host,
    Query(query): Query<SlackCallbackQuery>,
) -> Result<Response, AppError> {
    let scheme = if host.contains("localhost") || host.contains("127.0.0.1") {
        "http"
    } else {
        "https"
    };
    let base_url = format!("{}://{}", scheme, host);

    // Check for OAuth errors
    if let Some(error) = &query.error {
        return Ok(Redirect::temporary(&format!(
            "{}/settings?slack_error={}",
            base_url,
            urlencoding::encode(error)
        )).into_response());
    }

    let code = query.code.as_ref().ok_or_else(|| {
        AppError::BadRequest("Missing authorization code".to_string())
    })?;
    let oauth_state = query.state.as_ref().ok_or_else(|| {
        AppError::BadRequest("Missing state parameter".to_string())
    })?;

    // Validate state
    let valid_state = sqlx::query!(
        r#"
        DELETE FROM oauth_states
        WHERE state = $1 AND provider = 'slack' AND expires_at > NOW()
        RETURNING state
        "#,
        oauth_state
    )
    .fetch_optional(&state.db)
    .await?;

    if valid_state.is_none() {
        return Ok(Redirect::temporary(&format!(
            "{}/settings?slack_error={}",
            base_url,
            urlencoding::encode("Invalid or expired authorization. Please try again.")
        )).into_response());
    }

    let client_id = state
        .config
        .slack
        .client_id
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Slack OAuth not configured".to_string()))?;
    let client_secret = state
        .config
        .slack
        .client_secret
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Slack OAuth not configured".to_string()))?;

    // Exchange code for token
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://slack.com/api/oauth.v2.access")
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code.as_str()),
        ])
        .send()
        .await?;

    #[derive(Deserialize)]
    struct SlackTokenResponse {
        ok: bool,
        access_token: Option<String>,
        team: Option<SlackTeam>,
        bot_user_id: Option<String>,
        error: Option<String>,
    }

    #[derive(Deserialize)]
    struct SlackTeam {
        id: String,
        name: String,
    }

    let token: SlackTokenResponse = token_response.json().await?;

    if !token.ok {
        let error = token.error.unwrap_or_else(|| "Unknown error".to_string());
        return Ok(Redirect::temporary(&format!(
            "{}/settings?slack_error={}",
            base_url,
            urlencoding::encode(&error)
        )).into_response());
    }

    let access_token = token.access_token.ok_or_else(|| {
        AppError::Internal("No access token in response".to_string())
    })?;
    let team = token.team.ok_or_else(|| {
        AppError::Internal("No team in response".to_string())
    })?;

    // Delete existing connections and save new one
    sqlx::query!("DELETE FROM slack_connections")
        .execute(&state.db)
        .await?;

    sqlx::query!(
        r#"
        INSERT INTO slack_connections (id, access_token, team_id, team_name, bot_user_id)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        Uuid::new_v4(),
        access_token,
        team.id,
        team.name,
        token.bot_user_id,
    )
    .execute(&state.db)
    .await?;

    // Redirect to settings page with success
    Ok(Redirect::temporary(&format!(
        "{}/settings?slack_success={}",
        base_url,
        urlencoding::encode(&format!("Connected to {}", team.name))
    )).into_response())
}

// ============================================================================
// Slack Channel Management
// ============================================================================

/// GET /api/v1/auth/slack/channels - List available Slack channels
pub async fn slack_channels(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::integrations::slack::SlackChannel>>, AppError> {
    let channels = state.slack.list_channels().await?;
    Ok(Json(channels))
}

#[derive(Debug, Deserialize)]
pub struct SetChannelRequest {
    pub channel_id: String,
    pub channel_name: String,
}

/// POST /api/v1/auth/slack/channel - Set the notification channel
pub async fn slack_set_channel(
    State(state): State<AppState>,
    Json(req): Json<SetChannelRequest>,
) -> Result<Json<OAuthResult>, AppError> {
    state.slack.set_channel(&req.channel_id, &req.channel_name).await?;

    Ok(Json(OAuthResult {
        success: true,
        message: format!("Channel set to #{}", req.channel_name),
    }))
}

/// POST /api/v1/auth/slack/test - Send a test message
pub async fn slack_test(
    State(state): State<AppState>,
) -> Result<Json<OAuthResult>, AppError> {
    match state.slack.test_connection().await {
        Ok(_) => Ok(Json(OAuthResult {
            success: true,
            message: "Test message sent successfully".to_string(),
        })),
        Err(e) => Ok(Json(OAuthResult {
            success: false,
            message: e.to_string(),
        })),
    }
}

/// DELETE /api/v1/auth/slack - Disconnect Slack
pub async fn slack_disconnect(
    State(state): State<AppState>,
) -> Result<Json<OAuthResult>, AppError> {
    state.slack.disconnect().await?;

    Ok(Json(OAuthResult {
        success: true,
        message: "Slack disconnected".to_string(),
    }))
}
