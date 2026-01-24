//! Error Pattern Matching Module
//!
//! Provides pattern-based error categorization and auto-triage capabilities.
//! Patterns are regex-based and can be user-defined or built-in.
//!
//! This module enables:
//! - Matching incoming errors against known patterns
//! - Auto-categorizing known error types without LLM calls
//! - Tracking pattern match statistics
//! - User-defined custom patterns

use anyhow::Result;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// A compiled error pattern for matching
#[derive(Debug, Clone)]
pub struct CompiledPattern {
    pub id: Uuid,
    pub name: String,
    pub regex: Regex,
    pub category: String,
    pub suggested_action: Option<String>,
    pub severity: String,
    pub auto_triage: bool,
    pub auto_fixable: bool,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

/// Database model for error patterns
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ErrorPattern {
    pub id: Uuid,
    pub name: String,
    pub regex_pattern: String,
    pub category: String,
    pub suggested_action: Option<String>,
    pub severity: Option<String>,
    pub is_builtin: Option<bool>,
    pub enabled: Option<bool>,
    pub auto_triage: Option<bool>,
    pub auto_fixable: Option<bool>,
    pub description: Option<String>,
    pub example: Option<String>,
    pub tags: Option<Vec<String>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Response type for error patterns
#[derive(Debug, Serialize)]
pub struct ErrorPatternResponse {
    pub id: Uuid,
    pub name: String,
    pub regex_pattern: String,
    pub category: String,
    pub suggested_action: Option<String>,
    pub severity: String,
    pub is_builtin: bool,
    pub enabled: bool,
    pub auto_triage: bool,
    pub auto_fixable: bool,
    pub description: Option<String>,
    pub example: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ErrorPattern> for ErrorPatternResponse {
    fn from(p: ErrorPattern) -> Self {
        Self {
            id: p.id,
            name: p.name,
            regex_pattern: p.regex_pattern,
            category: p.category,
            suggested_action: p.suggested_action,
            severity: p.severity.unwrap_or_else(|| "medium".to_string()),
            is_builtin: p.is_builtin.unwrap_or(false),
            enabled: p.enabled.unwrap_or(true),
            auto_triage: p.auto_triage.unwrap_or(false),
            auto_fixable: p.auto_fixable.unwrap_or(true),
            description: p.description,
            example: p.example,
            tags: p.tags.unwrap_or_default(),
            created_at: p.created_at.unwrap_or_else(Utc::now),
            updated_at: p.updated_at.unwrap_or_else(Utc::now),
        }
    }
}

/// Database model for pattern match statistics
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct PatternMatchStat {
    pub id: Uuid,
    pub pattern_id: Uuid,
    pub incident_id: Uuid,
    pub confidence: f64,
    pub matched_text: Option<String>,
    pub led_to_fix: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Result of pattern matching
#[derive(Debug, Clone, Serialize)]
pub struct PatternMatch {
    pub pattern_id: Uuid,
    pub pattern_name: String,
    pub category: String,
    pub suggested_action: Option<String>,
    pub severity: String,
    pub auto_triage: bool,
    pub auto_fixable: bool,
    pub matched_text: String,
    pub confidence: f64,
}

/// Request to create a new pattern
#[derive(Debug, Deserialize)]
pub struct CreatePatternRequest {
    pub name: String,
    pub regex_pattern: String,
    pub category: String,
    pub suggested_action: Option<String>,
    pub severity: Option<String>,
    pub auto_triage: Option<bool>,
    pub auto_fixable: Option<bool>,
    pub description: Option<String>,
    pub example: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Request to update a pattern
#[derive(Debug, Deserialize)]
pub struct UpdatePatternRequest {
    pub name: Option<String>,
    pub regex_pattern: Option<String>,
    pub category: Option<String>,
    pub suggested_action: Option<String>,
    pub severity: Option<String>,
    pub enabled: Option<bool>,
    pub auto_triage: Option<bool>,
    pub auto_fixable: Option<bool>,
    pub description: Option<String>,
    pub example: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Pattern matcher that caches compiled patterns
pub struct PatternMatcher {
    /// Cached compiled patterns
    patterns: Arc<RwLock<Vec<CompiledPattern>>>,
    /// Database pool for loading/saving patterns
    db: PgPool,
}

impl PatternMatcher {
    /// Create a new pattern matcher
    pub async fn new(db: PgPool) -> Result<Self> {
        let matcher = Self {
            patterns: Arc::new(RwLock::new(Vec::new())),
            db,
        };

        // Load patterns from database
        matcher.reload_patterns().await?;

        Ok(matcher)
    }

    /// Reload patterns from the database
    pub async fn reload_patterns(&self) -> Result<()> {
        let patterns = sqlx::query_as!(
            ErrorPattern,
            r#"
            SELECT id, name, regex_pattern, category, suggested_action, severity,
                   is_builtin, enabled, auto_triage, auto_fixable, description,
                   example, tags, created_at, updated_at
            FROM error_patterns
            WHERE enabled = true
            ORDER BY is_builtin DESC, name ASC
            "#
        )
        .fetch_all(&self.db)
        .await?;

        let mut compiled = Vec::with_capacity(patterns.len());
        let mut failed_count = 0;

        for pattern in patterns {
            match Regex::new(&pattern.regex_pattern) {
                Ok(regex) => {
                    compiled.push(CompiledPattern {
                        id: pattern.id,
                        name: pattern.name,
                        regex,
                        category: pattern.category,
                        suggested_action: pattern.suggested_action,
                        severity: pattern.severity.unwrap_or_else(|| "medium".to_string()),
                        auto_triage: pattern.auto_triage.unwrap_or(false),
                        auto_fixable: pattern.auto_fixable.unwrap_or(true),
                        description: pattern.description,
                        tags: pattern.tags.unwrap_or_default(),
                    });
                }
                Err(e) => {
                    warn!(
                        pattern_id = %pattern.id,
                        pattern_name = %pattern.name,
                        error = %e,
                        "Failed to compile pattern regex"
                    );
                    failed_count += 1;
                }
            }
        }

        let mut patterns_lock = self.patterns.write().await;
        *patterns_lock = compiled;

        if failed_count > 0 {
            warn!(
                failed = failed_count,
                loaded = patterns_lock.len(),
                "Some patterns failed to compile"
            );
        } else {
            info!(count = patterns_lock.len(), "Loaded error patterns");
        }

        Ok(())
    }

    /// Match an error against all known patterns
    pub async fn match_error(&self, error_text: &str) -> Vec<PatternMatch> {
        let patterns = self.patterns.read().await;
        let mut matches = Vec::new();

        for pattern in patterns.iter() {
            if let Some(m) = pattern.regex.find(error_text) {
                let matched_text = m.as_str().to_string();
                // Calculate confidence based on match length relative to error text
                let confidence = (matched_text.len() as f64 / error_text.len().max(1) as f64)
                    .min(1.0)
                    .max(0.1);

                matches.push(PatternMatch {
                    pattern_id: pattern.id,
                    pattern_name: pattern.name.clone(),
                    category: pattern.category.clone(),
                    suggested_action: pattern.suggested_action.clone(),
                    severity: pattern.severity.clone(),
                    auto_triage: pattern.auto_triage,
                    auto_fixable: pattern.auto_fixable,
                    matched_text,
                    confidence,
                });
            }
        }

        // Sort by confidence (highest first)
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

        debug!(
            matches_count = matches.len(),
            error_preview = %error_text.chars().take(100).collect::<String>(),
            "Pattern matching complete"
        );

        matches
    }

    /// Match an error from a JSON sample (extracts error message and stack trace)
    pub async fn match_sample(&self, sample: &serde_json::Value) -> Vec<PatternMatch> {
        let error_text = extract_error_text(sample);
        self.match_error(&error_text).await
    }

    /// Record a pattern match in the database
    pub async fn record_match(
        &self,
        pattern_match: &PatternMatch,
        incident_id: Uuid,
    ) -> Result<Uuid> {
        let match_id = Uuid::new_v4();

        sqlx::query!(
            r#"
            INSERT INTO pattern_match_stats (id, pattern_id, incident_id, confidence, matched_text)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            match_id,
            pattern_match.pattern_id,
            incident_id,
            pattern_match.confidence,
            pattern_match.matched_text,
        )
        .execute(&self.db)
        .await?;

        Ok(match_id)
    }

    /// Update incident with matched pattern info
    pub async fn update_incident_pattern(
        &self,
        incident_id: Uuid,
        pattern_match: &PatternMatch,
        auto_triaged: bool,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE incidents
            SET matched_pattern_id = $1,
                pattern_auto_triaged = $2,
                updated_at = NOW()
            WHERE id = $3
            "#,
            pattern_match.pattern_id,
            auto_triaged,
            incident_id,
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Get pattern match statistics for a pattern
    pub async fn get_pattern_stats(&self, pattern_id: Uuid) -> Result<PatternStats> {
        let total_matches: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM pattern_match_stats WHERE pattern_id = $1",
            pattern_id
        )
        .fetch_one(&self.db)
        .await?
        .unwrap_or(0);

        let led_to_fix: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM pattern_match_stats WHERE pattern_id = $1 AND led_to_fix = true",
            pattern_id
        )
        .fetch_one(&self.db)
        .await?
        .unwrap_or(0);

        let avg_confidence: f64 = sqlx::query_scalar!(
            "SELECT AVG(confidence) FROM pattern_match_stats WHERE pattern_id = $1",
            pattern_id
        )
        .fetch_one(&self.db)
        .await?
        .unwrap_or(0.0);

        let last_match: Option<DateTime<Utc>> = sqlx::query_scalar!(
            "SELECT MAX(created_at) FROM pattern_match_stats WHERE pattern_id = $1",
            pattern_id
        )
        .fetch_one(&self.db)
        .await?;

        Ok(PatternStats {
            pattern_id,
            total_matches,
            successful_fixes: led_to_fix,
            average_confidence: avg_confidence,
            last_match,
        })
    }

    /// Get matches for a specific incident
    pub async fn get_incident_matches(&self, incident_id: Uuid) -> Result<Vec<IncidentPatternMatch>> {
        let matches = sqlx::query!(
            r#"
            SELECT
                pms.id,
                pms.pattern_id,
                pms.confidence,
                pms.matched_text,
                pms.led_to_fix,
                pms.created_at,
                ep.name as pattern_name,
                ep.category,
                ep.suggested_action,
                ep.severity
            FROM pattern_match_stats pms
            JOIN error_patterns ep ON pms.pattern_id = ep.id
            WHERE pms.incident_id = $1
            ORDER BY pms.confidence DESC
            "#,
            incident_id
        )
        .fetch_all(&self.db)
        .await?;

        Ok(matches
            .into_iter()
            .map(|m| IncidentPatternMatch {
                id: m.id,
                pattern_id: m.pattern_id,
                pattern_name: m.pattern_name,
                category: m.category,
                suggested_action: m.suggested_action,
                severity: m.severity.unwrap_or_else(|| "medium".to_string()),
                confidence: m.confidence,
                matched_text: m.matched_text,
                led_to_fix: m.led_to_fix,
                created_at: m.created_at.unwrap_or_else(Utc::now),
            })
            .collect())
    }

    /// Create a new pattern
    pub async fn create_pattern(&self, req: CreatePatternRequest) -> Result<ErrorPattern> {
        // Validate regex
        Regex::new(&req.regex_pattern).map_err(|e| anyhow::anyhow!("Invalid regex: {}", e))?;

        let id = Uuid::new_v4();
        let pattern = sqlx::query_as!(
            ErrorPattern,
            r#"
            INSERT INTO error_patterns (
                id, name, regex_pattern, category, suggested_action, severity,
                is_builtin, enabled, auto_triage, auto_fixable, description, example, tags
            )
            VALUES ($1, $2, $3, $4, $5, $6, false, true, $7, $8, $9, $10, $11)
            RETURNING id, name, regex_pattern, category, suggested_action, severity,
                      is_builtin, enabled, auto_triage, auto_fixable, description,
                      example, tags, created_at, updated_at
            "#,
            id,
            req.name,
            req.regex_pattern,
            req.category,
            req.suggested_action,
            req.severity.unwrap_or_else(|| "medium".to_string()),
            req.auto_triage.unwrap_or(false),
            req.auto_fixable.unwrap_or(true),
            req.description,
            req.example,
            req.tags.as_deref(),
        )
        .fetch_one(&self.db)
        .await?;

        // Reload patterns to include the new one
        self.reload_patterns().await?;

        Ok(pattern)
    }

    /// Update an existing pattern
    pub async fn update_pattern(
        &self,
        pattern_id: Uuid,
        req: UpdatePatternRequest,
    ) -> Result<ErrorPattern> {
        // If regex is being updated, validate it
        if let Some(ref regex_pattern) = req.regex_pattern {
            Regex::new(regex_pattern).map_err(|e| anyhow::anyhow!("Invalid regex: {}", e))?;
        }

        let pattern = sqlx::query_as!(
            ErrorPattern,
            r#"
            UPDATE error_patterns
            SET name = COALESCE($2, name),
                regex_pattern = COALESCE($3, regex_pattern),
                category = COALESCE($4, category),
                suggested_action = COALESCE($5, suggested_action),
                severity = COALESCE($6, severity),
                enabled = COALESCE($7, enabled),
                auto_triage = COALESCE($8, auto_triage),
                auto_fixable = COALESCE($9, auto_fixable),
                description = COALESCE($10, description),
                example = COALESCE($11, example),
                tags = COALESCE($12, tags),
                updated_at = NOW()
            WHERE id = $1 AND is_builtin = false
            RETURNING id, name, regex_pattern, category, suggested_action, severity,
                      is_builtin, enabled, auto_triage, auto_fixable, description,
                      example, tags, created_at, updated_at
            "#,
            pattern_id,
            req.name,
            req.regex_pattern,
            req.category,
            req.suggested_action,
            req.severity,
            req.enabled,
            req.auto_triage,
            req.auto_fixable,
            req.description,
            req.example,
            req.tags.as_deref(),
        )
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Pattern not found or is a built-in pattern"))?;

        // Reload patterns
        self.reload_patterns().await?;

        Ok(pattern)
    }

    /// Delete a pattern (only user-defined)
    pub async fn delete_pattern(&self, pattern_id: Uuid) -> Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM error_patterns WHERE id = $1 AND is_builtin = false",
            pattern_id
        )
        .execute(&self.db)
        .await?;

        if result.rows_affected() > 0 {
            self.reload_patterns().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List all patterns
    pub async fn list_patterns(
        &self,
        category: Option<&str>,
        builtin_only: Option<bool>,
    ) -> Result<Vec<ErrorPattern>> {
        let patterns = match (category, builtin_only) {
            (Some(cat), Some(builtin)) => {
                sqlx::query_as!(
                    ErrorPattern,
                    r#"
                    SELECT id, name, regex_pattern, category, suggested_action, severity,
                           is_builtin, enabled, auto_triage, auto_fixable, description,
                           example, tags, created_at, updated_at
                    FROM error_patterns
                    WHERE category = $1 AND is_builtin = $2
                    ORDER BY is_builtin DESC, name ASC
                    "#,
                    cat,
                    builtin
                )
                .fetch_all(&self.db)
                .await?
            }
            (Some(cat), None) => {
                sqlx::query_as!(
                    ErrorPattern,
                    r#"
                    SELECT id, name, regex_pattern, category, suggested_action, severity,
                           is_builtin, enabled, auto_triage, auto_fixable, description,
                           example, tags, created_at, updated_at
                    FROM error_patterns
                    WHERE category = $1
                    ORDER BY is_builtin DESC, name ASC
                    "#,
                    cat
                )
                .fetch_all(&self.db)
                .await?
            }
            (None, Some(builtin)) => {
                sqlx::query_as!(
                    ErrorPattern,
                    r#"
                    SELECT id, name, regex_pattern, category, suggested_action, severity,
                           is_builtin, enabled, auto_triage, auto_fixable, description,
                           example, tags, created_at, updated_at
                    FROM error_patterns
                    WHERE is_builtin = $1
                    ORDER BY is_builtin DESC, name ASC
                    "#,
                    builtin
                )
                .fetch_all(&self.db)
                .await?
            }
            (None, None) => {
                sqlx::query_as!(
                    ErrorPattern,
                    r#"
                    SELECT id, name, regex_pattern, category, suggested_action, severity,
                           is_builtin, enabled, auto_triage, auto_fixable, description,
                           example, tags, created_at, updated_at
                    FROM error_patterns
                    ORDER BY is_builtin DESC, name ASC
                    "#
                )
                .fetch_all(&self.db)
                .await?
            }
        };

        Ok(patterns)
    }

    /// Get a single pattern by ID
    pub async fn get_pattern(&self, pattern_id: Uuid) -> Result<Option<ErrorPattern>> {
        let pattern = sqlx::query_as!(
            ErrorPattern,
            r#"
            SELECT id, name, regex_pattern, category, suggested_action, severity,
                   is_builtin, enabled, auto_triage, auto_fixable, description,
                   example, tags, created_at, updated_at
            FROM error_patterns
            WHERE id = $1
            "#,
            pattern_id
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(pattern)
    }

    /// Get list of all categories
    pub async fn list_categories(&self) -> Result<Vec<CategoryInfo>> {
        let categories = sqlx::query!(
            r#"
            SELECT
                category,
                COUNT(*) as "count!",
                COUNT(*) FILTER (WHERE enabled = true) as "enabled_count!"
            FROM error_patterns
            GROUP BY category
            ORDER BY category
            "#
        )
        .fetch_all(&self.db)
        .await?;

        Ok(categories
            .into_iter()
            .map(|c| CategoryInfo {
                category: c.category,
                pattern_count: c.count,
                enabled_count: c.enabled_count,
            })
            .collect())
    }
}

/// Pattern statistics
#[derive(Debug, Serialize)]
pub struct PatternStats {
    pub pattern_id: Uuid,
    pub total_matches: i64,
    pub successful_fixes: i64,
    pub average_confidence: f64,
    pub last_match: Option<DateTime<Utc>>,
}

/// Pattern match info for an incident
#[derive(Debug, Serialize)]
pub struct IncidentPatternMatch {
    pub id: Uuid,
    pub pattern_id: Uuid,
    pub pattern_name: String,
    pub category: String,
    pub suggested_action: Option<String>,
    pub severity: String,
    pub confidence: f64,
    pub matched_text: Option<String>,
    pub led_to_fix: Option<bool>,
    pub created_at: DateTime<Utc>,
}

/// Category information
#[derive(Debug, Serialize)]
pub struct CategoryInfo {
    pub category: String,
    pub pattern_count: i64,
    pub enabled_count: i64,
}

/// Extract error text from a sample JSON for pattern matching
fn extract_error_text(sample: &serde_json::Value) -> String {
    let mut parts = Vec::new();

    // Try to extract common error fields
    if let Some(message) = sample.get("message").and_then(|v| v.as_str()) {
        parts.push(message.to_string());
    }
    if let Some(error) = sample.get("error").and_then(|v| v.as_str()) {
        parts.push(error.to_string());
    }
    if let Some(exception) = sample.get("exception").and_then(|v| v.as_str()) {
        parts.push(exception.to_string());
    }
    if let Some(body) = sample.get("body").and_then(|v| v.as_str()) {
        parts.push(body.to_string());
    }
    if let Some(stack_trace) = sample.get("stack_trace").and_then(|v| v.as_str()) {
        parts.push(stack_trace.to_string());
    }
    if let Some(stacktrace) = sample.get("stacktrace").and_then(|v| v.as_str()) {
        parts.push(stacktrace.to_string());
    }
    if let Some(trace) = sample.get("trace").and_then(|v| v.as_str()) {
        parts.push(trace.to_string());
    }

    // Also check nested structures
    if let Some(attributes) = sample.get("attributes").and_then(|v| v.as_object()) {
        for (key, value) in attributes {
            if let Some(s) = value.as_str() {
                if key.contains("error") || key.contains("exception") || key.contains("message") {
                    parts.push(s.to_string());
                }
            }
        }
    }

    // If nothing found, stringify the whole sample
    if parts.is_empty() {
        sample.to_string()
    } else {
        parts.join("\n")
    }
}

/// Auto-triage result from pattern matching
#[derive(Debug, Clone, Serialize)]
pub struct AutoTriageResult {
    pub should_skip_llm: bool,
    pub fixable: bool,
    pub category: String,
    pub confidence: f32,
    pub reason: String,
    pub suggested_files: Vec<String>,
    pub description: String,
    pub fix_approach: Option<String>,
    pub matched_pattern: Option<PatternMatch>,
}

impl PatternMatcher {
    /// Attempt to auto-triage an error using pattern matching
    /// Returns Some(AutoTriageResult) if a pattern with auto_triage enabled matches
    pub async fn auto_triage(&self, sample: &serde_json::Value) -> Option<AutoTriageResult> {
        let matches = self.match_sample(sample).await;

        // Find the first match with auto_triage enabled
        for m in matches {
            if m.auto_triage && m.confidence >= 0.2 {
                return Some(AutoTriageResult {
                    should_skip_llm: true,
                    fixable: m.auto_fixable,
                    category: m.category.clone(),
                    confidence: m.confidence as f32,
                    reason: format!(
                        "Matched pattern '{}': {}",
                        m.pattern_name,
                        m.suggested_action.as_deref().unwrap_or("No suggested action")
                    ),
                    suggested_files: vec![], // Pattern matching doesn't suggest files
                    description: format!(
                        "Error matched known pattern '{}' ({})",
                        m.pattern_name, m.category
                    ),
                    fix_approach: m.suggested_action.clone(),
                    matched_pattern: Some(m),
                });
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_error_text() {
        let sample = serde_json::json!({
            "message": "NullPointerException",
            "stack_trace": "at com.example.Main.main(Main.java:10)"
        });

        let text = extract_error_text(&sample);
        assert!(text.contains("NullPointerException"));
        assert!(text.contains("Main.java"));
    }

    #[test]
    fn test_extract_error_text_nested() {
        let sample = serde_json::json!({
            "attributes": {
                "error.message": "Connection refused",
                "other": "value"
            }
        });

        let text = extract_error_text(&sample);
        assert!(text.contains("Connection refused"));
    }

    #[tokio::test]
    async fn test_pattern_matching() {
        // Create a simple in-memory pattern for testing
        let regex = Regex::new(r"NullPointerException").unwrap();
        let pattern = CompiledPattern {
            id: Uuid::new_v4(),
            name: "NPE".to_string(),
            regex,
            category: "null_reference".to_string(),
            suggested_action: Some("Add null check".to_string()),
            severity: "high".to_string(),
            auto_triage: true,
            auto_fixable: true,
            description: None,
            tags: vec![],
        };

        // Test matching
        let error_text = "java.lang.NullPointerException: Cannot invoke method";
        let m = pattern.regex.find(error_text);
        assert!(m.is_some());
        assert_eq!(m.unwrap().as_str(), "NullPointerException");
    }
}
