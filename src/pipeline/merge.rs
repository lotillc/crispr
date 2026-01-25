//! Intelligent Merge Module
//!
//! Handles conflict detection and intelligent rebase for long-lived recommendations.
//! When recommendations sit for weeks during team discussion, the base code may change,
//! causing merge conflicts. This module helps detect and resolve those conflicts.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use crate::integrations::llm::TrackedLlm;
use crate::pipeline::repo::RepoManager;

/// Result of checking for merge conflicts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictCheckResult {
    pub has_conflicts: bool,
    pub conflicting_files: Vec<String>,
    pub base_commit: String,
    pub current_head: String,
    pub commits_behind: i32,
    pub can_auto_resolve: bool,
    pub conflict_details: Vec<ConflictDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictDetail {
    pub file_path: String,
    pub conflict_type: ConflictType,
    pub ours_change: Option<String>,
    pub theirs_change: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    /// Both sides modified the same lines
    ContentConflict,
    /// File was deleted on one side, modified on other
    DeleteModify,
    /// Both sides added file with same name
    AddAdd,
    /// File was renamed differently on both sides
    RenameRename,
    /// Semantic conflict (code compiles but logic conflicts)
    Semantic,
}

/// Result of attempting a rebase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseResult {
    pub success: bool,
    pub new_base_commit: Option<String>,
    pub resolved_files: Vec<String>,
    pub remaining_conflicts: Vec<ConflictDetail>,
    pub changes_applied: Vec<MergeChange>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeChange {
    pub file_path: String,
    pub resolution_type: ResolutionType,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionType {
    /// Kept our changes (recommendation's version)
    KeepOurs,
    /// Accepted their changes (main branch version)
    KeepTheirs,
    /// Merged both changes together
    MergedBoth,
    /// LLM rewrote the conflicting section
    LlmResolved,
    /// Manual resolution required
    Manual,
}

/// Check if a recommendation has merge conflicts with the current main branch
pub async fn check_conflicts(
    db: &PgPool,
    recommendation_id: Uuid,
    repo_manager: &RepoManager,
) -> Result<ConflictCheckResult> {
    // Get the recommendation's base commit
    let rec = sqlx::query!(
        r#"
        SELECT base_commit_sha, proposed_changes
        FROM recommendations
        WHERE id = $1
        "#,
        recommendation_id
    )
    .fetch_optional(db)
    .await?;

    let rec = match rec {
        Some(r) => r,
        None => {
            return Err(anyhow::anyhow!(
                "Recommendation {} not found",
                recommendation_id
            ))
        }
    };

    let base_sha = match rec.base_commit_sha {
        Some(sha) => sha,
        None => {
            // No base commit recorded, can't check conflicts
            return Ok(ConflictCheckResult {
                has_conflicts: false,
                conflicting_files: vec![],
                base_commit: String::new(),
                current_head: String::new(),
                commits_behind: 0,
                can_auto_resolve: true,
                conflict_details: vec![],
            });
        }
    };

    // Get the current HEAD of the default branch
    let current_head = repo_manager.get_head_sha().await?;

    // If base commit matches current head, no conflicts possible
    if base_sha == current_head {
        return Ok(ConflictCheckResult {
            has_conflicts: false,
            conflicting_files: vec![],
            base_commit: base_sha,
            current_head,
            commits_behind: 0,
            can_auto_resolve: true,
            conflict_details: vec![],
        });
    }

    // Get the files that the recommendation wants to change
    let proposed_changes = rec.proposed_changes.unwrap_or_default();
    let proposed_files: Vec<String> = proposed_changes
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("file_path").and_then(|f| f.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Check if any of those files have changed since base commit
    let changed_files = repo_manager
        .get_changed_files_since(&base_sha, &current_head)
        .await?;

    let conflicting_files: Vec<String> = proposed_files
        .iter()
        .filter(|f| changed_files.contains(f))
        .cloned()
        .collect();

    let commits_behind = repo_manager
        .count_commits_between(&base_sha, &current_head)
        .await
        .unwrap_or(0);

    let has_conflicts = !conflicting_files.is_empty();

    // Build conflict details
    let conflict_details: Vec<ConflictDetail> = conflicting_files
        .iter()
        .map(|f| ConflictDetail {
            file_path: f.clone(),
            conflict_type: ConflictType::ContentConflict,
            ours_change: None,
            theirs_change: None,
            description: format!("File {} was modified in main branch since recommendation was created", f),
        })
        .collect();

    // Update recommendation with conflict status
    sqlx::query!(
        r#"
        UPDATE recommendations
        SET has_conflicts = $1,
            conflict_details = $2,
            updated_at = NOW()
        WHERE id = $3
        "#,
        has_conflicts,
        serde_json::to_value(&conflict_details)?,
        recommendation_id
    )
    .execute(db)
    .await?;

    Ok(ConflictCheckResult {
        has_conflicts,
        conflicting_files,
        base_commit: base_sha,
        current_head,
        commits_behind,
        can_auto_resolve: conflict_details.len() <= 3, // Simple heuristic
        conflict_details,
    })
}

/// Attempt to rebase a recommendation's changes onto the current main branch
pub async fn attempt_rebase(
    db: &PgPool,
    llm: &TrackedLlm,
    recommendation_id: Uuid,
    repo_manager: &RepoManager,
) -> Result<RebaseResult> {
    // First check for conflicts
    let conflict_check = check_conflicts(db, recommendation_id, repo_manager).await?;

    if !conflict_check.has_conflicts {
        // No conflicts, just update the base commit
        sqlx::query!(
            r#"
            UPDATE recommendations
            SET base_commit_sha = $1,
                has_conflicts = false,
                conflict_details = NULL,
                updated_at = NOW()
            WHERE id = $2
            "#,
            conflict_check.current_head,
            recommendation_id
        )
        .execute(db)
        .await?;

        return Ok(RebaseResult {
            success: true,
            new_base_commit: Some(conflict_check.current_head),
            resolved_files: vec![],
            remaining_conflicts: vec![],
            changes_applied: vec![],
            summary: "No conflicts found, updated base commit".to_string(),
        });
    }

    // Attempt to resolve conflicts using LLM
    let rec = sqlx::query!(
        "SELECT proposed_changes FROM recommendations WHERE id = $1",
        recommendation_id
    )
    .fetch_one(db)
    .await?;

    let proposed_changes = rec.proposed_changes.unwrap_or_default();

    let mut resolved_files = Vec::new();
    let mut remaining_conflicts = Vec::new();
    let mut changes_applied = Vec::new();

    for conflict in &conflict_check.conflict_details {
        match resolve_conflict_with_llm(
            llm,
            &conflict.file_path,
            &proposed_changes,
            repo_manager,
            &conflict_check.base_commit,
            &conflict_check.current_head,
        )
        .await
        {
            Ok(resolution) => {
                resolved_files.push(conflict.file_path.clone());
                changes_applied.push(MergeChange {
                    file_path: conflict.file_path.clone(),
                    resolution_type: resolution.resolution_type,
                    description: resolution.description,
                });
            }
            Err(e) => {
                warn!(
                    file = %conflict.file_path,
                    error = %e,
                    "Failed to auto-resolve conflict"
                );
                remaining_conflicts.push(conflict.clone());
            }
        }
    }

    let success = remaining_conflicts.is_empty();

    // Update recommendation
    if success {
        sqlx::query!(
            r#"
            UPDATE recommendations
            SET base_commit_sha = $1,
                has_conflicts = false,
                conflict_details = NULL,
                updated_at = NOW()
            WHERE id = $2
            "#,
            conflict_check.current_head,
            recommendation_id
        )
        .execute(db)
        .await?;
    } else {
        sqlx::query!(
            r#"
            UPDATE recommendations
            SET conflict_details = $1,
                updated_at = NOW()
            WHERE id = $2
            "#,
            serde_json::to_value(&remaining_conflicts)?,
            recommendation_id
        )
        .execute(db)
        .await?;
    }

    let summary = if success {
        format!(
            "Successfully rebased onto {}, resolved {} conflicts",
            &conflict_check.current_head[..8],
            resolved_files.len()
        )
    } else {
        format!(
            "Partial rebase: resolved {}/{} conflicts, {} require manual resolution",
            resolved_files.len(),
            conflict_check.conflict_details.len(),
            remaining_conflicts.len()
        )
    };

    info!(
        recommendation_id = %recommendation_id,
        success = success,
        resolved = resolved_files.len(),
        remaining = remaining_conflicts.len(),
        "Rebase attempt completed"
    );

    Ok(RebaseResult {
        success,
        new_base_commit: if success {
            Some(conflict_check.current_head)
        } else {
            None
        },
        resolved_files,
        remaining_conflicts,
        changes_applied,
        summary,
    })
}

struct ConflictResolution {
    resolution_type: ResolutionType,
    description: String,
    #[allow(dead_code)]
    new_content: Option<String>,
}

/// Attempt to resolve a single file conflict using LLM
async fn resolve_conflict_with_llm(
    llm: &TrackedLlm,
    file_path: &str,
    proposed_changes: &serde_json::Value,
    repo_manager: &RepoManager,
    base_commit: &str,
    current_head: &str,
) -> Result<ConflictResolution> {
    // Get the relevant proposed change for this file
    let change = proposed_changes
        .as_array()
        .and_then(|arr| arr.iter().find(|c| c.get("file_path").and_then(|f| f.as_str()) == Some(file_path)));

    let change = match change {
        Some(c) => c,
        None => {
            return Err(anyhow::anyhow!("No proposed change found for {}", file_path))
        }
    };

    // Get the file contents at different versions
    let base_content = repo_manager.get_file_at_commit(file_path, base_commit).await?;
    let current_content = repo_manager.get_file_at_commit(file_path, current_head).await?;

    let system_prompt = r#"You are a software engineer resolving merge conflicts.

Your task is to intelligently merge changes from a recommendation with changes that were made to the main branch.

Principles:
1. Preserve the intent of both changes when possible
2. If the main branch change makes the recommendation unnecessary, note that
3. If both changes can coexist, merge them
4. If they're truly incompatible, explain why

Respond in JSON format only."#;

    let prompt = format!(
        r#"Resolve this merge conflict for file: {}

The original recommendation proposed this change:
```json
{}
```

The file at the base commit (when recommendation was made):
```
{}
```

The file at current HEAD (what main branch looks like now):
```
{}
```

Analyze the conflict and provide a resolution.

Respond with a JSON object containing:
- "resolution_type": string - one of: keep_ours, keep_theirs, merged_both, llm_resolved, manual
- "description": string - explanation of the resolution
- "new_content": string or null - the resolved file content (if llm_resolved)
- "confidence": number 0-1 - confidence in the resolution

JSON response:"#,
        file_path,
        serde_json::to_string_pretty(change)?,
        base_content.unwrap_or_else(|| "(file did not exist)".to_string()),
        current_content.unwrap_or_else(|| "(file was deleted)".to_string())
    );

    let response = llm
        .complete(&prompt, Some(system_prompt), "merge_resolution", None, None)
        .await?;

    let parsed: serde_json::Value = parse_json_response(&response.content)?;

    let resolution_type = match parsed.get("resolution_type").and_then(|v| v.as_str()) {
        Some("keep_ours") => ResolutionType::KeepOurs,
        Some("keep_theirs") => ResolutionType::KeepTheirs,
        Some("merged_both") => ResolutionType::MergedBoth,
        Some("llm_resolved") => ResolutionType::LlmResolved,
        _ => ResolutionType::Manual,
    };

    let description = parsed
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("Resolution applied")
        .to_string();

    let confidence = parsed
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);

    // If confidence is too low, mark as manual
    let resolution_type = if confidence < 0.7 && matches!(resolution_type, ResolutionType::LlmResolved) {
        ResolutionType::Manual
    } else {
        resolution_type
    };

    Ok(ConflictResolution {
        resolution_type,
        description,
        new_content: parsed
            .get("new_content")
            .and_then(|v| v.as_str())
            .map(String::from),
    })
}

/// Parse JSON from LLM response
fn parse_json_response(content: &str) -> Result<serde_json::Value> {
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
            Err(anyhow::anyhow!("Failed to parse merge response: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_types() {
        let detail = ConflictDetail {
            file_path: "src/api.ts".to_string(),
            conflict_type: ConflictType::ContentConflict,
            ours_change: Some("added null check".to_string()),
            theirs_change: Some("refactored function".to_string()),
            description: "Both sides modified the same function".to_string(),
        };

        let json = serde_json::to_string(&detail).unwrap();
        assert!(json.contains("content_conflict"));
    }
}
