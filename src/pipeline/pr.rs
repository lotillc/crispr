//! Pull Request Creation Pipeline Stage
//!
//! Creates GitHub PRs with the generated fixes, supporting both single-file
//! and multi-file fixes. Multi-file fixes are committed atomically to ensure
//! all related changes are applied together.

use anyhow::Result;
use std::sync::Arc;

use crate::integrations::github::{GitHubClient, PullRequest};
use crate::integrations::llm::TrackedLlm;
use crate::store::models::{Incident, Project};

use super::fix::GeneratedFix;
use super::repo::RepoManager;

/// Result of PR creation
#[derive(Debug, Clone)]
pub struct PrResult {
    pub url: String,
    pub number: i32,
    pub branch: String,
    pub title: String,
    /// Number of files changed in the fix
    pub files_changed: usize,
    /// Number of test files added
    pub tests_added: usize,
}

/// Create a PR with the generated fix
pub async fn create(
    github: &Arc<GitHubClient>,
    llm: &TrackedLlm,
    project: &Project,
    incident: &Incident,
    fix: &GeneratedFix,
    branch_prefix: &str,
) -> Result<PrResult> {
    let owner = &project.github_owner;
    let repo = &project.github_repo;
    let default_branch = project.default_branch.as_deref().unwrap_or("main");

    // Generate branch name
    let branch_name = format!(
        "{}fix-{}-{}",
        branch_prefix,
        incident.id.to_string().split('-').next().unwrap_or("unknown"),
        sanitize_for_branch(&incident.title.as_deref().unwrap_or("error")[..30.min(incident.title.as_deref().unwrap_or("error").len())])
    );

    // Get the base branch SHA
    let base_ref = github.get_branch(owner, repo, default_branch).await?;
    let base_sha = base_ref.object.sha;

    // Create the new branch
    github.create_branch(owner, repo, &branch_name, &base_sha).await?;

    // Commit each file change
    for change in &fix.changes {
        github.create_or_update_file(
            owner,
            repo,
            &change.path,
            &change.content,
            &format!("fix: {}", change.description),
            &branch_name,
            change.original_sha.as_deref(),
        ).await?;
    }

    // Commit generated test files
    for test in &fix.generated_tests {
        github.create_or_update_file(
            owner,
            repo,
            &test.path,
            &test.content,
            &format!("test: {}", test.description),
            &branch_name,
            None, // Tests are new files
        ).await?;
    }

    // Generate PR title and body
    let pr_title = generate_pr_title(incident, fix);
    let pr_body = generate_pr_body(llm, incident, fix).await?;

    // Log multi-file fix information
    if fix.is_multi_file() {
        tracing::info!(
            file_count = fix.file_count(),
            files = ?fix.affected_files(),
            "Creating PR with multi-file fix"
        );
    }

    // Create the PR
    let pr: PullRequest = github.create_pull_request(
        owner,
        repo,
        &pr_title,
        &pr_body,
        &branch_name,
        default_branch,
    ).await?;

    Ok(PrResult {
        url: pr.html_url,
        number: pr.number,
        branch: branch_name,
        title: pr_title,
        files_changed: fix.file_count(),
        tests_added: fix.test_count(),
    })
}

/// Generate PR title from incident and fix
fn generate_pr_title(incident: &Incident, fix: &GeneratedFix) -> String {
    // Use fix explanation if concise, otherwise use incident title
    if fix.explanation.len() < 60 && !fix.explanation.contains('\n') {
        format!("fix: {}", fix.explanation)
    } else if let Some(title) = &incident.title {
        let short_title = title.chars().take(50).collect::<String>();
        format!("fix: {}", short_title)
    } else {
        format!("fix: error in {}", fix.changes.first().map(|c| c.path.as_str()).unwrap_or("code"))
    }
}

