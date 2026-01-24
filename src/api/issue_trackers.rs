//! Issue Tracker API Endpoints
//!
//! Configure and manage issue tracker integrations per project.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::AppState;
use crate::error::{AppError, OptionExt};
use crate::integrations::issue_trackers::{
    get_incident_external_issues, get_project_issue_trackers, IncidentExternalIssue,
    IssueTrackerConfigResponse, ProjectIssueTracker,
};

// ============================================================================
// Project Issue Tracker Config
// ============================================================================

/// GET /api/v1/projects/:id/issue-trackers - List all issue tracker configs for a project
pub async fn list_project_trackers(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<IssueTrackerConfigResponse>>, AppError> {
    // Verify project exists
    let _project = sqlx::query_scalar!("SELECT id FROM projects WHERE id = $1", project_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_not_found("Project not found")?;

    let trackers = sqlx::query_as!(
        ProjectIssueTracker,
        r#"
        SELECT id, project_id, provider, enabled,
               github_labels, github_assignees,
               jira_site_url, jira_project_key, jira_issue_type, jira_priority,
               status_mapping, webhook_url, webhook_headers, webhook_events,
               linear_team_id, linear_project_id,
               created_at, updated_at
        FROM project_issue_trackers
        WHERE project_id = $1
        ORDER BY provider
        "#,
        project_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        trackers
            .into_iter()
            .map(IssueTrackerConfigResponse::from)
            .collect(),
    ))
}

/// Request body for creating/updating issue tracker config
#[derive(Debug, Deserialize)]
pub struct UpsertIssueTrackerRequest {
    pub provider: String,
    pub enabled: Option<bool>,

    // GitHub config
    pub github_labels: Option<Vec<String>>,
    pub github_assignees: Option<Vec<String>>,

    // Jira config
    pub jira_site_url: Option<String>,
    pub jira_project_key: Option<String>,
    pub jira_issue_type: Option<String>,
    pub jira_priority: Option<String>,

    // Status mapping
    pub status_mapping: Option<serde_json::Value>,

    // Webhook config
    pub webhook_url: Option<String>,
    pub webhook_headers: Option<serde_json::Value>,
    pub webhook_events: Option<Vec<String>>,

    // Linear config
    pub linear_team_id: Option<String>,
    pub linear_project_id: Option<String>,
}

/// PUT /api/v1/projects/:id/issue-trackers/:provider - Create or update issue tracker config
pub async fn upsert_project_tracker(
    State(state): State<AppState>,
    Path((project_id, provider)): Path<(Uuid, String)>,
    Json(body): Json<UpsertIssueTrackerRequest>,
) -> Result<Json<IssueTrackerConfigResponse>, AppError> {
    // Verify project exists
    let _project = sqlx::query_scalar!("SELECT id FROM projects WHERE id = $1", project_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_not_found("Project not found")?;

    // Validate provider
    if !["github", "jira", "linear", "webhook"].contains(&provider.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Invalid provider: {}. Must be one of: github, jira, linear, webhook",
            provider
        )));
    }

    let enabled = body.enabled.unwrap_or(true);

    let tracker = sqlx::query_as!(
        ProjectIssueTracker,
        r#"
        INSERT INTO project_issue_trackers
            (project_id, provider, enabled,
             github_labels, github_assignees,
             jira_site_url, jira_project_key, jira_issue_type, jira_priority,
             status_mapping, webhook_url, webhook_headers, webhook_events,
             linear_team_id, linear_project_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ON CONFLICT (project_id, provider) DO UPDATE SET
            enabled = EXCLUDED.enabled,
            github_labels = COALESCE(EXCLUDED.github_labels, project_issue_trackers.github_labels),
            github_assignees = COALESCE(EXCLUDED.github_assignees, project_issue_trackers.github_assignees),
            jira_site_url = COALESCE(EXCLUDED.jira_site_url, project_issue_trackers.jira_site_url),
            jira_project_key = COALESCE(EXCLUDED.jira_project_key, project_issue_trackers.jira_project_key),
            jira_issue_type = COALESCE(EXCLUDED.jira_issue_type, project_issue_trackers.jira_issue_type),
            jira_priority = COALESCE(EXCLUDED.jira_priority, project_issue_trackers.jira_priority),
            status_mapping = COALESCE(EXCLUDED.status_mapping, project_issue_trackers.status_mapping),
            webhook_url = COALESCE(EXCLUDED.webhook_url, project_issue_trackers.webhook_url),
            webhook_headers = COALESCE(EXCLUDED.webhook_headers, project_issue_trackers.webhook_headers),
            webhook_events = COALESCE(EXCLUDED.webhook_events, project_issue_trackers.webhook_events),
            linear_team_id = COALESCE(EXCLUDED.linear_team_id, project_issue_trackers.linear_team_id),
            linear_project_id = COALESCE(EXCLUDED.linear_project_id, project_issue_trackers.linear_project_id),
            updated_at = NOW()
        RETURNING id, project_id, provider, enabled,
                  github_labels, github_assignees,
                  jira_site_url, jira_project_key, jira_issue_type, jira_priority,
                  status_mapping, webhook_url, webhook_headers, webhook_events,
                  linear_team_id, linear_project_id,
                  created_at, updated_at
        "#,
        project_id,
        provider,
        enabled,
        body.github_labels.as_deref(),
        body.github_assignees.as_deref(),
        body.jira_site_url,
        body.jira_project_key,
        body.jira_issue_type,
        body.jira_priority,
        body.status_mapping,
        body.webhook_url,
        body.webhook_headers,
        body.webhook_events.as_deref(),
        body.linear_team_id,
        body.linear_project_id
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(IssueTrackerConfigResponse::from(tracker)))
}

