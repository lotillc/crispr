//! Incidents API endpoints
//!
//! List, view, and manage incidents.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use crate::error::{AppError, OptionExt};
use crate::store::models::{ActivityLog, Incident, IncidentResponse};

#[derive(Debug, Deserialize)]
pub struct ListIncidentsQuery {
    pub project_id: Option<Uuid>,
    pub state: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct IncidentListResponse {
    pub incidents: Vec<IncidentResponse>,
    pub total: i64,
}

/// GET /api/v1/incidents - List incidents
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListIncidentsQuery>,
) -> Result<Json<IncidentListResponse>, AppError> {
    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let (incidents, total) = match (&query.project_id, &query.state) {
        (Some(project_id), Some(incident_state)) => {
            let incidents = sqlx::query_as!(
                Incident,
                r#"
                SELECT id, project_id, fingerprint, title, state, severity,
                       occurrence_count, first_seen, last_seen, cooldown_until,
                       sample_location, fix_attempts, pr_url, pr_number, pr_state,
                       created_at, updated_at
                FROM incidents
                WHERE project_id = $1 AND state = $2
                ORDER BY last_seen DESC
                LIMIT $3 OFFSET $4
                "#,
                project_id,
                incident_state,
                limit,
                offset
            )
            .fetch_all(&state.db)
            .await?;

            let total: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM incidents WHERE project_id = $1 AND state = $2",
                project_id,
                incident_state
            )
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

            (incidents, total)
        }
        (Some(project_id), None) => {
            let incidents = sqlx::query_as!(
                Incident,
                r#"
                SELECT id, project_id, fingerprint, title, state, severity,
                       occurrence_count, first_seen, last_seen, cooldown_until,
                       sample_location, fix_attempts, pr_url, pr_number, pr_state,
                       created_at, updated_at
                FROM incidents
                WHERE project_id = $1
                ORDER BY last_seen DESC
                LIMIT $2 OFFSET $3
                "#,
                project_id,
                limit,
                offset
            )
            .fetch_all(&state.db)
            .await?;

            let total: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM incidents WHERE project_id = $1",
                project_id
            )
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

            (incidents, total)
        }
        (None, Some(incident_state)) => {
            let incidents = sqlx::query_as!(
                Incident,
                r#"
                SELECT id, project_id, fingerprint, title, state, severity,
                       occurrence_count, first_seen, last_seen, cooldown_until,
                       sample_location, fix_attempts, pr_url, pr_number, pr_state,
                       created_at, updated_at
                FROM incidents
                WHERE state = $1
                ORDER BY last_seen DESC
                LIMIT $2 OFFSET $3
                "#,
                incident_state,
                limit,
                offset
            )
            .fetch_all(&state.db)
            .await?;

            let total: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM incidents WHERE state = $1",
                incident_state
            )
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

            (incidents, total)
        }
        (None, None) => {
            let incidents = sqlx::query_as!(
                Incident,
                r#"
                SELECT id, project_id, fingerprint, title, state, severity,
                       occurrence_count, first_seen, last_seen, cooldown_until,
                       sample_location, fix_attempts, pr_url, pr_number, pr_state,
                       created_at, updated_at
                FROM incidents
                ORDER BY last_seen DESC
                LIMIT $1 OFFSET $2
                "#,
                limit,
                offset
            )
            .fetch_all(&state.db)
            .await?;

            let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM incidents")
                .fetch_one(&state.db)
                .await?
                .unwrap_or(0);

            (incidents, total)
        }
    };

    Ok(Json(IncidentListResponse {
        incidents: incidents.into_iter().map(IncidentResponse::from).collect(),
        total,
    }))
}

#[derive(Debug, Serialize)]
pub struct IncidentDetailResponse {
    #[serde(flatten)]
    pub incident: IncidentResponse,
    pub project: ProjectInfo,
    pub sample: Option<serde_json::Value>,
    pub activity: Vec<ActivityEntry>,
}

#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    pub github_owner: String,
    pub github_repo: String,
}

