//! Recommendations API endpoints
//!
//! Manage architectural recommendations from internist analysis.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use super::AppState;
use crate::error::{AppError, OptionExt};
use crate::pipeline::{
    attempt_rebase, check_conflicts, gather_incident_summaries, analyze_patterns,
    create_recommendations, ConflictCheckResult, RebaseResult,
};
use crate::store::models::{
    Recommendation, RecommendationComment, RecommendationCommentResponse,
    RecommendationResponse, InternistAnalysisRun, InternistAnalysisRunResponse,
};

#[derive(Debug, Deserialize)]
pub struct ListRecommendationsQuery {
    pub status: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RecommendationListResponse {
    pub recommendations: Vec<RecommendationResponse>,
    pub total: i64,
}

/// GET /api/v1/recommendations - List recommendations
pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListRecommendationsQuery>,
) -> Result<Json<RecommendationListResponse>, AppError> {
    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    let (recommendations, total) = match (&query.status, &query.category) {
        (Some(status), Some(category)) => {
            let recommendations = sqlx::query_as!(
                Recommendation,
                r#"
                SELECT id, status, title, summary, analysis, proposed_changes,
                       priority, category, confidence, source_incident_ids,
                       pr_url, pr_number, pr_state, pr_branch,
                       base_commit_sha, has_conflicts, conflict_details,
                       analyzed_at, proposed_at, approved_at, approved_by,
                       completed_at, created_at, updated_at
                FROM recommendations
                WHERE status = $1 AND category = $2
                ORDER BY created_at DESC
                LIMIT $3 OFFSET $4
                "#,
                status,
                category,
                limit,
                offset
            )
            .fetch_all(&state.db)
            .await?;

            let total: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM recommendations WHERE status = $1 AND category = $2",
                status,
                category
            )
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

            (recommendations, total)
        }
        (Some(status), None) => {
            let recommendations = sqlx::query_as!(
                Recommendation,
                r#"
                SELECT id, status, title, summary, analysis, proposed_changes,
                       priority, category, confidence, source_incident_ids,
                       pr_url, pr_number, pr_state, pr_branch,
                       base_commit_sha, has_conflicts, conflict_details,
                       analyzed_at, proposed_at, approved_at, approved_by,
                       completed_at, created_at, updated_at
                FROM recommendations
                WHERE status = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
                status,
                limit,
                offset
            )
            .fetch_all(&state.db)
            .await?;

            let total: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM recommendations WHERE status = $1",
                status
            )
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

            (recommendations, total)
        }
        (None, Some(category)) => {
            let recommendations = sqlx::query_as!(
                Recommendation,
                r#"
                SELECT id, status, title, summary, analysis, proposed_changes,
                       priority, category, confidence, source_incident_ids,
                       pr_url, pr_number, pr_state, pr_branch,
                       base_commit_sha, has_conflicts, conflict_details,
                       analyzed_at, proposed_at, approved_at, approved_by,
                       completed_at, created_at, updated_at
                FROM recommendations
                WHERE category = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
                category,
                limit,
                offset
            )
            .fetch_all(&state.db)
            .await?;

            let total: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM recommendations WHERE category = $1",
                category
            )
            .fetch_one(&state.db)
            .await?
            .unwrap_or(0);

            (recommendations, total)
        }
        (None, None) => {
            let recommendations = sqlx::query_as!(
                Recommendation,
                r#"
                SELECT id, status, title, summary, analysis, proposed_changes,
                       priority, category, confidence, source_incident_ids,
                       pr_url, pr_number, pr_state, pr_branch,
                       base_commit_sha, has_conflicts, conflict_details,
                       analyzed_at, proposed_at, approved_at, approved_by,
                       completed_at, created_at, updated_at
                FROM recommendations
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                "#,
                limit,
                offset
            )
            .fetch_all(&state.db)
            .await?;

            let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM recommendations")
                .fetch_one(&state.db)
                .await?
                .unwrap_or(0);

            (recommendations, total)
        }
    };

    Ok(Json(RecommendationListResponse {
        recommendations: recommendations.into_iter().map(Into::into).collect(),
        total,
    }))
}