/// DELETE /api/v1/projects/:id/issue-trackers/:provider - Remove issue tracker config
pub async fn delete_project_tracker(
    State(state): State<AppState>,
    Path((project_id, provider)): Path<(Uuid, String)>,
) -> Result<Json<DeleteResponse>, AppError> {
    let result = sqlx::query!(
        "DELETE FROM project_issue_trackers WHERE project_id = $1 AND provider = $2",
        project_id,
        provider
    )
    .execute(&state.db)
    .await?;

    Ok(Json(DeleteResponse {
        deleted: result.rows_affected() > 0,
    }))
}

#[derive(Debug, Serialize)]
pub struct DeleteResponse {
    pub deleted: bool,
}

// ============================================================================
// Incident External Issues
// ============================================================================

/// GET /api/v1/incidents/:id/external-issues - List external issues for an incident
pub async fn list_incident_external_issues(
    State(state): State<AppState>,
    Path(incident_id): Path<Uuid>,
) -> Result<Json<Vec<IncidentExternalIssue>>, AppError> {
    // Verify incident exists
    let _incident = sqlx::query_scalar!("SELECT id FROM incidents WHERE id = $1", incident_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_not_found("Incident not found")?;

    let issues = get_incident_external_issues(&state.db, incident_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(issues))
}

// ============================================================================
// Jira OAuth
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct JiraApiTokenRequest {
    pub site_url: String,
    pub email: String,
    pub api_token: String,
}

/// POST /api/v1/auth/jira/token - Connect Jira with API token
pub async fn connect_jira_token(
    State(state): State<AppState>,
    Json(body): Json<JiraApiTokenRequest>,
) -> Result<Json<JiraConnectionResponse>, AppError> {
    // Test the connection
    let client = crate::integrations::issue_trackers::jira::JiraClient::with_api_token(
        &body.site_url,
        &body.email,
        &body.api_token,
    );

    let user = client
        .test_connection()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to connect to Jira: {}", e)))?;

    // Store credentials
    sqlx::query!(
        r#"
        INSERT INTO jira_connections (site_url, api_token, user_email, connected_by)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (site_url) DO UPDATE SET
            api_token = EXCLUDED.api_token,
            user_email = EXCLUDED.user_email,
            connected_by = EXCLUDED.connected_by,
            updated_at = NOW()
        "#,
        body.site_url,
        body.api_token,
        body.email,
        user.display_name
    )
    .execute(&state.db)
    .await?;

    Ok(Json(JiraConnectionResponse {
        connected: true,
        site_url: body.site_url,
        user_name: user.display_name,
        user_email: user.email_address,
    }))
}

#[derive(Debug, Serialize)]
pub struct JiraConnectionResponse {
    pub connected: bool,
    pub site_url: String,
    pub user_name: String,
    pub user_email: Option<String>,
}

/// GET /api/v1/auth/jira/status - Get Jira connection status
pub async fn get_jira_status(
    State(state): State<AppState>,
) -> Result<Json<JiraStatusResponse>, AppError> {
    let conn = sqlx::query!(
        r#"
        SELECT site_url, connected_by, user_email, created_at
        FROM jira_connections
        ORDER BY created_at DESC
        LIMIT 1
        "#
    )
    .fetch_optional(&state.db)
    .await?;

    match conn {
        Some(c) => Ok(Json(JiraStatusResponse {
            connected: true,
            site_url: Some(c.site_url),
            connected_by: c.connected_by,
            user_email: c.user_email,
        })),
        None => Ok(Json(JiraStatusResponse {
            connected: false,
            site_url: None,
            connected_by: None,
            user_email: None,
        })),
    }
}

#[derive(Debug, Serialize)]
pub struct JiraStatusResponse {
    pub connected: bool,
    pub site_url: Option<String>,
    pub connected_by: Option<String>,
    pub user_email: Option<String>,
}

/// DELETE /api/v1/auth/jira - Disconnect Jira
pub async fn disconnect_jira(
    State(state): State<AppState>,
) -> Result<Json<DeleteResponse>, AppError> {
    let result = sqlx::query!("DELETE FROM jira_connections")
        .execute(&state.db)
        .await?;

    Ok(Json(DeleteResponse {
        deleted: result.rows_affected() > 0,
    }))
}

// ============================================================================
// Linear OAuth
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct LinearApiKeyRequest {
    pub api_key: String,
}

/// POST /api/v1/auth/linear/token - Connect Linear with API key
pub async fn connect_linear_token(
    State(state): State<AppState>,
    Json(body): Json<LinearApiKeyRequest>,
) -> Result<Json<LinearConnectionResponse>, AppError> {
    // Test the connection
    let client = crate::integrations::issue_trackers::linear::LinearClient::new(&body.api_key);

    let viewer = client
        .test_connection()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to connect to Linear: {}", e)))?;

    // Store credentials
    sqlx::query!(
        r#"
        INSERT INTO linear_connections (access_token, connected_by)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
        "#,
        body.api_key,
        viewer.name
    )
    .execute(&state.db)
    .await?;

    Ok(Json(LinearConnectionResponse {
        connected: true,
        user_name: viewer.name,
        user_email: viewer.email,
    }))
}

#[derive(Debug, Serialize)]
pub struct LinearConnectionResponse {
    pub connected: bool,
    pub user_name: String,
    pub user_email: String,
}

/// GET /api/v1/auth/linear/status - Get Linear connection status
pub async fn get_linear_status(
    State(state): State<AppState>,
) -> Result<Json<LinearStatusResponse>, AppError> {
    let conn = sqlx::query!(
        r#"
        SELECT connected_by, organization_name, created_at
        FROM linear_connections
        ORDER BY created_at DESC
        LIMIT 1
        "#
    )
    .fetch_optional(&state.db)
    .await?;

    match conn {
        Some(c) => Ok(Json(LinearStatusResponse {
            connected: true,
            connected_by: c.connected_by,
            organization_name: c.organization_name,
        })),
        None => Ok(Json(LinearStatusResponse {
            connected: false,
            connected_by: None,
            organization_name: None,
        })),
    }
}

#[derive(Debug, Serialize)]
pub struct LinearStatusResponse {
    pub connected: bool,
    pub connected_by: Option<String>,
    pub organization_name: Option<String>,
}

/// DELETE /api/v1/auth/linear - Disconnect Linear
pub async fn disconnect_linear(
    State(state): State<AppState>,
) -> Result<Json<DeleteResponse>, AppError> {
    let result = sqlx::query!("DELETE FROM linear_connections")
        .execute(&state.db)
        .await?;

    Ok(Json(DeleteResponse {
        deleted: result.rows_affected() > 0,
    }))
}

// ============================================================================
// Test Webhook
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct TestWebhookRequest {
    pub url: String,
    pub headers: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct TestWebhookResponse {
    pub success: bool,
    pub status_code: Option<i32>,
    pub response_body: Option<String>,
    pub error: Option<String>,
}

/// POST /api/v1/projects/:id/issue-trackers/webhook/test - Test webhook configuration
pub async fn test_webhook(
    State(_state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<TestWebhookRequest>,
) -> Result<Json<TestWebhookResponse>, AppError> {
    let client = reqwest::Client::builder()
        .user_agent("CRISPR-Webhook/1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let test_payload = serde_json::json!({
        "event": "test",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "message": "Test webhook from CRISPR",
        "project_id": project_id.to_string()
    });

    let mut request = client
        .post(&body.url)
        .header("Content-Type", "application/json")
        .header("X-CRISPR-Event", "test");

    // Add custom headers
    if let Some(headers) = &body.headers {
        if let Some(obj) = headers.as_object() {
            for (key, value) in obj {
                if let Some(v) = value.as_str() {
                    request = request.header(key, v);
                }
            }
        }
    }

    match request.json(&test_payload).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16() as i32;
            let body = resp.text().await.ok();

            Ok(Json(TestWebhookResponse {
                success: status >= 200 && status < 300,
                status_code: Some(status),
                response_body: body,
                error: None,
            }))
        }
        Err(e) => Ok(Json(TestWebhookResponse {
            success: false,
            status_code: None,
            response_body: None,
            error: Some(e.to_string()),
        })),
    }
}
