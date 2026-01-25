//! Internist Pipeline - Pattern Analysis Module
//!
//! Uses the internist LLM tier to perform high-level pattern analysis across incidents
//! to identify architectural issues, propose systemic fixes, and detect recurring patterns.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use crate::integrations::llm::TrackedLlm;

/// Summary of an incident for pattern analysis
#[derive(Debug, Clone, Serialize)]
pub struct IncidentSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,  // e.g., "acme/backend-api" or "acme/user-service"
    pub title: Option<String>,
    pub fingerprint: String,
    pub category: Option<String>,
    pub occurrence_count: i32,
    pub state: String,
    pub error_message: Option<String>,
    pub stack_trace: Option<String>,
    pub affected_files: Vec<String>,
    pub fix_applied: Option<String>,
    pub fix_successful: bool,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

/// A pattern found across multiple incidents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    pub pattern_type: String,
    pub description: String,
    pub incident_ids: Vec<Uuid>,
    pub confidence: f32,
    pub root_cause: Option<String>,
}

/// A recommendation generated from pattern analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationData {
    pub title: String,
    pub summary: String,
    pub category: String,
    pub priority: String,
    pub confidence: f32,
    pub analysis: AnalysisDetails,
    pub proposed_changes: Vec<ProposedChange>,
    pub related_incident_ids: Vec<Uuid>,
    /// If true, this recommendation requires human intervention (e.g., infrastructure changes)
    /// and cannot be automatically fixed by code changes
    #[serde(default)]
    pub requires_human_intervention: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisDetails {
    pub root_cause: String,
    pub impact_assessment: String,
    pub affected_components: Vec<String>,
    pub patterns_identified: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedChange {
    pub file_path: Option<String>,
    pub change_type: String,
    pub description: String,
    pub rationale: String,
}

/// Result of internist analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternistAnalysis {
    pub recommendations: Vec<RecommendationData>,
    pub patterns_found: Vec<PatternMatch>,
    pub summary: String,
}

const INTERNIST_SYSTEM_PROMPT: &str = r#"You are a senior software architect performing a "medical rounds" analysis of a codebase's health.

Your role is to analyze patterns across multiple production incidents to:
1. Identify architectural issues causing recurring bugs
2. Detect API contract violations or breaking changes
3. Propose systemic fixes rather than ad-hoc patches
4. Assess the overall health of affected components
5. Identify cross-service dependencies and infrastructure issues

Think like an internist doctor:
- Look for underlying conditions, not just symptoms
- Consider how issues in one area might affect others
- Look for correlations between different services/projects failing together
- Recommend preventive measures, not just treatments
- Prioritize based on severity and impact

IMPORTANT: You are NOT obligated to make a recommendation. If you don't see a clear path to improvement that wouldn't introduce more risk, say so. Only recommend changes when you have high confidence they will help.

Categories of issues to watch for:
- api_contract: Breaking changes to APIs, inconsistent interfaces
- architecture: Structural problems, tight coupling, missing abstractions
- error_pattern: Recurring error types that need systematic handling
- performance: Systemic performance issues
- security: Security patterns or vulnerabilities
- infrastructure: Database sizing, resource limits, scaling issues (REQUIRES HUMAN)

For infrastructure issues that cannot be fixed by code changes (database scaling, server resources, etc.), mark them with "requires_human_intervention": true. These will create tickets for humans to address.

Respond in JSON format only."#;

