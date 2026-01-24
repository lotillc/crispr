//! Patterns API endpoints
//!
//! List, create, update, and delete error patterns.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use crate::error::{AppError, OptionExt};
use crate::patterns::{
    CategoryInfo, CreatePatternRequest, ErrorPatternResponse, IncidentPatternMatch,
    PatternStats, UpdatePatternRequest,
};

#[derive(Debug, Deserialize)]
pub struct ListPatternsQuery {
    pub category: Option<String>,
    pub builtin: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PatternListResponse {
    pub patterns: Vec<ErrorPatternResponse>,
    pub total: usize,
}

/// GET /api/v1/patterns - List all patterns
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListPatternsQuery>,
) -> Result<Json<PatternListResponse>, AppError> {
    let patterns = state
        .patterns
        .list_patterns(query.category.as_deref(), query.builtin)
        .await?;

    let total = patterns.len();
    Ok(Json(PatternListResponse {
        patterns: patterns.into_iter().map(ErrorPatternResponse::from).collect(),
        total,
    }))
}

/// GET /api/v1/patterns/categories - List all pattern categories
pub async fn list_categories(
    State(state): State<AppState>,
) -> Result<Json<Vec<CategoryInfo>>, AppError> {
    let categories = state.patterns.list_categories().await?;
    Ok(Json(categories))
}

/// GET /api/v1/patterns/:id - Get a specific pattern
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ErrorPatternResponse>, AppError> {
    let pattern = state
        .patterns
        .get_pattern(id)
        .await?
        .ok_or_not_found("Pattern not found")?;

    Ok(Json(ErrorPatternResponse::from(pattern)))
}

/// POST /api/v1/patterns - Create a new pattern
pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreatePatternRequest>,
) -> Result<Json<ErrorPatternResponse>, AppError> {
    // Validate required fields
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("Name is required".to_string()));
    }
    if req.regex_pattern.trim().is_empty() {
        return Err(AppError::BadRequest("Regex pattern is required".to_string()));
    }
    if req.category.trim().is_empty() {
        return Err(AppError::BadRequest("Category is required".to_string()));
    }

    let pattern = state.patterns.create_pattern(req).await.map_err(|e| {
        if e.to_string().contains("Invalid regex") {
            AppError::BadRequest(e.to_string())
        } else {
            AppError::Internal(e.to_string())
        }
    })?;

    Ok(Json(ErrorPatternResponse::from(pattern)))
}

/// PATCH /api/v1/patterns/:id - Update a pattern
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePatternRequest>,
) -> Result<Json<ErrorPatternResponse>, AppError> {
    let pattern = state.patterns.update_pattern(id, req).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("not found") || msg.contains("built-in") {
            AppError::BadRequest(msg)
        } else if msg.contains("Invalid regex") {
            AppError::BadRequest(msg)
        } else {
            AppError::Internal(msg)
        }
    })?;

    Ok(Json(ErrorPatternResponse::from(pattern)))
}

/// DELETE /api/v1/patterns/:id - Delete a pattern
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let deleted = state.patterns.delete_pattern(id).await?;

    if deleted {
        Ok(Json(serde_json::json!({ "deleted": true })))
    } else {
        Err(AppError::BadRequest(
            "Pattern not found or is a built-in pattern".to_string(),
        ))
    }
}

/// GET /api/v1/patterns/:id/stats - Get pattern statistics
pub async fn stats(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PatternStats>, AppError> {
    // Verify pattern exists
    state
        .patterns
        .get_pattern(id)
        .await?
        .ok_or_not_found("Pattern not found")?;

    let stats = state.patterns.get_pattern_stats(id).await?;
    Ok(Json(stats))
}

/// POST /api/v1/patterns/test - Test a regex pattern against sample text
#[derive(Debug, Deserialize)]
pub struct TestPatternRequest {
    pub regex_pattern: String,
    pub sample_text: String,
}

#[derive(Debug, Serialize)]
pub struct TestPatternResponse {
    pub matches: bool,
    pub matched_text: Option<String>,
    pub match_start: Option<usize>,
    pub match_end: Option<usize>,
    pub error: Option<String>,
}

pub async fn test_pattern(
    Json(req): Json<TestPatternRequest>,
) -> Result<Json<TestPatternResponse>, AppError> {
    match regex::Regex::new(&req.regex_pattern) {
        Ok(regex) => {
            if let Some(m) = regex.find(&req.sample_text) {
                Ok(Json(TestPatternResponse {
                    matches: true,
                    matched_text: Some(m.as_str().to_string()),
                    match_start: Some(m.start()),
                    match_end: Some(m.end()),
                    error: None,
                }))
            } else {
                Ok(Json(TestPatternResponse {
                    matches: false,
                    matched_text: None,
                    match_start: None,
                    match_end: None,
                    error: None,
                }))
            }
        }
        Err(e) => Ok(Json(TestPatternResponse {
            matches: false,
            matched_text: None,
            match_start: None,
            match_end: None,
            error: Some(e.to_string()),
        })),
    }
}

/// POST /api/v1/patterns/reload - Reload patterns from database
pub async fn reload(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    state.patterns.reload_patterns().await?;
    Ok(Json(serde_json::json!({ "reloaded": true })))
}

/// GET /api/v1/incidents/:id/pattern-matches - Get pattern matches for an incident
pub async fn incident_matches(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<IncidentPatternMatch>>, AppError> {
    let matches = state.patterns.get_incident_matches(id).await?;
    Ok(Json(matches))
}

#[derive(Debug, Serialize)]
pub struct PatternMatchSummary {
    pub total_patterns: i64,
    pub enabled_patterns: i64,
    pub builtin_patterns: i64,
    pub custom_patterns: i64,
    pub total_matches: i64,
    pub matches_today: i64,
    pub top_categories: Vec<CategoryMatchCount>,
}

#[derive(Debug, Serialize)]
pub struct CategoryMatchCount {
    pub category: String,
    pub match_count: i64,
}

/// GET /api/v1/patterns/summary - Get pattern matching summary/stats
pub async fn summary(State(state): State<AppState>) -> Result<Json<PatternMatchSummary>, AppError> {
    let total_patterns: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM error_patterns")
        .fetch_one(&state.db)
        .await?
        .unwrap_or(0);

    let enabled_patterns: i64 =
        sqlx::query_scalar!("SELECT COUNT(*) FROM error_patterns WHERE enabled = true")
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

    let builtin_patterns: i64 =
        sqlx::query_scalar!("SELECT COUNT(*) FROM error_patterns WHERE is_builtin = true")
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

    let custom_patterns: i64 =
        sqlx::query_scalar!("SELECT COUNT(*) FROM error_patterns WHERE is_builtin = false")
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

    let total_matches: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM pattern_match_stats")
        .fetch_one(&state.db)
        .await?
        .unwrap_or(0);

    let matches_today: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM pattern_match_stats WHERE created_at > NOW() - INTERVAL '24 hours'"
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0);

    let top_categories = sqlx::query!(
        r#"
        SELECT ep.category, COUNT(pms.id)::bigint as "match_count!"
        FROM pattern_match_stats pms
        JOIN error_patterns ep ON pms.pattern_id = ep.id
        GROUP BY ep.category
        ORDER BY "match_count!" DESC
        LIMIT 10
        "#
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r| CategoryMatchCount {
        category: r.category,
        match_count: r.match_count,
    })
    .collect();

    Ok(Json(PatternMatchSummary {
        total_patterns,
        enabled_patterns,
        builtin_patterns,
        custom_patterns,
        total_matches,
        matches_today,
        top_categories,
    }))
}
