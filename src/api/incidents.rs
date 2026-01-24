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
use crate::pipeline::{get_queue, get_verification_summary, start_verification, SurgicalQueue, VerificationSummary};
use crate::store::models::{ActivityLog, GeneratedTestRecord, GeneratedTestResponse, Incident, IncidentResponse};

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
                       tests_generated, test_count, matched_pattern_id, pattern_auto_triaged,
                       queued_at, queue_position, verification_round, verification_status,
                       verified_at, pr_merged_at,
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
                       tests_generated, test_count, matched_pattern_id, pattern_auto_triaged,
                       queued_at, queue_position, verification_round, verification_status,
                       verified_at, pr_merged_at,
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
                       tests_generated, test_count, matched_pattern_id, pattern_auto_triaged,
                       queued_at, queue_position, verification_round, verification_status,
                       verified_at, pr_merged_at,
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
                       tests_generated, test_count, matched_pattern_id, pattern_auto_triaged,
                       queued_at, queue_position, verification_round, verification_status,
                       verified_at, pr_merged_at,
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
               tests_generated, test_count, matched_pattern_id, pattern_auto_triaged,
               queued_at, queue_position, verification_round, verification_status,
               verified_at, pr_merged_at,
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
               tests_generated, test_count, matched_pattern_id, pattern_auto_triaged,
               queued_at, queue_position, verification_round, verification_status,
               verified_at, pr_merged_at,
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
                  tests_generated, test_count, matched_pattern_id, pattern_auto_triaged,
                  queued_at, queue_position, verification_round, verification_status,
                  verified_at, pr_merged_at,
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
               tests_generated, test_count, matched_pattern_id, pattern_auto_triaged,
               queued_at, queue_position, verification_round, verification_status,
               verified_at, pr_merged_at,
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
                  tests_generated, test_count, matched_pattern_id, pattern_auto_triaged,
                  queued_at, queue_position, verification_round, verification_status,
                  verified_at, pr_merged_at,
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
    /// Number of incidents that were auto-triaged via pattern matching
    pub auto_triaged: i64,
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

    let auto_triaged: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM incidents WHERE pattern_auto_triaged = true"
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0);

    Ok(Json(IncidentStats {
        total,
        by_state,
        recent_activity,
        auto_triaged,
    }))
}

// ============================================================================
// Generated Tests API
// ============================================================================

#[derive(Debug, Serialize)]
pub struct GeneratedTestsListResponse {
    pub tests: Vec<GeneratedTestResponse>,
    pub total: i64,
}

