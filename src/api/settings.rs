//! Settings API endpoints
//!
//! Endpoints for configuring LLM providers and viewing usage statistics.

use axum::{extract::State, Json};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::AppState;
use crate::error::AppError;
use crate::integrations::llm;

// ============================================================================
// LLM Configuration
// ============================================================================

#[derive(Debug, Serialize)]
pub struct LlmConfigResponse {
    pub configured: bool,
    pub reader: Option<LlmProviderInfo>,
    pub writer: Option<LlmProviderInfo>,
}

#[derive(Debug, Serialize)]
pub struct LlmProviderInfo {
    pub provider: String,
    pub model: String,
    pub has_api_key: bool,
    pub base_url: Option<String>,
}

/// GET /api/v1/settings/llm - Get current LLM configuration
pub async fn get_llm(State(state): State<AppState>) -> Result<Json<LlmConfigResponse>, AppError> {
    let config = sqlx::query!(
        r#"
        SELECT reader_provider, reader_model, reader_api_key, reader_base_url,
               writer_provider, writer_model, writer_api_key, writer_base_url
        FROM llm_config
        ORDER BY created_at DESC
        LIMIT 1
        "#
    )
    .fetch_optional(&state.db)
    .await?;

    match config {
        Some(c) => Ok(Json(LlmConfigResponse {
            configured: true,
            reader: Some(LlmProviderInfo {
                provider: c.reader_provider,
                model: c.reader_model,
                has_api_key: c.reader_api_key.is_some(),
                base_url: c.reader_base_url,
            }),
            writer: Some(LlmProviderInfo {
                provider: c.writer_provider,
                model: c.writer_model,
                has_api_key: c.writer_api_key.is_some(),
                base_url: c.writer_base_url,
            }),
        })),
        None => Ok(Json(LlmConfigResponse {
            configured: false,
            reader: None,
            writer: None,
        })),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateLlmRequest {
    pub reader_provider: String,
    pub reader_model: String,
    pub reader_api_key: Option<String>,
    pub reader_base_url: Option<String>,

    pub writer_provider: String,
    pub writer_model: String,
    pub writer_api_key: Option<String>,
    pub writer_base_url: Option<String>,
}

/// PUT /api/v1/settings/llm - Update LLM configuration
pub async fn update_llm(
    State(state): State<AppState>,
    Json(req): Json<UpdateLlmRequest>,
) -> Result<Json<LlmConfigResponse>, AppError> {
    // Check if config exists
    let existing = sqlx::query!("SELECT id FROM llm_config LIMIT 1")
        .fetch_optional(&state.db)
        .await?;

    if let Some(row) = existing {
        // Update existing config
        sqlx::query!(
            r#"
            UPDATE llm_config SET
                reader_provider = $1,
                reader_model = $2,
                reader_api_key = COALESCE($3, reader_api_key),
                reader_base_url = $4,
                writer_provider = $5,
                writer_model = $6,
                writer_api_key = COALESCE($7, writer_api_key),
                writer_base_url = $8,
                updated_at = NOW()
            WHERE id = $9
            "#,
            req.reader_provider,
            req.reader_model,
            req.reader_api_key,
            req.reader_base_url,
            req.writer_provider,
            req.writer_model,
            req.writer_api_key,
            req.writer_base_url,
            row.id,
        )
        .execute(&state.db)
        .await?;
    } else {
        // Insert new config
        sqlx::query!(
            r#"
            INSERT INTO llm_config
            (id, reader_provider, reader_model, reader_api_key, reader_base_url,
             writer_provider, writer_model, writer_api_key, writer_base_url)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            Uuid::new_v4(),
            req.reader_provider,
            req.reader_model,
            req.reader_api_key,
            req.reader_base_url,
            req.writer_provider,
            req.writer_model,
            req.writer_api_key,
            req.writer_base_url,
        )
        .execute(&state.db)
        .await?;
    }

    // Reload LLM manager
    state.llm.reload().await?;

    get_llm(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct VerifyLlmRequest {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VerifyLlmResponse {
    pub valid: bool,
    pub error: Option<String>,
    pub models: Vec<llm::Model>,
}

/// POST /api/v1/settings/llm/verify - Verify LLM credentials
pub async fn verify_llm(Json(req): Json<VerifyLlmRequest>) -> Json<VerifyLlmResponse> {
    match llm::list_models(&req.provider, req.api_key.as_deref(), req.base_url.as_deref()).await {
        Ok(models) => Json(VerifyLlmResponse {
            valid: true,
            error: None,
            models,
        }),
        Err(e) => Json(VerifyLlmResponse {
            valid: false,
            error: Some(e.to_string()),
            models: vec![],
        }),
    }
}

#[derive(Debug, Deserialize)]
pub struct ListModelsRequest {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

/// POST /api/v1/settings/llm/models - List available models for a provider
pub async fn list_models(
    Json(req): Json<ListModelsRequest>,
) -> Result<Json<Vec<llm::Model>>, AppError> {
    let models =
        llm::list_models(&req.provider, req.api_key.as_deref(), req.base_url.as_deref()).await?;
    Ok(Json(models))
}

// ============================================================================
// Usage Statistics
// ============================================================================

#[derive(Debug, Serialize)]
pub struct UsageSummary {
    pub period: String,
    pub reader: UsageStats,
    pub writer: UsageStats,
    pub total: UsageStats,
    pub by_project: Vec<ProjectUsage>,
    pub by_day: Vec<DailyUsage>,
}

#[derive(Debug, Serialize, Default)]
pub struct UsageStats {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: Decimal,
}

#[derive(Debug, Serialize)]
pub struct ProjectUsage {
    pub project_id: Uuid,
    pub owner: String,
    pub repo: String,
    pub cost_usd: Decimal,
}

#[derive(Debug, Serialize)]
pub struct DailyUsage {
    pub date: NaiveDate,
    pub reader_cost: Decimal,
    pub writer_cost: Decimal,
}

/// GET /api/v1/usage/summary - Get usage summary for last 7 days
pub async fn usage_summary(
    State(state): State<AppState>,
) -> Result<Json<UsageSummary>, AppError> {
    let since = chrono::Utc::now() - chrono::Duration::days(7);

    // Reader stats
    let reader = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as "requests!",
            COALESCE(SUM(input_tokens), 0) as "input_tokens!",
            COALESCE(SUM(output_tokens), 0) as "output_tokens!",
            COALESCE(SUM(cost_usd), 0) as "cost_usd!"
        FROM llm_usage
        WHERE role = 'reader' AND created_at > $1
        "#,
        since
    )
    .fetch_one(&state.db)
    .await?;

    // Writer stats
    let writer = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as "requests!",
            COALESCE(SUM(input_tokens), 0) as "input_tokens!",
            COALESCE(SUM(output_tokens), 0) as "output_tokens!",
            COALESCE(SUM(cost_usd), 0) as "cost_usd!"
        FROM llm_usage
        WHERE role = 'writer' AND created_at > $1
        "#,
        since
    )
    .fetch_one(&state.db)
    .await?;

    let total = UsageStats {
        requests: reader.requests + writer.requests,
        input_tokens: reader.input_tokens + writer.input_tokens,
        output_tokens: reader.output_tokens + writer.output_tokens,
        cost_usd: reader.cost_usd + writer.cost_usd,
    };

    // Usage by project
    let by_project = sqlx::query!(
        r#"
        SELECT
            p.id as "project_id!",
            p.github_owner as "owner!",
            p.github_repo as "repo!",
            COALESCE(SUM(u.cost_usd), 0) as "cost_usd!"
        FROM projects p
        LEFT JOIN llm_usage u ON u.project_id = p.id AND u.created_at > $1
        GROUP BY p.id, p.github_owner, p.github_repo
        HAVING COALESCE(SUM(u.cost_usd), 0) > 0
        ORDER BY "cost_usd!" DESC
        LIMIT 10
        "#,
        since
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r| ProjectUsage {
        project_id: r.project_id,
        owner: r.owner,
        repo: r.repo,
        cost_usd: r.cost_usd,
    })
    .collect();

    // Usage by day
    let by_day = sqlx::query!(
        r#"
        SELECT
            DATE(created_at) as "date!",
            COALESCE(SUM(CASE WHEN role = 'reader' THEN cost_usd ELSE 0 END), 0) as "reader_cost!",
            COALESCE(SUM(CASE WHEN role = 'writer' THEN cost_usd ELSE 0 END), 0) as "writer_cost!"
        FROM llm_usage
        WHERE created_at > $1
        GROUP BY DATE(created_at)
        ORDER BY "date!" ASC
        "#,
        since
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r| DailyUsage {
        date: r.date,
        reader_cost: r.reader_cost,
        writer_cost: r.writer_cost,
    })
    .collect();

    Ok(Json(UsageSummary {
        period: "last_7_days".into(),
        reader: UsageStats {
            requests: reader.requests,
            input_tokens: reader.input_tokens,
            output_tokens: reader.output_tokens,
            cost_usd: reader.cost_usd,
        },
        writer: UsageStats {
            requests: writer.requests,
            input_tokens: writer.input_tokens,
            output_tokens: writer.output_tokens,
            cost_usd: writer.cost_usd,
        },
        total,
        by_project,
        by_day,
    }))
}

#[derive(Debug, Deserialize)]
pub struct UsageDetailsQuery {
    pub days: Option<i64>,
    pub project_id: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct UsageDetail {
    pub id: Uuid,
    pub incident_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub role: String,
    pub provider: String,
    pub model: String,
    pub operation: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_usd: Option<Decimal>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// GET /api/v1/usage/details - Get detailed usage records
pub async fn usage_details(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<UsageDetailsQuery>,
) -> Result<Json<Vec<UsageDetail>>, AppError> {
    let days = query.days.unwrap_or(7);
    let limit = query.limit.unwrap_or(100).min(1000);
    let since = chrono::Utc::now() - chrono::Duration::days(days);

    let details = if let Some(project_id) = query.project_id {
        sqlx::query_as!(
            UsageDetail,
            r#"
            SELECT id, incident_id, project_id, role, provider, model, operation,
                   input_tokens, output_tokens, cost_usd, created_at as "created_at!"
            FROM llm_usage
            WHERE created_at > $1 AND project_id = $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
            since,
            project_id,
            limit
        )
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as!(
            UsageDetail,
            r#"
            SELECT id, incident_id, project_id, role, provider, model, operation,
                   input_tokens, output_tokens, cost_usd, created_at as "created_at!"
            FROM llm_usage
            WHERE created_at > $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            since,
            limit
        )
        .fetch_all(&state.db)
        .await?
    };

    Ok(Json(details))
}