/// Run pattern analysis across recent incidents
pub async fn analyze_patterns(
    llm: &TrackedLlm,
    incidents: &[IncidentSummary],
    project_context: &str,
) -> Result<InternistAnalysis> {
    if incidents.is_empty() {
        return Ok(InternistAnalysis {
            recommendations: vec![],
            patterns_found: vec![],
            summary: "No incidents to analyze".to_string(),
        });
    }

    let incidents_json = serde_json::to_string_pretty(incidents)?;

    let context_section = if project_context.is_empty() {
        String::new()
    } else {
        format!(
            r#"
Project Context:
{}

"#,
            project_context
        )
    };

    let prompt = format!(
        r#"Perform a comprehensive pattern analysis of these recent production incidents.
{context_section}
Incidents ({} total, potentially across multiple services/projects):
```json
{}
```

Analyze these incidents to:
1. Identify recurring patterns and their root causes
2. Detect architectural issues that lead to multiple incidents
3. Look for CROSS-SERVICE patterns - are multiple services failing due to shared infrastructure?
4. Find opportunities for systemic improvements
5. Prioritize recommendations by impact and urgency

IMPORTANT: You are NOT obligated to make a recommendation. If the incidents appear to be:
- Unrelated one-off issues that were already fixed appropriately
- Not indicative of any systemic problem
- Would require changes that introduce more risk than they solve
Then return an empty recommendations array and explain in the summary why no recommendation is warranted.

Respond with a JSON object containing:
- "recommendations": array of recommendation objects (CAN BE EMPTY if no clear improvements), each with:
  - "title": string - concise title for the recommendation
  - "summary": string - brief description (1-2 sentences)
  - "category": string - one of: api_contract, architecture, error_pattern, performance, security, infrastructure
  - "priority": string - one of: low, medium, high, critical
  - "confidence": number 0-1 - how confident in this recommendation
  - "requires_human_intervention": boolean - true if this CANNOT be fixed by code (e.g., increase DB size, add servers, update configs)
  - "analysis": object with:
    - "root_cause": string - the underlying cause
    - "impact_assessment": string - what happens if not addressed
    - "affected_components": array of strings - components/files/services affected
    - "patterns_identified": array of strings - patterns that led to this recommendation
  - "proposed_changes": array of objects, each with:
    - "file_path": string or null - specific file if applicable (null for infrastructure changes)
    - "change_type": string - refactor, add, modify, remove, infrastructure
    - "description": string - what to change (for humans, describe what action they need to take)
    - "rationale": string - why this change helps
  - "related_incident_ids": array of UUIDs - incidents that informed this
- "patterns_found": array of pattern objects, each with:
  - "pattern_type": string - type of pattern (e.g., "null_check_missing", "error_handling_gap", "resource_exhaustion")
  - "description": string - description of the pattern
  - "incident_ids": array of UUIDs - incidents exhibiting this pattern
  - "confidence": number 0-1
  - "root_cause": string or null
- "summary": string - overall assessment. If no recommendations, explain why (e.g., "These incidents were unrelated and appropriately handled. No systemic issues detected.")

JSON response:"#,
        incidents.len(),
        incidents_json
    );

    let response = llm
        .complete(&prompt, Some(INTERNIST_SYSTEM_PROMPT), "internist_analysis", None, None)
        .await?;

    let result = parse_json_response(&response.content)?;

    info!(
        recommendations = result.recommendations.len(),
        patterns = result.patterns_found.len(),
        "Internist analysis completed"
    );

    Ok(result)
}

