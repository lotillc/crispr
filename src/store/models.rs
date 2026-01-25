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
    /// ID of the matched error pattern (if any)
    pub matched_pattern_id: Option<Uuid>,
    /// Whether this incident was auto-triaged via pattern matching
    pub pattern_auto_triaged: Option<bool>,
    /// Timestamp when incident was queued for surgical lock
    pub queued_at: Option<DateTime<Utc>>,
    /// Position in queue for repo lock
    pub queue_position: Option<i32>,
    /// Current verification round
    pub verification_round: Option<i32>,
    /// Verification status: 'verifying' | 'verified' | 'recurred'
    pub verification_status: Option<String>,
    /// When verification completed successfully
    pub verified_at: Option<DateTime<Utc>>,
    /// When the PR was merged
    pub pr_merged_at: Option<DateTime<Utc>>,
    /// LLM-recommended verification monitoring duration in hours
    pub verification_duration_hours: Option<i32>,
    /// When verification monitoring started
    pub verification_started_at: Option<DateTime<Utc>>,
    /// When verification monitoring is expected to complete
    pub verification_ends_at: Option<DateTime<Utc>>,
    /// Count of new occurrences during verification (indicates recurrence)
    pub recurrence_count: Option<i32>,
    /// When recurrence was first detected
    pub recurrence_detected_at: Option<DateTime<Utc>>,
    /// Whether this incident's fix modifies API contract files
    pub has_api_contract_changes: Option<bool>,
    /// Details about API contract changes (if any)
    pub api_contract_change_details: Option<serde_json::Value>,
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
    pub internist_provider: Option<String>,
    pub internist_model: Option<String>,
    pub internist_api_key: Option<String>,
    pub internist_base_url: Option<String>,
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
    /// ID of the matched error pattern (if any)
    pub matched_pattern_id: Option<Uuid>,
    /// Whether this incident was auto-triaged via pattern matching
    pub pattern_auto_triaged: bool,
    /// Queue position if waiting for surgical lock
    pub queue_position: Option<i32>,
    /// When queued for surgical lock
    pub queued_at: Option<DateTime<Utc>>,
    /// Current verification round
    pub verification_round: Option<i32>,
    /// Verification status
    pub verification_status: Option<String>,
    /// When verification completed
    pub verified_at: Option<DateTime<Utc>>,
    /// When PR was merged
    pub pr_merged_at: Option<DateTime<Utc>>,
    /// LLM-recommended monitoring duration in hours
    pub verification_duration_hours: Option<i32>,
    /// When verification monitoring started
    pub verification_started_at: Option<DateTime<Utc>>,
    /// When verification monitoring is expected to complete
    pub verification_ends_at: Option<DateTime<Utc>>,
    /// Count of new occurrences during verification
    pub recurrence_count: Option<i32>,
    /// When recurrence was first detected
    pub recurrence_detected_at: Option<DateTime<Utc>>,
    /// Whether this incident's fix modifies API contract files
    pub has_api_contract_changes: bool,
    /// Details about API contract changes (if any)
    pub api_contract_change_details: Option<serde_json::Value>,
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
            matched_pattern_id: i.matched_pattern_id,
            pattern_auto_triaged: i.pattern_auto_triaged.unwrap_or(false),
            queue_position: i.queue_position,
            queued_at: i.queued_at,
            verification_round: i.verification_round,
            verification_status: i.verification_status,
            verified_at: i.verified_at,
            pr_merged_at: i.pr_merged_at,
            verification_duration_hours: i.verification_duration_hours,
            verification_started_at: i.verification_started_at,
            verification_ends_at: i.verification_ends_at,
            recurrence_count: i.recurrence_count,
            recurrence_detected_at: i.recurrence_detected_at,
            has_api_contract_changes: i.has_api_contract_changes.unwrap_or(false),
            api_contract_change_details: i.api_contract_change_details,
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

