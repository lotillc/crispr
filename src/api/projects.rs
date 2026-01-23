//! Projects API endpoints
//!
//! CRUD operations for watched repositories.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use crate::error::{AppError, OptionExt};
use crate::store::models::{Project, ProjectResponse};

#[derive(Debug, Deserialize)]
pub struct ListProjectsQuery {
    pub enabled_only: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResponse>,
    pub total: i64,
}

/// GET /api/v1/projects - List all projects
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListProjectsQuery>,
) -> Result<Json<ProjectListResponse>, AppError> {
    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let (projects, total) = if query.enabled_only.unwrap_or(false) {
        let projects = sqlx::query_as!(
            Project,
            r#"
            SELECT id, github_owner, github_repo, default_branch, language,
                   enabled, auto_registered, settings, created_at, updated_at
            FROM projects
            WHERE enabled = true
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(&state.db)
        .await?;

        let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM projects WHERE enabled = true")
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

        (projects, total)
    } else {
        let projects = sqlx::query_as!(
            Project,
            r#"
            SELECT id, github_owner, github_repo, default_branch, language,
                   enabled, auto_registered, settings, created_at, updated_at
            FROM projects
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(&state.db)
        .await?;

        let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM projects")
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

        (projects, total)
    };

    Ok(Json(ProjectListResponse {
        projects: projects.into_iter().map(ProjectResponse::from).collect(),
        total,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub github_owner: String,
    pub github_repo: String,
    pub default_branch: Option<String>,
    pub language: Option<String>,
}

/// POST /api/v1/projects - Create a new project
pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<ProjectResponse>, AppError> {
    // Check if project already exists
    let existing = sqlx::query!(
        "SELECT id FROM projects WHERE github_owner = $1 AND github_repo = $2",
        req.github_owner,
        req.github_repo
    )
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Err(AppError::Conflict(format!(
            "Project {}/{} already exists",
            req.github_owner, req.github_repo
        )));
    }

    // Try to get default branch from GitHub if not provided
    let default_branch = if let Some(branch) = req.default_branch {
        branch
    } else if state.github.is_connected().await {
        state
            .github
            .get_default_branch(&req.github_owner, &req.github_repo)
            .await
            .unwrap_or_else(|_| "main".to_string())
    } else {
        "main".to_string()
    };

    // Try to get language from GitHub if not provided
    let language = if let Some(lang) = req.language {
        Some(lang)
    } else if state.github.is_connected().await {
        state
            .github
            .get_repo(&req.github_owner, &req.github_repo)
            .await
            .ok()
            .and_then(|r| r.language)
    } else {
        None
    };

    let project = sqlx::query_as!(
        Project,
        r#"
        INSERT INTO projects (id, github_owner, github_repo, default_branch, language, auto_registered)
        VALUES ($1, $2, $3, $4, $5, false)
        RETURNING id, github_owner, github_repo, default_branch, language,
                  enabled, auto_registered, settings, created_at, updated_at
        "#,
        Uuid::new_v4(),
        req.github_owner,
        req.github_repo,
        default_branch,
        language,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ProjectResponse::from(project)))
}

/// GET /api/v1/projects/:id - Get a project by ID
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectResponse>, AppError> {
    let project = sqlx::query_as!(
        Project,
        r#"
        SELECT id, github_owner, github_repo, default_branch, language,
               enabled, auto_registered, settings, created_at, updated_at
        FROM projects WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Project not found")?;

    Ok(Json(ProjectResponse::from(project)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub default_branch: Option<String>,
    pub language: Option<String>,
    pub enabled: Option<bool>,
    pub settings: Option<serde_json::Value>,
}

/// PATCH /api/v1/projects/:id - Update a project
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectResponse>, AppError> {
    // Verify project exists
    let existing = sqlx::query!("SELECT id FROM projects WHERE id = $1", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_not_found("Project not found")?;

    let project = sqlx::query_as!(
        Project,
        r#"
        UPDATE projects SET
            default_branch = COALESCE($1, default_branch),
            language = COALESCE($2, language),
            enabled = COALESCE($3, enabled),
            settings = COALESCE($4, settings),
            updated_at = NOW()
        WHERE id = $5
        RETURNING id, github_owner, github_repo, default_branch, language,
                  enabled, auto_registered, settings, created_at, updated_at
        "#,
        req.default_branch,
        req.language,
        req.enabled,
        req.settings,
        id,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ProjectResponse::from(project)))
}

/// DELETE /api/v1/projects/:id - Delete a project
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = sqlx::query!("DELETE FROM projects WHERE id = $1", id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Project not found".to_string()));
    }

    Ok(Json(serde_json::json!({
        "deleted": true,
        "id": id
    })))
}

#[derive(Debug, Serialize)]
pub struct ProjectStats {
    pub total_incidents: i64,
    pub open_incidents: i64,
    pub fixed_incidents: i64,
    pub prs_opened: i64,
    pub prs_merged: i64,
}

/// GET /api/v1/projects/:id/stats - Get project statistics
pub async fn stats(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProjectStats>, AppError> {
    // Verify project exists
    sqlx::query!("SELECT id FROM projects WHERE id = $1", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_not_found("Project not found")?;

    let stats = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as "total!",
            COUNT(*) FILTER (WHERE state IN ('detected', 'triaging', 'fixing')) as "open!",
            COUNT(*) FILTER (WHERE state = 'pr_merged') as "fixed!",
            COUNT(*) FILTER (WHERE pr_url IS NOT NULL) as "prs_opened!",
            COUNT(*) FILTER (WHERE pr_state = 'merged') as "prs_merged!"
        FROM incidents
        WHERE project_id = $1
        "#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ProjectStats {
        total_incidents: stats.total,
        open_incidents: stats.open,
        fixed_incidents: stats.fixed,
        prs_opened: stats.prs_opened,
        prs_merged: stats.prs_merged,
    }))
}