/// Helper to convert incident row to summary
fn row_to_summary(
    id: Uuid,
    project_id: Uuid,
    project_repo: String,
    title: Option<String>,
    fingerprint: String,
    state: Option<String>,
    occurrence_count: Option<i32>,
    pr_state: Option<String>,
    verification_status: Option<String>,
    last_seen: Option<chrono::DateTime<chrono::Utc>>,
    activity_details: Option<serde_json::Value>,
) -> IncidentSummary {
    let details: serde_json::Value = activity_details.unwrap_or(serde_json::Value::Null);
    let category = details.get("category").and_then(|v: &serde_json::Value| v.as_str()).map(String::from);
    let fix_applied = details.get("fix_approach").and_then(|v: &serde_json::Value| v.as_str()).map(String::from);
    let affected_files: Vec<String> = details
        .get("suggested_files")
        .and_then(|v: &serde_json::Value| v.as_array())
        .map(|arr: &Vec<serde_json::Value>| {
            arr.iter()
                .filter_map(|v: &serde_json::Value| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let fix_successful = verification_status.as_deref() == Some("verified")
        || pr_state.as_deref() == Some("merged");

    IncidentSummary {
        id,
        project_id,
        project_name: project_repo,
        title,
        fingerprint,
        category,
        occurrence_count: occurrence_count.unwrap_or(1),
        state: state.unwrap_or_else(|| "detected".to_string()),
        error_message: None,
        stack_trace: None,
        affected_files,
        fix_applied,
        fix_successful,
        last_seen,
    }
}

/// Gather incident summaries from the database (across ALL projects for cross-service analysis)
pub async fn gather_incident_summaries(
    db: &PgPool,
    min_incidents: i32,
    max_incidents: i32,
) -> Result<Vec<IncidentSummary>> {
    // Get recent incidents with their details, including project info for cross-service analysis
    let mut summaries: Vec<IncidentSummary> = Vec::new();

    let mut stream = sqlx::query!(
        r#"
        SELECT
            i.id,
            i.project_id,
            p.github_owner || '/' || p.github_repo as "project_repo!",
            i.title,
            i.fingerprint,
            i.state,
            i.occurrence_count,
            i.pr_state,
            i.verification_status,
            i.last_seen,
            al.details as activity_details
        FROM incidents i
        JOIN projects p ON i.project_id = p.id
        LEFT JOIN LATERAL (
            SELECT details
            FROM activity_log
            WHERE incident_id = i.id
            AND action IN ('triage_completed', 'fix_generated')
            ORDER BY created_at DESC
            LIMIT 1
        ) al ON true
        WHERE i.created_at > NOW() - INTERVAL '30 days'
        ORDER BY i.occurrence_count DESC, i.last_seen DESC
        LIMIT $1
        "#,
        max_incidents as i64
    )
    .fetch(db);

    use futures::StreamExt;
    while let Some(row) = stream.next().await {
        let i = row?;
        summaries.push(row_to_summary(
            i.id,
            i.project_id,
            i.project_repo,
            i.title,
            i.fingerprint,
            i.state,
            i.occurrence_count,
            i.pr_state,
            i.verification_status,
            i.last_seen,
            i.activity_details,
        ));
    }

    if summaries.len() < min_incidents as usize {
        return Ok(vec![]); // Not enough incidents for meaningful analysis
    }

    Ok(summaries)
}

/// Gather a focused set of incidents for post-verification analysis
/// Called after an incident is verified to see if there are related patterns
pub async fn gather_recent_verified_incidents(
    db: &PgPool,
    just_verified_id: Uuid,
    lookback_hours: i32,
) -> Result<Vec<IncidentSummary>> {
    // Get the just-verified incident plus recent incidents for pattern analysis
    let mut summaries: Vec<IncidentSummary> = Vec::new();

    let mut stream = sqlx::query!(
        r#"
        SELECT
            i.id,
            i.project_id,
            p.github_owner || '/' || p.github_repo as "project_repo!",
            i.title,
            i.fingerprint,
            i.state,
            i.occurrence_count,
            i.pr_state,
            i.verification_status,
            i.last_seen,
            al.details as activity_details
        FROM incidents i
        JOIN projects p ON i.project_id = p.id
        LEFT JOIN LATERAL (
            SELECT details
            FROM activity_log
            WHERE incident_id = i.id
            AND action IN ('triage_completed', 'fix_generated')
            ORDER BY created_at DESC
            LIMIT 1
        ) al ON true
        WHERE (
            i.id = $1
            OR (
                i.verified_at > NOW() - ($2 || ' hours')::INTERVAL
                AND i.verification_status = 'verified'
            )
            OR (
                i.last_seen > NOW() - ($2 || ' hours')::INTERVAL
                AND i.state IN ('detected', 'triaging', 'pending_fix')
            )
        )
        ORDER BY i.last_seen DESC
        LIMIT 50
        "#,
        just_verified_id,
        lookback_hours.to_string()
    )
    .fetch(db);

    use futures::StreamExt;
    while let Some(row) = stream.next().await {
        let i = row?;
        summaries.push(row_to_summary(
            i.id,
            i.project_id,
            i.project_repo,
            i.title,
            i.fingerprint,
            i.state,
            i.occurrence_count,
            i.pr_state,
            i.verification_status,
            i.last_seen,
            i.activity_details,
        ));
    }

    Ok(summaries)
}

/// Create recommendations in the database from analysis results
pub async fn create_recommendations(
    db: &PgPool,
    analysis: &InternistAnalysis,
    run_id: Uuid,
) -> Result<Vec<Uuid>> {
    let mut created_ids = Vec::new();

    for rec in &analysis.recommendations {
        let id = Uuid::new_v4();

        let analysis_json = serde_json::to_value(&rec.analysis)?;
        let proposed_changes_json = serde_json::to_value(&rec.proposed_changes)?;
        let incident_ids: Vec<Uuid> = rec.related_incident_ids.clone();

        sqlx::query!(
            r#"
            INSERT INTO recommendations (
                id, status, title, summary, analysis, proposed_changes,
                priority, category, confidence, source_incident_ids,
                requires_human_intervention,
                analyzed_at, proposed_at
            ) VALUES (
                $1, 'proposed', $2, $3, $4, $5,
                $6, $7, $8, $9, $10,
                NOW(), NOW()
            )
            "#,
            id,
            rec.title,
            rec.summary,
            analysis_json,
            proposed_changes_json,
            rec.priority,
            rec.category,
            rec.confidence,
            &incident_ids,
            rec.requires_human_intervention,
        )
        .execute(db)
        .await?;

        // Link to incidents
        for incident_id in &rec.related_incident_ids {
            let _ = sqlx::query!(
                r#"
                INSERT INTO recommendation_incidents (recommendation_id, incident_id, relevance_score)
                VALUES ($1, $2, $3)
                ON CONFLICT DO NOTHING
                "#,
                id,
                incident_id,
                rec.confidence,
            )
            .execute(db)
            .await;
        }

        created_ids.push(id);
    }

    // Update run with count
    sqlx::query!(
        "UPDATE internist_analysis_runs SET recommendations_created = $1 WHERE id = $2",
        created_ids.len() as i32,
        run_id
    )
    .execute(db)
    .await?;

    Ok(created_ids)
}

/// Parse JSON from LLM response, handling markdown code blocks
fn parse_json_response(content: &str) -> Result<InternistAnalysis> {
    let json_str = if content.contains("```json") {
        content
            .split("```json")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap_or(content)
            .trim()
    } else if content.contains("```") {
        content
            .split("```")
            .nth(1)
            .unwrap_or(content)
            .trim()
    } else {
        content.trim()
    };

    match serde_json::from_str(json_str) {
        Ok(result) => Ok(result),
        Err(e) => {
            if let Some(start) = json_str.find('{') {
                if let Some(end) = json_str.rfind('}') {
                    let json_obj = &json_str[start..=end];
                    return Ok(serde_json::from_str(json_obj)?);
                }
            }
            warn!(error = %e, "Failed to parse internist response, returning empty analysis");
            Ok(InternistAnalysis {
                recommendations: vec![],
                patterns_found: vec![],
                summary: format!("Failed to parse analysis: {}", e),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_response() {
        let response = r#"```json
{
    "recommendations": [],
    "patterns_found": [],
    "summary": "No significant patterns found"
}
```"#;

        let result = parse_json_response(response).unwrap();
        assert_eq!(result.summary, "No significant patterns found");
    }
}