/// Generate PR body with detailed explanation
async fn generate_pr_body(
    _llm: &TrackedLlm,
    incident: &Incident,
    fix: &GeneratedFix,
) -> Result<String> {
    // Build a summary of changes
    let changes_summary: String = fix.changes
        .iter()
        .map(|c| format!("- `{}`: {}", c.path, c.description))
        .collect::<Vec<_>>()
        .join("\n");

    // Add multi-file fix indicator
    let multi_file_note = if fix.is_multi_file() {
        format!(
            "\n> **Note:** This is a multi-file fix affecting {} files. All changes should be reviewed together as they are interdependent.\n",
            fix.file_count()
        )
    } else {
        String::new()
    };

    let risks_section = if fix.risks.is_empty() {
        String::new()
    } else {
        format!("\n## Potential Risks\n{}\n", fix.risks.iter().map(|r| format!("- {}", r)).collect::<Vec<_>>().join("\n"))
    };

    let tests_section = if fix.suggested_tests.is_empty() {
        String::new()
    } else {
        format!("\n## Suggested Test Commands\n```\n{}\n```\n", fix.suggested_tests.join("\n"))
    };

    let generated_tests_section = if fix.generated_tests.is_empty() {
        String::new()
    } else {
        let tests_list: String = fix.generated_tests
            .iter()
            .map(|t| format!("- `{}` ({}): {}", t.path, t.framework, t.description))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n## Generated Tests\nThis PR includes automatically generated unit tests:\n{}\n", tests_list)
    };

    let body = format!(
        r#"## Summary
{}
{}
## Changes ({} file{})
{}
{}{}{}

## Error Details
- **Incident ID**: `{}`
- **Occurrences**: {}
- **First seen**: {}

## How to Test
1. Review the changes in this PR
2. Run the generated tests included in this PR
3. Run the suggested test commands (if any)
4. Deploy to a staging environment and verify the error no longer occurs

---
*This PR was automatically generated by [CRISPR](https://github.com/your-org/crispr) - Continuous Repair and Incident Self-Patching Runtime*

*Confidence: {:.0}%*"#,
        fix.explanation,
        multi_file_note,
        fix.file_count(),
        if fix.file_count() == 1 { "" } else { "s" },
        changes_summary,
        risks_section,
        tests_section,
        generated_tests_section,
        incident.id,
        incident.occurrence_count.unwrap_or(1),
        incident.first_seen.map(|t| t.to_rfc3339()).unwrap_or_else(|| "Unknown".to_string()),
        fix.confidence * 100.0,
    );

    Ok(body)
}

/// Create a PR with the generated fix using local repository
///
/// This function handles both single-file and multi-file fixes. For multi-file fixes,
/// all changes are committed atomically in a single commit to ensure consistency.
pub async fn create_from_local(
    github: &Arc<GitHubClient>,
    repo_manager: &RepoManager,
    llm: &TrackedLlm,
    project: &Project,
    incident: &Incident,
    fix: &GeneratedFix,
    branch_prefix: &str,
) -> Result<PrResult> {
    let owner = &project.github_owner;
    let repo = &project.github_repo;
    let default_branch = project.default_branch.as_deref().unwrap_or("main");

    // Log multi-file fix information
    if fix.is_multi_file() {
        tracing::info!(
            file_count = fix.file_count(),
            files = ?fix.affected_files(),
            "Creating local PR with multi-file fix"
        );
    }

    // Ensure repo is cloned and up to date
    repo_manager.ensure_repo(owner, repo)?;

    // Checkout default branch first
    repo_manager.checkout_default(owner, repo)?;

    // Generate branch name
    let incident_short = incident.id.to_string().split('-').next().unwrap_or("unknown").to_string();
    let title_part = incident.title.as_deref().unwrap_or("error");
    let title_short = &title_part[..30.min(title_part.len())];
    let branch_name = format!(
        "{}fix-{}-{}",
        branch_prefix,
        incident_short,
        sanitize_for_branch(title_short)
    );

    // Create and checkout the new branch
    repo_manager.create_branch(owner, repo, &branch_name)?;

    // Write all the file changes
    let mut changed_files: Vec<String> = Vec::new();
    for change in &fix.changes {
        tracing::debug!(path = %change.path, "Writing fix to file");
        repo_manager.write_file(owner, repo, &change.path, &change.content)?;
        changed_files.push(change.path.clone());
    }

    // Write all generated test files
    let mut test_files: Vec<String> = Vec::new();
    for test in &fix.generated_tests {
        tracing::debug!(path = %test.path, framework = %test.framework, "Writing generated test file");
        repo_manager.write_file(owner, repo, &test.path, &test.content)?;
        test_files.push(test.path.clone());
    }

    // Build commit message for the fix
    // For multi-file fixes, list all affected files in the commit message
    let commit_message = if fix.is_multi_file() {
        let files_list = fix.affected_files().join(", ");
        format!(
            "fix: {}\n\nAffected files ({}):\n{}\n\nIncident: {}\n\nGenerated by CRISPR",
            fix.explanation,
            fix.file_count(),
            fix.changes.iter().map(|c| format!("- {}: {}", c.path, c.description)).collect::<Vec<_>>().join("\n"),
            incident.id
        )
    } else {
        format!(
            "fix: {}\n\nIncident: {}\n\nGenerated by CRISPR",
            fix.explanation,
            incident.id
        )
    };

    // Commit the fix changes (all files in one atomic commit for multi-file fixes)
    let changed_refs: Vec<&str> = changed_files.iter().map(|s| s.as_str()).collect();
    repo_manager.commit(owner, repo, &commit_message, &changed_refs)?;

    tracing::info!(
        file_count = changed_files.len(),
        "Committed fix changes"
    );

    // Commit the test files separately if there are any
    if !test_files.is_empty() {
        let test_commit_message = format!(
            "test: add regression tests for fix\n\nIncident: {}\n\nGenerated by CRISPR",
            incident.id
        );
        let test_refs: Vec<&str> = test_files.iter().map(|s| s.as_str()).collect();
        repo_manager.commit(owner, repo, &test_commit_message, &test_refs)?;

        tracing::info!(
            test_count = test_files.len(),
            "Committed test files"
        );
    }

    // Push the branch
    repo_manager.push(owner, repo, &branch_name)?;

    // Now create the PR via GitHub API
    let pr_title = generate_pr_title(incident, fix);
    let pr_body = generate_pr_body(llm, incident, fix).await?;

    let pr: PullRequest = github.create_pull_request(
        owner,
        repo,
        &pr_title,
        &pr_body,
        &branch_name,
        default_branch,
    ).await?;

    // Checkout back to default branch
    repo_manager.checkout_default(owner, repo)?;

    Ok(PrResult {
        url: pr.html_url,
        number: pr.number,
        branch: branch_name,
        title: pr_title,
        files_changed: fix.file_count(),
        tests_added: fix.test_count(),
    })
}

/// Sanitize a string for use in a git branch name
fn sanitize_for_branch(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_for_branch() {
        assert_eq!(sanitize_for_branch("Hello World!"), "hello-world");
        assert_eq!(sanitize_for_branch("NullPointerException"), "nullpointerexception");
        assert_eq!(sanitize_for_branch("fix: something"), "fix--something");
    }
}
