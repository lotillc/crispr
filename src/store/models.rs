use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// Instance configuration
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct InstanceConfig {
    pub id: Uuid,
    pub environment: String,
    pub display_name: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// GitHub connection
#[derive(Debug, Clone, FromRow)]
pub struct GitHubConnection {
    pub id: Uuid,
    pub mode: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub user_login: Option<String>,
    pub user_id: Option<i64>,
    pub scopes: Option<Vec<String>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Slack connection
#[derive(Debug, Clone, FromRow)]
pub struct SlackConnection {
    pub id: Uuid,
    pub access_token: String,
    pub team_id: String,
    pub team_name: Option<String>,
    pub channel_id: Option<String>,
    pub channel_name: Option<String>,
    pub bot_user_id: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Project (watched repository)
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Project {
    pub id: Uuid,
    pub github_owner: String,
    pub github_repo: String,
    pub default_branch: Option<String>,
    pub language: Option<String>,
    pub enabled: Option<bool>,
    pub auto_registered: Option<bool>,
    pub settings: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Incident (unique error)
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Incident {
    pub id: Uuid,
    pub project_id: Uuid,
    pub fingerprint: String,
    pub title: Option<String>,
    pub state: Option<String>,
    pub severity: Option<String>,
    pub occurrence_count: Option<i32>,
    pub first_seen: Option<DateTime<Utc>>,
    pub last_seen: Option<DateTime<Utc>>,
    pub cooldown_until: Option<DateTime<Utc>>,
    pub sample_location: Option<String>,
    pub fix_attempts: Option<i32>,
    pub pr_url: Option<String>,
    pub pr_number: Option<i32>,
    pub pr_state: Option<String>,
    /// Whether tests were generated for this incident's fix
    pub tests_generated: Option<bool>,
    /// Number of tests generated
    pub test_count: Option<i32>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Sample (stored log payload)
#[derive(Debug, Clone, FromRow)]
pub struct Sample {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub payload: Vec<u8>,
    pub created_at: Option<DateTime<Utc>>,
}

// Generated test (unit test generated alongside a fix)
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct GeneratedTestRecord {
    pub id: Uuid,
    pub incident_id: Uuid,
    /// Suggested test file path (e.g., "src/user.test.ts")
    pub file_path: String,
    /// The test source code
    pub content: String,
    /// Description of what the test verifies
    pub description: Option<String>,
    /// Test framework used (e.g., "jest", "pytest", "go test")
    pub framework: String,
    /// Whether the test was included in the PR
    pub included_in_pr: Option<bool>,
    /// SHA of the commit that included this test
    pub commit_sha: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

// Activity log entry
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ActivityLog {
    pub id: Uuid,
    pub incident_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub action: String,
    pub details: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}

// LLM configuration
#[derive(Debug, Clone, FromRow)]
pub struct LlmConfig {
    pub id: Uuid,
    pub reader_provider: String,
    pub reader_model: String,
    pub reader_api_key: Option<String>,
    pub reader_base_url: Option<String>,
    pub writer_provider: String,
    pub writer_model: String,
    pub writer_api_key: Option<String>,
    pub writer_base_url: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// LLM usage record
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct LlmUsage {
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
    pub created_at: Option<DateTime<Utc>>,
}

// LLM pricing override
#[derive(Debug, Clone, FromRow)]
pub struct LlmPricingOverride {
    pub id: Uuid,
    pub provider: String,
    pub model: String,
    pub input_per_mtok: Decimal,
    pub output_per_mtok: Decimal,
    pub created_at: Option<DateTime<Utc>>,
}

// API response types

#[derive(Debug, Serialize)]
pub struct UsageStats {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: Decimal,
}

impl Default for UsageStats {
    fn default() -> Self {
        Self {
            requests: 0,
            input_tokens: 0,
            output_tokens: 0,
            cost_usd: Decimal::ZERO,
        }
    }
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

// Request/response types for API

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub github_owner: String,
    pub github_repo: String,
    pub default_branch: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub default_branch: Option<String>,
    pub language: Option<String>,
    pub enabled: Option<bool>,
    pub settings: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub github_owner: String,
    pub github_repo: String,
    pub default_branch: String,
    pub language: Option<String>,
    pub enabled: bool,
    pub auto_registered: bool,
    pub settings: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            github_owner: p.github_owner,
            github_repo: p.github_repo,
            default_branch: p.default_branch.unwrap_or_else(|| "main".to_string()),
            language: p.language,
            enabled: p.enabled.unwrap_or(true),
            auto_registered: p.auto_registered.unwrap_or(false),
            settings: p.settings.unwrap_or_else(|| serde_json::json!({})),
            created_at: p.created_at.unwrap_or_else(Utc::now),
            updated_at: p.updated_at.unwrap_or_else(Utc::now),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct IncidentResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub fingerprint: String,
    pub title: Option<String>,
    pub state: String,
    pub severity: Option<String>,
    pub occurrence_count: i32,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub cooldown_until: Option<DateTime<Utc>>,
    pub fix_attempts: i32,
    pub pr_url: Option<String>,
    pub pr_number: Option<i32>,
    pub pr_state: Option<String>,
    /// Whether tests were generated for this incident's fix
    pub tests_generated: bool,
    /// Number of tests generated
    pub test_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Incident> for IncidentResponse {
    fn from(i: Incident) -> Self {
        Self {
            id: i.id,
            project_id: i.project_id,
            fingerprint: i.fingerprint,
            title: i.title,
            state: i.state.unwrap_or_else(|| "detected".to_string()),
            severity: i.severity,
            occurrence_count: i.occurrence_count.unwrap_or(1),
            first_seen: i.first_seen.unwrap_or_else(Utc::now),
            last_seen: i.last_seen.unwrap_or_else(Utc::now),
            cooldown_until: i.cooldown_until,
            fix_attempts: i.fix_attempts.unwrap_or(0),
            pr_url: i.pr_url,
            pr_number: i.pr_number,
            pr_state: i.pr_state,
            tests_generated: i.tests_generated.unwrap_or(false),
            test_count: i.test_count.unwrap_or(0),
            created_at: i.created_at.unwrap_or_else(Utc::now),
            updated_at: i.updated_at.unwrap_or_else(Utc::now),
        }
    }
}

/// Response type for generated test records
#[derive(Debug, Serialize)]
pub struct GeneratedTestResponse {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub file_path: String,
    pub content: String,
    pub description: Option<String>,
    pub framework: String,
    pub included_in_pr: bool,
    pub commit_sha: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<GeneratedTestRecord> for GeneratedTestResponse {
    fn from(t: GeneratedTestRecord) -> Self {
        Self {
            id: t.id,
            incident_id: t.incident_id,
            file_path: t.file_path,
            content: t.content,
            description: t.description,
            framework: t.framework,
            included_in_pr: t.included_in_pr.unwrap_or(false),
            commit_sha: t.commit_sha,
            created_at: t.created_at.unwrap_or_else(Utc::now),
        }
    }
}