#[derive(Debug, Serialize)]
pub struct ActivityEntry {
    pub id: Uuid,
    pub action: String,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// GET /api/v1/incidents/:id - Get incident details
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<IncidentDetailResponse>, AppError> {
    let incident = sqlx::query_as!(
        Incident,
        r#"
        SELECT id, project_id, fingerprint, title, state, severity,
               occurrence_count, first_seen, last_seen, cooldown_until,
               sample_location, fix_attempts, pr_url, pr_number, pr_state,
               created_at, updated_at
        FROM incidents WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Incident not found")?;

    // Get project info
    let project = sqlx::query!(
        "SELECT github_owner, github_repo FROM projects WHERE id = $1",
        incident.project_id
    )
    .fetch_one(&state.db)
    .await?;

    // Get sample if available
    let sample = if let Some(location) = &incident.sample_location {
        match state.storage.get(location).await {
            Ok(bytes) => serde_json::from_slice(&bytes).ok(),
            Err(_) => None,
        }
    } else {
        None
    };

    // Get activity log
    let activity = sqlx::query_as!(
        ActivityLog,
        r#"
        SELECT id, incident_id, project_id, action, details, created_at
        FROM activity_log
        WHERE incident_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
        id
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|a| ActivityEntry {
        id: a.id,
        action: a.action,
        details: a.details.unwrap_or_else(|| serde_json::json!({})),
        created_at: a.created_at.unwrap_or_else(Utc::now),
    })
    .collect();

    Ok(Json(IncidentDetailResponse {
        incident: IncidentResponse::from(incident),
        project: ProjectInfo {
            github_owner: project.github_owner,
            github_repo: project.github_repo,
        },
        sample,
        activity,
    }))
}

/// POST /api/v1/incidents/:id/retry - Retry processing an incident
pub async fn retry(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<IncidentResponse>, AppError> {
    // Check incident exists and is in a retryable state
    let incident = sqlx::query_as!(
        Incident,
        r#"
        SELECT id, project_id, fingerprint, title, state, severity,
               occurrence_count, first_seen, last_seen, cooldown_until,
               sample_location, fix_attempts, pr_url, pr_number, pr_state,
               created_at, updated_at
        FROM incidents WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Incident not found")?;

    let state_str = incident.state.as_deref().unwrap_or("unknown");
    if !matches!(
        state_str,
        "fix_failed" | "unfixable" | "cooldown" | "pr_closed"
    ) {
        return Err(AppError::BadRequest(format!(
            "Cannot retry incident in state '{}'",
            state_str
        )));
    }

    // Reset to detected state
    let updated = sqlx::query_as!(
        Incident,
        r#"
        UPDATE incidents
        SET state = 'detected',
            cooldown_until = NULL,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, fingerprint, title, state, severity,
                  occurrence_count, first_seen, last_seen, cooldown_until,
                  sample_location, fix_attempts, pr_url, pr_number, pr_state,
                  created_at, updated_at
        "#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    // Log the retry
    sqlx::query!(
        r#"
        INSERT INTO activity_log (id, incident_id, project_id, action, details)
        VALUES ($1, $2, $3, 'retry_requested', '{}'::jsonb)
        "#,
        Uuid::new_v4(),
        id,
        incident.project_id,
    )
    .execute(&state.db)
    .await?;

    Ok(Json(IncidentResponse::from(updated)))
}

/// POST /api/v1/incidents/:id/ignore - Mark incident as ignored/unfixable
pub async fn ignore(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<IncidentResponse>, AppError> {
    let incident = sqlx::query_as!(
        Incident,
        r#"
        SELECT id, project_id, fingerprint, title, state, severity,
               occurrence_count, first_seen, last_seen, cooldown_until,
               sample_location, fix_attempts, pr_url, pr_number, pr_state,
               created_at, updated_at
        FROM incidents WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Incident not found")?;

    let updated = sqlx::query_as!(
        Incident,
        r#"
        UPDATE incidents
        SET state = 'unfixable',
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, fingerprint, title, state, severity,
                  occurrence_count, first_seen, last_seen, cooldown_until,
                  sample_location, fix_attempts, pr_url, pr_number, pr_state,
                  created_at, updated_at
        "#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    // Log the ignore action
    sqlx::query!(
        r#"
        INSERT INTO activity_log (id, incident_id, project_id, action, details)
        VALUES ($1, $2, $3, 'manually_ignored', '{}'::jsonb)
        "#,
        Uuid::new_v4(),
        id,
        incident.project_id,
    )
    .execute(&state.db)
    .await?;

    Ok(Json(IncidentResponse::from(updated)))
}

#[derive(Debug, Serialize)]
pub struct IncidentStats {
    pub total: i64,
    pub by_state: Vec<StateCount>,
    pub recent_activity: i64,
}

#[derive(Debug, Serialize)]
pub struct StateCount {
    pub state: String,
    pub count: i64,
}

/// GET /api/v1/incidents/stats - Get incident statistics
pub async fn stats(State(state): State<AppState>) -> Result<Json<IncidentStats>, AppError> {
    let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM incidents")
        .fetch_one(&state.db)
        .await?
        .unwrap_or(0);

    let by_state = sqlx::query!(
        r#"
        SELECT state as "state!", COUNT(*)::bigint as "count!"
        FROM incidents
        GROUP BY state
        ORDER BY "count!" DESC
        "#
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r| StateCount {
        state: r.state,
        count: r.count,
    })
    .collect();

    let recent_activity: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM incidents WHERE last_seen > NOW() - INTERVAL '24 hours'"
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0);

    Ok(Json(IncidentStats {
        total,
        by_state,
        recent_activity,
    }))
}
