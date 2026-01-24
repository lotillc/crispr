//! GitHub Issues Integration
//!
//! Creates and updates GitHub issues to track incidents in the same repository.

use anyhow::Result;
use sqlx::PgPool;
use tracing::{debug, info};

use super::{get_external_issue, upsert_external_issue, IncidentStateChange, ProjectIssueTracker};
use crate::integrations::github::GitHubClient;

/// Map CRISPR states to GitHub issue labels
fn state_to_label(state: &str) -> &'static str {
    match state {
        "detected" => "crispr:detected",
        "triaging" => "crispr:triaging",
        "pending_fix" => "crispr:queued",
        "fixing" => "crispr:fixing",
        "pr_opening" | "pr_open" => "crispr:pr-open",
        "pr_merged" => "crispr:pr-merged",
        "verifying" | "pending_verification" => "crispr:verifying",
        "verified" => "crispr:verified",
        "unfixable" => "crispr:unfixable",
        "fix_failed" => "crispr:failed",
        "pr_closed" => "crispr:pr-closed",
        "max_attempts" => "crispr:max-attempts",
        "cooldown" => "crispr:cooldown",
        "ignored" => "crispr:ignored",
        _ => "crispr:unknown",
    }
}

/// Check if state is terminal (issue should be closed)
fn is_terminal_state(state: &str) -> bool {
    matches!(state, "verified" | "ignored")
}

/// Check if state needs attention (add attention label)
fn needs_attention(state: &str) -> bool {
    matches!(
        state,
        "unfixable" | "fix_failed" | "pr_closed" | "max_attempts"
    )
}

/// Build issue body from incident state
fn build_issue_body(event: &IncidentStateChange) -> String {
    let mut body = String::new();

    body.push_str("## CRISPR Incident\n\n");

    if let Some(title) = &event.title {
        body.push_str(&format!("**Error:** {}\n\n", title));
    }

    body.push_str(&format!("**Status:** `{}`\n", event.new_state));
    body.push_str(&format!("**Fingerprint:** `{}`\n", event.fingerprint));
    body.push_str(&format!("**Fix Attempts:** {}\n", event.fix_attempts));

    if let Some(duration) = event.verification_duration_hours {
        body.push_str(&format!("**Verification Period:** {} hours\n", duration));
    }

    if let Some(pr_url) = &event.pr_url {
        body.push_str(&format!("\n### Pull Request\n\n{}\n", pr_url));
    }

    body.push_str("\n---\n");
    body.push_str("*This issue is managed by [CRISPR](https://github.com/your-org/crispr). ");
    body.push_str("Do not close manually - it will be closed automatically when the fix is verified.*\n");

    body
}

/// Build issue title from incident
fn build_issue_title(event: &IncidentStateChange) -> String {
    let prefix = "[CRISPR]";
    if let Some(title) = &event.title {
        // Truncate to avoid GitHub's 256 char limit
        let max_len = 200;
        if title.len() > max_len {
            format!("{} {}...", prefix, &title[..max_len])
        } else {
            format!("{} {}", prefix, title)
        }
    } else {
        format!("{} Incident {}", prefix, &event.fingerprint[..8])
    }
}

/// Sync incident to GitHub Issues
pub async fn sync_to_github(
    db: &PgPool,
    github: &GitHubClient,
    config: &ProjectIssueTracker,
    event: &IncidentStateChange,
) -> Result<()> {
    let owner = &event.github_owner;
    let repo = &event.github_repo;

    // Check if we already have an issue for this incident
    let existing = get_external_issue(db, event.incident_id, "github").await?;

    // Build labels
    let mut labels = vec![
        "crispr".to_string(),
        state_to_label(&event.new_state).to_string(),
    ];

    // Add configured labels
    if let Some(extra_labels) = &config.github_labels {
        labels.extend(extra_labels.clone());
    }

    // Add attention label if needed
    if needs_attention(&event.new_state) {
        labels.push("needs-attention".to_string());
    }

    // Remove old state labels (keep only current)
    labels.retain(|l| !l.starts_with("crispr:") || l == state_to_label(&event.new_state));

    let title = build_issue_title(event);
    let body = build_issue_body(event);

    if let Some(existing_issue) = existing {
        // Update existing issue
        let issue_number: i32 = existing_issue
            .external_id
            .as_ref()
            .and_then(|id| id.parse().ok())
            .unwrap_or(0);

        if issue_number > 0 {
            // Update issue body and labels
            github
                .update_issue(owner, repo, issue_number, Some(&title), Some(&body), Some(&labels))
                .await?;

            // Close if terminal state
            if is_terminal_state(&event.new_state) {
                github.close_issue(owner, repo, issue_number).await?;
            }

            // Update our record
            upsert_external_issue(
                db,
                event.incident_id,
                "github",
                &issue_number.to_string(),
                &format!("https://github.com/{}/{}/issues/{}", owner, repo, issue_number),
                Some(&event.new_state),
            )
            .await?;

            info!(
                incident_id = %event.incident_id,
                issue_number = issue_number,
                state = %event.new_state,
                "Updated GitHub issue"
            );
        }
    } else {
        // Create new issue
        let assignees = config.github_assignees.clone().unwrap_or_default();

        let issue = github
            .create_issue(owner, repo, &title, &body, &labels, &assignees)
            .await?;

        // Record the external issue
        upsert_external_issue(
            db,
            event.incident_id,
            "github",
            &issue.number.to_string(),
            &issue.html_url,
            Some(&event.new_state),
        )
        .await?;

        info!(
            incident_id = %event.incident_id,
            issue_number = issue.number,
            issue_url = %issue.html_url,
            "Created GitHub issue"
        );
    }

    Ok(())
}

/// GitHub issue response from API
#[derive(Debug, serde::Deserialize)]
pub struct GitHubIssue {
    pub number: i32,
    pub html_url: String,
    pub state: String,
}