// Repo surgical lock for per-repo serialization
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct RepoSurgicalLock {
    pub id: Uuid,
    pub project_id: Uuid,
    pub incident_id: Uuid,
    pub worker_id: String,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

// Verification round record
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct VerificationRoundRecord {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub round_number: i32,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub occurrence_count_start: i32,
    pub occurrence_count_end: Option<i32>,
    pub duration_hours: i32,
}

// Incident relations for tracking overlapping fixes
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct IncidentRelation {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub related_incident_id: Uuid,
    pub relation_type: String,
    pub confidence: Option<f32>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Response type for incident relations
#[derive(Debug, Serialize)]
pub struct IncidentRelationResponse {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub related_incident_id: Uuid,
    pub relation_type: String,
    pub confidence: Option<f32>,
    pub created_at: DateTime<Utc>,
}

impl From<IncidentRelation> for IncidentRelationResponse {
    fn from(r: IncidentRelation) -> Self {
        Self {
            id: r.id,
            incident_id: r.incident_id,
            related_incident_id: r.related_incident_id,
            relation_type: r.relation_type,
            confidence: r.confidence,
            created_at: r.created_at.unwrap_or_else(Utc::now),
        }
    }
}

// Architectural recommendation from internist analysis
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Recommendation {
    pub id: Uuid,
    pub status: String,
    pub title: String,
    pub summary: Option<String>,
    pub analysis: Option<serde_json::Value>,
    pub proposed_changes: Option<serde_json::Value>,
    pub priority: Option<String>,
    pub category: Option<String>,
    pub confidence: Option<f32>,
    pub source_incident_ids: Option<Vec<Uuid>>,
    pub pr_url: Option<String>,
    pub pr_number: Option<i32>,
    pub pr_state: Option<String>,
    pub pr_branch: Option<String>,
    pub base_commit_sha: Option<String>,
    pub has_conflicts: Option<bool>,
    pub conflict_details: Option<serde_json::Value>,
    pub analyzed_at: Option<DateTime<Utc>>,
    pub proposed_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
    pub approved_by: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct RecommendationResponse {
    pub id: Uuid,
    pub status: String,
    pub title: String,
    pub summary: Option<String>,
    pub analysis: Option<serde_json::Value>,
    pub proposed_changes: Option<serde_json::Value>,
    pub priority: String,
    pub category: Option<String>,
    pub confidence: Option<f32>,
    pub source_incident_ids: Vec<Uuid>,
    pub pr_url: Option<String>,
    pub pr_number: Option<i32>,
    pub pr_state: Option<String>,
    pub pr_branch: Option<String>,
    pub base_commit_sha: Option<String>,
    pub has_conflicts: bool,
    pub conflict_details: Option<serde_json::Value>,
    pub analyzed_at: Option<DateTime<Utc>>,
    pub proposed_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
    pub approved_by: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Recommendation> for RecommendationResponse {
    fn from(r: Recommendation) -> Self {
        Self {
            id: r.id,
            status: r.status,
            title: r.title,
            summary: r.summary,
            analysis: r.analysis,
            proposed_changes: r.proposed_changes,
            priority: r.priority.unwrap_or_else(|| "medium".to_string()),
            category: r.category,
            confidence: r.confidence,
            source_incident_ids: r.source_incident_ids.unwrap_or_default(),
            pr_url: r.pr_url,
            pr_number: r.pr_number,
            pr_state: r.pr_state,
            pr_branch: r.pr_branch,
            base_commit_sha: r.base_commit_sha,
            has_conflicts: r.has_conflicts.unwrap_or(false),
            conflict_details: r.conflict_details,
            analyzed_at: r.analyzed_at,
            proposed_at: r.proposed_at,
            approved_at: r.approved_at,
            approved_by: r.approved_by,
            completed_at: r.completed_at,
            created_at: r.created_at.unwrap_or_else(Utc::now),
            updated_at: r.updated_at.unwrap_or_else(Utc::now),
        }
    }
}

// Discussion comment on a recommendation
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct RecommendationComment {
    pub id: Uuid,
    pub recommendation_id: Uuid,
    pub author: String,
    pub content: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct RecommendationCommentResponse {
    pub id: Uuid,
    pub recommendation_id: Uuid,
    pub author: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl From<RecommendationComment> for RecommendationCommentResponse {
    fn from(c: RecommendationComment) -> Self {
        Self {
            id: c.id,
            recommendation_id: c.recommendation_id,
            author: c.author,
            content: c.content,
            created_at: c.created_at.unwrap_or_else(Utc::now),
        }
    }
}

// Link between recommendations and incidents
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct RecommendationIncident {
    pub id: Uuid,
    pub recommendation_id: Uuid,
    pub incident_id: Uuid,
    pub relevance_score: Option<f32>,
    pub created_at: Option<DateTime<Utc>>,
}

// API contract file for breaking change detection
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ApiContractFile {
    pub id: Uuid,
    pub project_id: Uuid,
    pub file_path: String,
    pub file_type: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ApiContractFileResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub file_path: String,
    pub file_type: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<ApiContractFile> for ApiContractFileResponse {
    fn from(f: ApiContractFile) -> Self {
        Self {
            id: f.id,
            project_id: f.project_id,
            file_path: f.file_path,
            file_type: f.file_type,
            created_at: f.created_at.unwrap_or_else(Utc::now),
        }
    }
}

// Internist analysis run tracking
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct InternistAnalysisRun {
    pub id: Uuid,
    pub status: String,
    pub incident_count: Option<i32>,
    pub recommendations_created: Option<i32>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_usd: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct InternistAnalysisRunResponse {
    pub id: Uuid,
    pub status: String,
    pub incident_count: i32,
    pub recommendations_created: i32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub cost_usd: Option<Decimal>,
}

impl From<InternistAnalysisRun> for InternistAnalysisRunResponse {
    fn from(r: InternistAnalysisRun) -> Self {
        Self {
            id: r.id,
            status: r.status,
            incident_count: r.incident_count.unwrap_or(0),
            recommendations_created: r.recommendations_created.unwrap_or(0),
            started_at: r.started_at.unwrap_or_else(Utc::now),
            completed_at: r.completed_at,
            error: r.error,
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cost_usd: r.cost_usd,
        }
    }
}