/// GET /api/v1/incidents/:id/tests - List generated tests for an incident
pub async fn list_tests(
    State(state): State<AppState>,
    Path(incident_id): Path<Uuid>,
) -> Result<Json<GeneratedTestsListResponse>, AppError> {
    // Verify incident exists
    let _incident = sqlx::query_scalar!(
        "SELECT id FROM incidents WHERE id = $1",
        incident_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Incident not found")?;

    // Fetch all generated tests for this incident
    let tests = sqlx::query_as!(
        GeneratedTestRecord,
        r#"
        SELECT id, incident_id, file_path, content, description, framework,
               included_in_pr, commit_sha, created_at
        FROM generated_tests
        WHERE incident_id = $1
        ORDER BY created_at DESC
        "#,
        incident_id
    )
    .fetch_all(&state.db)
    .await?;

    let total = tests.len() as i64;
    let responses: Vec<GeneratedTestResponse> = tests.into_iter().map(GeneratedTestResponse::from).collect();

    Ok(Json(GeneratedTestsListResponse {
        tests: responses,
        total,
    }))
}

/// Path parameters for test-specific endpoints
#[derive(Debug, Deserialize)]
pub struct TestPath {
    pub id: Uuid,
    pub test_id: Uuid,
}

/// Request body for updating a generated test
#[derive(Debug, Deserialize)]
pub struct UpdateTestRequest {
    /// Whether to include this test in the PR
    pub included_in_pr: Option<bool>,
}

/// PATCH /api/v1/incidents/:id/tests/:test_id - Update a generated test
pub async fn update_test(
    State(state): State<AppState>,
    Path(path): Path<TestPath>,
    Json(body): Json<UpdateTestRequest>,
) -> Result<Json<GeneratedTestResponse>, AppError> {
    // Verify incident exists
    let _incident = sqlx::query_scalar!(
        "SELECT id FROM incidents WHERE id = $1",
        path.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Incident not found")?;

    // Verify test exists and belongs to this incident
    let test = sqlx::query_as!(
        GeneratedTestRecord,
        r#"
        SELECT id, incident_id, file_path, content, description, framework,
               included_in_pr, commit_sha, created_at
        FROM generated_tests
        WHERE id = $1 AND incident_id = $2
        "#,
        path.test_id,
        path.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Generated test not found")?;

    // Update the test if included_in_pr is provided
    let updated_test = if let Some(included) = body.included_in_pr {
        sqlx::query_as!(
            GeneratedTestRecord,
            r#"
            UPDATE generated_tests
            SET included_in_pr = $1
            WHERE id = $2
            RETURNING id, incident_id, file_path, content, description, framework,
                      included_in_pr, commit_sha, created_at
            "#,
            included,
            path.test_id
        )
        .fetch_one(&state.db)
        .await?
    } else {
        test
    };

    Ok(Json(GeneratedTestResponse::from(updated_test)))
}

/// GET /api/v1/incidents/:id/tests/:test_id - Get a single generated test
pub async fn get_test(
    State(state): State<AppState>,
    Path(path): Path<TestPath>,
) -> Result<Json<GeneratedTestResponse>, AppError> {
    // Verify incident exists
    let _incident = sqlx::query_scalar!(
        "SELECT id FROM incidents WHERE id = $1",
        path.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Incident not found")?;

    // Fetch the test
    let test = sqlx::query_as!(
        GeneratedTestRecord,
        r#"
        SELECT id, incident_id, file_path, content, description, framework,
               included_in_pr, commit_sha, created_at
        FROM generated_tests
        WHERE id = $1 AND incident_id = $2
        "#,
        path.test_id,
        path.id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Generated test not found")?;

    Ok(Json(GeneratedTestResponse::from(test)))
}

// ============================================================================
// Verification API
// ============================================================================

/// GET /api/v1/incidents/:id/verification - Get verification status and rounds
pub async fn get_verification(
    State(state): State<AppState>,
    Path(incident_id): Path<Uuid>,
) -> Result<Json<VerificationSummary>, AppError> {
    // Verify incident exists
    let _incident = sqlx::query_scalar!(
        "SELECT id FROM incidents WHERE id = $1",
        incident_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Incident not found")?;

    let summary = get_verification_summary(&state.db, incident_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(summary))
}

/// POST /api/v1/incidents/:id/verify-now - Force start verification
pub async fn verify_now(
    State(state): State<AppState>,
    Path(incident_id): Path<Uuid>,
) -> Result<Json<VerificationSummary>, AppError> {
    // Verify incident exists
    let incident = sqlx::query!(
        "SELECT id, pr_state, verification_status FROM incidents WHERE id = $1",
        incident_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Incident not found")?;

    // Check if already verifying
    if incident.verification_status.as_deref() == Some("verifying") {
        return Err(AppError::BadRequest("Incident is already in verification".to_string()));
    }

    // Start verification
    start_verification(&state.db, incident_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Log the manual verification start
    sqlx::query!(
        r#"
        INSERT INTO activity_log (id, incident_id, action, details)
        VALUES ($1, $2, 'verification_started_manually', '{}'::jsonb)
        "#,
        Uuid::new_v4(),
        incident_id,
    )
    .execute(&state.db)
    .await?;

    // Return the updated summary
    let summary = get_verification_summary(&state.db, incident_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(summary))
}

// ============================================================================
// Surgical Queue API
// ============================================================================

/// GET /api/v1/projects/:id/queue - Get surgical queue for a project
pub async fn get_project_queue(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<SurgicalQueue>, AppError> {
    // Verify project exists
    let _project = sqlx::query_scalar!(
        "SELECT id FROM projects WHERE id = $1",
        project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Project not found")?;

    let queue = get_queue(&state.db, project_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(queue))
}