/// GET /api/v1/recommendations/:id - Get recommendation details
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RecommendationResponse>, AppError> {
    let recommendation = sqlx::query_as!(
        Recommendation,
        r#"
        SELECT id, status, title, summary, analysis, proposed_changes,
               priority, category, confidence, source_incident_ids,
               pr_url, pr_number, pr_state, pr_branch,
               base_commit_sha, has_conflicts, conflict_details,
               analyzed_at, proposed_at, approved_at, approved_by,
               completed_at, created_at, updated_at
        FROM recommendations
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Recommendation not found")?;

    Ok(Json(recommendation.into()))
}

#[derive(Debug, Deserialize)]
pub struct ApproveRequest {
    pub approved_by: String,
}

/// POST /api/v1/recommendations/:id/approve - Approve recommendation
pub async fn approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ApproveRequest>,
) -> Result<Json<RecommendationResponse>, AppError> {
    let recommendation = sqlx::query_as!(
        Recommendation,
        r#"
        UPDATE recommendations
        SET status = 'approved',
            approved_at = NOW(),
            approved_by = $1,
            updated_at = NOW()
        WHERE id = $2
        RETURNING id, status, title, summary, analysis, proposed_changes,
                  priority, category, confidence, source_incident_ids,
                  pr_url, pr_number, pr_state, pr_branch,
                  base_commit_sha, has_conflicts, conflict_details,
                  analyzed_at, proposed_at, approved_at, approved_by,
                  completed_at, created_at, updated_at
        "#,
        body.approved_by,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Recommendation not found")?;

    info!(recommendation_id = %id, approved_by = %body.approved_by, "Recommendation approved");

    Ok(Json(recommendation.into()))
}

#[derive(Debug, Deserialize)]
pub struct RejectRequest {
    pub reason: Option<String>,
    pub rejected_by: String,
}

/// POST /api/v1/recommendations/:id/reject - Reject recommendation
pub async fn reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<RejectRequest>,
) -> Result<Json<RecommendationResponse>, AppError> {
    // Add rejection comment if reason provided
    if let Some(reason) = &body.reason {
        sqlx::query!(
            r#"
            INSERT INTO recommendation_comments (id, recommendation_id, author, content)
            VALUES ($1, $2, $3, $4)
            "#,
            Uuid::new_v4(),
            id,
            body.rejected_by,
            format!("Rejected: {}", reason)
        )
        .execute(&state.db)
        .await?;
    }

    let recommendation = sqlx::query_as!(
        Recommendation,
        r#"
        UPDATE recommendations
        SET status = 'rejected',
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, status, title, summary, analysis, proposed_changes,
                  priority, category, confidence, source_incident_ids,
                  pr_url, pr_number, pr_state, pr_branch,
                  base_commit_sha, has_conflicts, conflict_details,
                  analyzed_at, proposed_at, approved_at, approved_by,
                  completed_at, created_at, updated_at
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Recommendation not found")?;

    info!(recommendation_id = %id, rejected_by = %body.rejected_by, "Recommendation rejected");

    Ok(Json(recommendation.into()))
}

/// POST /api/v1/recommendations/:id/check-conflicts - Check for merge conflicts
pub async fn check_conflicts_endpoint(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConflictCheckResult>, AppError> {
    // For now, return a placeholder - actual implementation requires RepoManager context
    // This would be called with the appropriate repo context in production
    let result = ConflictCheckResult {
        has_conflicts: false,
        conflicting_files: vec![],
        base_commit: String::new(),
        current_head: String::new(),
        commits_behind: 0,
        can_auto_resolve: true,
        conflict_details: vec![],
    };

    Ok(Json(result))
}

/// POST /api/v1/recommendations/:id/rebase - Attempt intelligent rebase
pub async fn rebase_endpoint(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RebaseResult>, AppError> {
    // For now, return a placeholder - actual implementation requires RepoManager and LLM
    let result = RebaseResult {
        success: true,
        new_base_commit: None,
        resolved_files: vec![],
        remaining_conflicts: vec![],
        changes_applied: vec![],
        summary: "Rebase not yet implemented".to_string(),
    };

    Ok(Json(result))
}

/// POST /api/v1/recommendations/:id/implement - Create PR from approved recommendation
pub async fn implement(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RecommendationResponse>, AppError> {
    // Verify recommendation is approved
    let rec = sqlx::query!(
        "SELECT status FROM recommendations WHERE id = $1",
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Recommendation not found")?;

    if rec.status != "approved" {
        return Err(AppError::BadRequest(
            "Recommendation must be approved before implementation".to_string(),
        ));
    }

    // Update status to implementing
    let recommendation = sqlx::query_as!(
        Recommendation,
        r#"
        UPDATE recommendations
        SET status = 'implementing',
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, status, title, summary, analysis, proposed_changes,
                  priority, category, confidence, source_incident_ids,
                  pr_url, pr_number, pr_state, pr_branch,
                  base_commit_sha, has_conflicts, conflict_details,
                  analyzed_at, proposed_at, approved_at, approved_by,
                  completed_at, created_at, updated_at
        "#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    // Note: Actual PR creation would be done by the worker
    info!(recommendation_id = %id, "Recommendation queued for implementation");

    Ok(Json(recommendation.into()))
}

#[derive(Debug, Deserialize)]
pub struct AddCommentRequest {
    pub author: String,
    pub content: String,
}

/// POST /api/v1/recommendations/:id/comments - Add discussion comment
pub async fn add_comment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddCommentRequest>,
) -> Result<Json<RecommendationCommentResponse>, AppError> {
    // Verify recommendation exists
    let _ = sqlx::query!("SELECT id FROM recommendations WHERE id = $1", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_not_found("Recommendation not found")?;

    // If we're adding a comment, mark as discussing
    sqlx::query!(
        r#"
        UPDATE recommendations
        SET status = CASE WHEN status = 'proposed' THEN 'discussing' ELSE status END,
            updated_at = NOW()
        WHERE id = $1
        "#,
        id
    )
    .execute(&state.db)
    .await?;

    let comment = sqlx::query_as!(
        RecommendationComment,
        r#"
        INSERT INTO recommendation_comments (id, recommendation_id, author, content)
        VALUES ($1, $2, $3, $4)
        RETURNING id, recommendation_id, author, content, created_at
        "#,
        Uuid::new_v4(),
        id,
        body.author,
        body.content
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(comment.into()))
}

/// GET /api/v1/recommendations/:id/comments - Get recommendation comments
pub async fn list_comments(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<RecommendationCommentResponse>>, AppError> {
    let comments = sqlx::query_as!(
        RecommendationComment,
        r#"
        SELECT id, recommendation_id, author, content, created_at
        FROM recommendation_comments
        WHERE recommendation_id = $1
        ORDER BY created_at ASC
        "#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(comments.into_iter().map(Into::into).collect()))
}

#[derive(Debug, Serialize)]
pub struct AnalyzeResponse {
    pub run_id: Uuid,
    pub status: String,
    pub message: String,
}

/// POST /api/v1/internist/analyze - Trigger manual analysis
pub async fn trigger_analysis(
    State(state): State<AppState>,
) -> Result<Json<AnalyzeResponse>, AppError> {
    // Check if internist is configured
    if !state.llm.is_internist_configured().await {
        return Err(AppError::BadRequest(
            "Internist LLM not configured. Configure it in Settings > LLM Configuration.".to_string(),
        ));
    }

    // Create analysis run record
    let run_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO internist_analysis_runs (id, status)
        VALUES ($1, 'running')
        "#,
        run_id
    )
    .execute(&state.db)
    .await?;

    // Gather incidents
    let incidents = gather_incident_summaries(&state.db, 10, 100).await?;

    if incidents.is_empty() {
        sqlx::query!(
            r#"
            UPDATE internist_analysis_runs
            SET status = 'completed',
                completed_at = NOW(),
                incident_count = 0
            WHERE id = $1
            "#,
            run_id
        )
        .execute(&state.db)
        .await?;

        return Ok(Json(AnalyzeResponse {
            run_id,
            status: "completed".to_string(),
            message: "Not enough incidents for analysis (minimum 10 required)".to_string(),
        }));
    }

    // Get internist LLM
    let internist = state.llm.internist().await.ok_or_else(|| {
        AppError::Internal("Internist LLM not available".to_string())
    })?;

    // Run analysis
    let analysis = analyze_patterns(&internist, &incidents, "").await?;

    // Create recommendations
    let created_ids = create_recommendations(&state.db, &analysis, run_id).await?;

    // Update run record
    sqlx::query!(
        r#"
        UPDATE internist_analysis_runs
        SET status = 'completed',
            completed_at = NOW(),
            incident_count = $1,
            recommendations_created = $2
        WHERE id = $3
        "#,
        incidents.len() as i32,
        created_ids.len() as i32,
        run_id
    )
    .execute(&state.db)
    .await?;

    info!(
        run_id = %run_id,
        incidents = incidents.len(),
        recommendations = created_ids.len(),
        "Internist analysis completed"
    );

    Ok(Json(AnalyzeResponse {
        run_id,
        status: "completed".to_string(),
        message: format!(
            "Analysis complete. Analyzed {} incidents, created {} recommendations.",
            incidents.len(),
            created_ids.len()
        ),
    }))
}

/// GET /api/v1/internist/runs - Get recent analysis runs
pub async fn list_analysis_runs(
    State(state): State<AppState>,
    Query(query): Query<ListRecommendationsQuery>,
) -> Result<Json<Vec<InternistAnalysisRunResponse>>, AppError> {
    let limit = query.limit.unwrap_or(20).min(100);

    let runs = sqlx::query_as!(
        InternistAnalysisRun,
        r#"
        SELECT id, status, incident_count, recommendations_created,
               started_at, completed_at, error, input_tokens, output_tokens, cost_usd
        FROM internist_analysis_runs
        ORDER BY started_at DESC
        LIMIT $1
        "#,
        limit
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(runs.into_iter().map(Into::into).collect()))
}

// ============================================================================
// Internist Chat Endpoints
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed_update: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub intent: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub message: ChatMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready_for_approval: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<String>>,
}

/// POST /api/v1/recommendations/:id/chat - Send message to Internist
pub async fn chat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    // Verify recommendation exists and get context
    let recommendation = sqlx::query_as!(
        Recommendation,
        r#"
        SELECT id, status, title, summary, analysis, proposed_changes,
               priority, category, confidence, source_incident_ids,
               pr_url, pr_number, pr_state, pr_branch,
               base_commit_sha, has_conflicts, conflict_details,
               analyzed_at, proposed_at, approved_at, approved_by,
               completed_at, created_at, updated_at
        FROM recommendations
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Recommendation not found")?;

    // Get internist LLM
    let internist = state.llm.internist().await.ok_or_else(|| {
        AppError::BadRequest("Internist LLM not configured".to_string())
    })?;

    // Get existing chat history
    let history = sqlx::query_as!(
        RecommendationChatMessage,
        r#"
        SELECT id, recommendation_id, role, content, proposed_update, created_at
        FROM recommendation_chat_messages
        WHERE recommendation_id = $1
        ORDER BY created_at ASC
        "#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    // Store user message
    let user_msg_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO recommendation_chat_messages (id, recommendation_id, role, content)
        VALUES ($1, $2, 'user', $3)
        "#,
        user_msg_id,
        id,
        body.message
    )
    .execute(&state.db)
    .await?;

    // Build prompt for Internist
    let prompt = build_chat_prompt(&recommendation, &history, &body.message, body.intent.as_deref());

    // Call Internist LLM
    let system = Some("You are an experienced software architect helping refine recommendations for fixing systemic issues. Be helpful, concise, and provide actionable suggestions.");
    let llm_response = internist.complete(&prompt, system, "internist_chat", None, None).await?;
    let response = llm_response.content;

    // Parse response for any proposed updates
    let (content, proposed_update) = parse_chat_response(&response);

    // Store assistant message
    let assistant_msg_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query!(
        r#"
        INSERT INTO recommendation_chat_messages (id, recommendation_id, role, content, proposed_update)
        VALUES ($1, $2, 'assistant', $3, $4)
        "#,
        assistant_msg_id,
        id,
        content,
        proposed_update
    )
    .execute(&state.db)
    .await?;

    let message = ChatMessage {
        id: assistant_msg_id,
        role: "assistant".to_string(),
        content,
        timestamp: now,
        proposed_update,
    };

    // Determine if ready for approval based on response
    let ready_for_approval = message.content.to_lowercase().contains("ready for implementation")
        || message.content.to_lowercase().contains("ready to approve");

    Ok(Json(ChatResponse {
        message,
        ready_for_approval: Some(ready_for_approval),
        suggestions: None,
    }))
}

/// GET /api/v1/recommendations/:id/chat - Get chat history
pub async fn get_chat_history(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<ChatMessage>>, AppError> {
    // Verify recommendation exists
    let _ = sqlx::query!("SELECT id FROM recommendations WHERE id = $1", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_not_found("Recommendation not found")?;

    let messages = sqlx::query_as!(
        RecommendationChatMessage,
        r#"
        SELECT id, recommendation_id, role, content, proposed_update, created_at
        FROM recommendation_chat_messages
        WHERE recommendation_id = $1
        ORDER BY created_at ASC
        "#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(messages.into_iter().map(|m| ChatMessage {
        id: m.id,
        role: m.role,
        content: m.content,
        timestamp: m.created_at.unwrap_or_else(chrono::Utc::now),
        proposed_update: m.proposed_update,
    }).collect()))
}

#[derive(Debug, Deserialize)]
pub struct ApplySuggestionRequest {
    pub message_id: Uuid,
}

/// POST /api/v1/recommendations/:id/apply-suggestion - Apply suggestion from chat
pub async fn apply_suggestion(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ApplySuggestionRequest>,
) -> Result<Json<RecommendationResponse>, AppError> {
    // Get the chat message with the proposed update
    let message = sqlx::query_as!(
        RecommendationChatMessage,
        r#"
        SELECT id, recommendation_id, role, content, proposed_update, created_at
        FROM recommendation_chat_messages
        WHERE id = $1 AND recommendation_id = $2
        "#,
        body.message_id,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Chat message not found")?;

    let proposed_update = message.proposed_update
        .ok_or_else(|| AppError::BadRequest("Message has no proposed update".to_string()))?;

    // Apply the updates
    let update: serde_json::Value = proposed_update;

    // Build dynamic update
    let mut updates = vec![];

    if let Some(title) = update.get("title").and_then(|v| v.as_str()) {
        updates.push(format!("title = '{}'", title.replace('\'', "''")));
    }
    if let Some(summary) = update.get("summary").and_then(|v| v.as_str()) {
        updates.push(format!("summary = '{}'", summary.replace('\'', "''")));
    }
    if let Some(priority) = update.get("priority").and_then(|v| v.as_str()) {
        updates.push(format!("priority = '{}'", priority));
    }
    if let Some(analysis) = update.get("analysis") {
        updates.push(format!("analysis = '{}'", analysis.to_string().replace('\'', "''")));
    }
    if let Some(proposed_changes) = update.get("proposed_changes") {
        updates.push(format!("proposed_changes = '{}'", proposed_changes.to_string().replace('\'', "''")));
    }

    if !updates.is_empty() {
        updates.push("updated_at = NOW()".to_string());
        let update_query = format!(
            "UPDATE recommendations SET {} WHERE id = $1",
            updates.join(", ")
        );
        sqlx::query(&update_query)
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    // Fetch and return updated recommendation
    let recommendation = sqlx::query_as!(
        Recommendation,
        r#"
        SELECT id, status, title, summary, analysis, proposed_changes,
               priority, category, confidence, source_incident_ids,
               pr_url, pr_number, pr_state, pr_branch,
               base_commit_sha, has_conflicts, conflict_details,
               analyzed_at, proposed_at, approved_at, approved_by,
               completed_at, created_at, updated_at
        FROM recommendations
        WHERE id = $1
        "#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    info!(recommendation_id = %id, "Applied suggestion from chat");

    Ok(Json(recommendation.into()))
}

#[derive(Debug, Serialize)]
pub struct ConvertToIncidentResponse {
    pub incident_id: Uuid,
    pub message: String,
}

/// POST /api/v1/recommendations/:id/convert-to-incident - Convert to triage ticket
pub async fn convert_to_incident(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConvertToIncidentResponse>, AppError> {
    // Get recommendation
    let recommendation = sqlx::query_as!(
        Recommendation,
        r#"
        SELECT id, status, title, summary, analysis, proposed_changes,
               priority, category, confidence, source_incident_ids,
               pr_url, pr_number, pr_state, pr_branch,
               base_commit_sha, has_conflicts, conflict_details,
               analyzed_at, proposed_at, approved_at, approved_by,
               completed_at, created_at, updated_at
        FROM recommendations
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_not_found("Recommendation not found")?;

    // Get the first project from source incidents (or default project)
    let project_id: Option<Uuid> = if let Some(ref incident_ids) = recommendation.source_incident_ids {
        if let Some(first_id) = incident_ids.first() {
            let result = sqlx::query!("SELECT project_id FROM incidents WHERE id = $1", first_id)
                .fetch_optional(&state.db)
                .await?;
            result.map(|r| r.project_id)
        } else {
            None
        }
    } else {
        None
    };

    let project_id = project_id.ok_or_else(|| {
        AppError::BadRequest("No project found for recommendation".to_string())
    })?;

    // Create a new incident from the recommendation
    let incident_id = Uuid::new_v4();
    let fingerprint = format!("recommendation-{}", id);

    // Build the incident title with context
    let title = format!(
        "[Internist] {}",
        recommendation.title
    );

    sqlx::query!(
        r#"
        INSERT INTO incidents (
            id, project_id, fingerprint, title, state,
            occurrence_count, first_seen, last_seen,
            has_api_contract_changes
        )
        VALUES ($1, $2, $3, $4, 'pending_fix', 1, NOW(), NOW(), $5)
        "#,
        incident_id,
        project_id,
        fingerprint,
        title,
        recommendation.category.as_deref() == Some("api_contract")
    )
    .execute(&state.db)
    .await?;

    // Store the detailed fix information in the sample_location field
    // (This is where detailed context is typically stored)
    let analysis = recommendation.analysis.as_ref()
        .map(|a| serde_json::from_value::<serde_json::Value>(a.clone()).ok())
        .flatten();

    let root_cause = analysis.as_ref()
        .and_then(|a| a.get("root_cause"))
        .and_then(|v| v.as_str())
        .unwrap_or("See recommendation analysis");

    let impact = analysis.as_ref()
        .and_then(|a| a.get("impact_assessment"))
        .and_then(|v| v.as_str())
        .unwrap_or("See recommendation for details");

    let proposed_changes = recommendation.proposed_changes.as_ref()
        .map(|p| serde_json::from_value::<Vec<serde_json::Value>>(p.clone()).ok())
        .flatten()
        .unwrap_or_default();

    let fix_approach = proposed_changes.iter()
        .filter_map(|c| {
            let file = c.get("file_path").and_then(|v| v.as_str()).unwrap_or("unknown");
            let desc = c.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let rationale = c.get("rationale").and_then(|v| v.as_str()).unwrap_or("");
            Some(format!("File: {}\nChange: {}\nRationale: {}", file, desc, rationale))
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let sample_content = format!(
        "=== INTERNIST RECOMMENDATION ===\n\n\
         Summary: {}\n\n\
         Root Cause: {}\n\n\
         Impact: {}\n\n\
         === PROPOSED CHANGES ===\n\n{}\n\n\
         === VERIFICATION ===\n\
         This architectural fix requires extended monitoring (24h recommended).",
        recommendation.summary.as_deref().unwrap_or(&recommendation.title),
        root_cause,
        impact,
        fix_approach
    );

    // Update with sample location
    sqlx::query!(
        "UPDATE incidents SET sample_location = $1 WHERE id = $2",
        sample_content,
        incident_id
    )
    .execute(&state.db)
    .await?;

    // Link recommendation to incident
    sqlx::query!(
        r#"
        INSERT INTO recommendation_incidents (recommendation_id, incident_id, relevance_score)
        VALUES ($1, $2, 1.0)
        ON CONFLICT DO NOTHING
        "#,
        id,
        incident_id
    )
    .execute(&state.db)
    .await?;

    // Update recommendation status
    sqlx::query!(
        r#"
        UPDATE recommendations
        SET status = 'implementing',
            updated_at = NOW()
        WHERE id = $1
        "#,
        id
    )
    .execute(&state.db)
    .await?;

    info!(recommendation_id = %id, incident_id = %incident_id, "Converted recommendation to incident");

    Ok(Json(ConvertToIncidentResponse {
        incident_id,
        message: format!(
            "Created triage ticket '{}' with comprehensive fix instructions",
            recommendation.title
        ),
    }))
}

// Helper structs and functions

#[derive(Debug)]
struct RecommendationChatMessage {
    id: Uuid,
    recommendation_id: Uuid,
    role: String,
    content: String,
    proposed_update: Option<serde_json::Value>,
    created_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn build_chat_prompt(
    recommendation: &Recommendation,
    history: &[RecommendationChatMessage],
    user_message: &str,
    intent: Option<&str>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str("You are an experienced software architect helping refine a recommendation for fixing systemic issues.\n\n");

    prompt.push_str("## Current Recommendation\n");
    prompt.push_str(&format!("**Title:** {}\n", recommendation.title));
    if let Some(summary) = &recommendation.summary {
        prompt.push_str(&format!("**Summary:** {}\n", summary));
    }
    if let Some(analysis) = &recommendation.analysis {
        prompt.push_str(&format!("**Analysis:** {}\n", analysis));
    }
    if let Some(priority) = &recommendation.priority {
        prompt.push_str(&format!("**Priority:** {}\n", priority));
    }
    prompt.push_str("\n");

    // Add chat history
    if !history.is_empty() {
        prompt.push_str("## Previous Discussion\n");
        for msg in history {
            prompt.push_str(&format!("**{}:** {}\n\n", msg.role, msg.content));
        }
    }

    // Add current message
    prompt.push_str(&format!("## User Message\n{}\n\n", user_message));

    // Add intent-specific instructions
    match intent {
        Some("rewrite") => {
            prompt.push_str("The user wants you to REWRITE this recommendation. Provide a complete new version.\n");
            prompt.push_str("If you propose changes, include them in a JSON block marked with ```json ... ``` containing fields like title, summary, analysis, priority, and/or proposed_changes.\n");
        }
        Some("refine") => {
            prompt.push_str("The user wants you to REFINE specific parts of this recommendation.\n");
            prompt.push_str("If you propose changes, include them in a JSON block marked with ```json ... ```.\n");
        }
        Some("approve") => {
            prompt.push_str("The user is considering APPROVING this recommendation. Summarize why it's ready or what might still need work.\n");
        }
        _ => {
            prompt.push_str("Respond helpfully. If you suggest changes, include them in a JSON block marked with ```json ... ```.\n");
        }
    }

    prompt.push_str("\nBe concise but thorough. Use markdown formatting for readability.\n");

    prompt
}

fn parse_chat_response(response: &str) -> (String, Option<serde_json::Value>) {
    // Look for JSON blocks in the response
    let json_pattern = regex::Regex::new(r"```json\s*([\s\S]*?)\s*```").ok();

    if let Some(re) = json_pattern {
        if let Some(captures) = re.captures(response) {
            if let Some(json_str) = captures.get(1) {
                if let Ok(proposed) = serde_json::from_str::<serde_json::Value>(json_str.as_str()) {
                    // Remove the JSON block from the content
                    let content = re.replace(response, "").trim().to_string();
                    return (content, Some(proposed));
                }
            }
        }
    }

    (response.to_string(), None)
}
