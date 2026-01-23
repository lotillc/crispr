//! Context Gathering Pipeline Stage
//!
//! Fetches relevant code from the repository to provide context for fix generation.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::integrations::github::GitHubClient;
use crate::integrations::llm::TrackedLlm;
use crate::store::models::Project;

use super::repo::RepoManager;
use super::triage::TriageResult;

/// Gathered context for fix generation
#[derive(Debug, Clone, Serialize)]
pub struct FixContext {
    /// Project information
    pub project: ProjectInfo,
    /// Triage result
    pub triage: TriageResult,
    /// Relevant source files with content
    pub files: Vec<SourceFile>,
    /// Error sample
    pub sample: serde_json::Value,
    /// Additional context from LLM analysis
    pub analysis: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectInfo {
    pub owner: String,
    pub repo: String,
    pub default_branch: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
    pub sha: String,
}

const CONTEXT_SYSTEM_PROMPT: &str = r#"You are analyzing code context for an error fix. Your job is to:
1. Identify the root cause location
2. Understand the code flow that led to the error
3. Note any relevant patterns or dependencies

Be concise and focus on what's needed to fix the error."#;

/// Gather context for fix generation
pub async fn gather(
    llm: &TrackedLlm,
    github: &Arc<GitHubClient>,
    project: &Project,
    triage: &TriageResult,
    sample: &serde_json::Value,
    incident_id: Uuid,
) -> Result<FixContext> {
    let owner = &project.github_owner;
    let repo = &project.github_repo;
    let default_branch = project.default_branch.as_deref().unwrap_or("main");

    // Collect files to fetch
    let mut files_to_fetch: Vec<String> = triage.suggested_files.clone();

    // Also extract files from stack trace
    if let Some(stack) = sample.get("stack_trace").and_then(|v| v.as_str()) {
        files_to_fetch.extend(extract_files_from_stack(stack));
    }

    // Deduplicate
    files_to_fetch.sort();
    files_to_fetch.dedup();

    // Limit to reasonable number of files
    files_to_fetch.truncate(10);

    // Fetch file contents
    let mut files = Vec::new();
    for path in &files_to_fetch {
        match github.get_file(owner, repo, path, Some(default_branch)).await {
            Ok(content) => {
                if let Ok(decoded) = content.decoded_content() {
                    files.push(SourceFile {
                        path: path.clone(),
                        content: decoded,
                        sha: content.sha,
                    });
                }
            }
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "Failed to fetch file");
            }
        }
    }

    // If no files found, try to find related files via GitHub search
    if files.is_empty() {
        if let Some(message) = sample.get("message").and_then(|v| v.as_str()) {
            // Extract potential identifiers from error message
            let search_terms = extract_search_terms(message);
            for term in search_terms.iter().take(3) {
                if let Ok(search_result) = github.search_code(owner, repo, term).await {
                    for item in search_result.items.iter().take(3) {
                        if let Ok(content) = github.get_file(owner, repo, &item.path, Some(default_branch)).await {
                            if let Ok(decoded) = content.decoded_content() {
                                files.push(SourceFile {
                                    path: item.path.clone(),
                                    content: decoded,
                                    sha: content.sha,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // Get LLM analysis of the context if we have files
    let analysis = if !files.is_empty() {
        let files_summary: String = files
            .iter()
            .map(|f| format!("### {}\n```\n{}\n```", f.path, truncate_content(&f.content, 2000)))
            .collect::<Vec<_>>()
            .join("\n\n");

        let prompt = format!(
            r#"Analyze this code context for the error:

Error: {}
Category: {}

Files:
{}

Briefly explain:
1. Where the error is occurring
2. What's causing it
3. Key code patterns to be aware of when fixing"#,
            triage.description,
            triage.category,
            files_summary
        );

        match llm.complete(&prompt, Some(CONTEXT_SYSTEM_PROMPT), "context", Some(incident_id), Some(project.id)).await {
            Ok(response) => Some(response.content),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to get context analysis");
                None
            }
        }
    } else {
        None
    };

    Ok(FixContext {
        project: ProjectInfo {
            owner: owner.clone(),
            repo: repo.clone(),
            default_branch: default_branch.to_string(),
            language: project.language.clone(),
        },
        triage: triage.clone(),
        files,
        sample: sample.clone(),
        analysis,
    })
}

/// Gather context for fix generation using local repository clone
pub async fn gather_from_local(
    llm: &TrackedLlm,
    repo_manager: &RepoManager,
    project: &Project,
    triage: &TriageResult,
    sample: &serde_json::Value,
    incident_id: Uuid,
    crispr_context: &str,
) -> Result<FixContext> {
    let owner = &project.github_owner;
    let repo = &project.github_repo;
    let default_branch = project.default_branch.as_deref().unwrap_or("main");

    // Ensure repo is cloned and up to date
    repo_manager.ensure_repo(owner, repo)?;

    // Collect files to fetch
    let mut files_to_fetch: Vec<String> = triage.suggested_files.clone();

    // Also extract files from stack trace
    if let Some(stack) = sample.get("stack_trace").and_then(|v| v.as_str()) {
        files_to_fetch.extend(extract_files_from_stack(stack));
    }

    // Deduplicate
    files_to_fetch.sort();
    files_to_fetch.dedup();

    // Limit to reasonable number of files
    files_to_fetch.truncate(10);

    // Fetch file contents from local clone
    let mut files = Vec::new();
    for path in &files_to_fetch {
        match repo_manager.read_file(owner, repo, path) {
            Ok(content) => {
                // Generate a pseudo-SHA from the content
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(content.as_bytes());
                let sha = format!("{:x}", hasher.finalize())[..7].to_string();

                files.push(SourceFile {
                    path: path.clone(),
                    content,
                    sha,
                });
            }
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "Failed to read file from local repo");
            }
        }
    }

    // If no files found, try to find related files by searching the local repo
    if files.is_empty() {
        if let Some(message) = sample.get("message").and_then(|v| v.as_str()) {
            let search_terms = extract_search_terms(message);
            for term in search_terms.iter().take(3) {
                // Simple local file search using glob patterns
                let repo_path = repo_manager.repo_path(owner, repo);
                let pattern = format!("{}/**/*", repo_path.display());
                if let Ok(entries) = glob::glob(&pattern) {
                    for entry in entries.flatten().take(3) {
                        if let Ok(content) = std::fs::read_to_string(&entry) {
                            if content.contains(term) {
                                if let Ok(rel_path) = entry.strip_prefix(&repo_path) {
                                    use sha2::{Sha256, Digest};
                                    let mut hasher = Sha256::new();
                                    hasher.update(content.as_bytes());
                                    let sha = format!("{:x}", hasher.finalize())[..7].to_string();

                                    files.push(SourceFile {
                                        path: rel_path.to_string_lossy().to_string(),
                                        content,
                                        sha,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Get LLM analysis of the context if we have files
    let analysis = if !files.is_empty() {
        let files_summary: String = files
            .iter()
            .map(|f| format!("### {}\n```\n{}\n```", f.path, truncate_content(&f.content, 2000)))
            .collect::<Vec<_>>()
            .join("\n\n");

        let context_section = if crispr_context.is_empty() {
            String::new()
        } else {
            format!("\n{}\n", crispr_context)
        };

        let prompt = format!(
            r#"Analyze this code context for the error:
{context_section}
Error: {}
Category: {}

Files:
{}

Briefly explain:
1. Where the error is occurring
2. What's causing it
3. Key code patterns to be aware of when fixing"#,
            triage.description,
            triage.category,
            files_summary
        );

        match llm.complete(&prompt, Some(CONTEXT_SYSTEM_PROMPT), "context", Some(incident_id), Some(project.id)).await {
            Ok(response) => Some(response.content),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to get context analysis");
                None
            }
        }
    } else {
        None
    };

    Ok(FixContext {
        project: ProjectInfo {
            owner: owner.clone(),
            repo: repo.clone(),
            default_branch: default_branch.to_string(),
            language: project.language.clone(),
        },
        triage: triage.clone(),
        files,
        sample: sample.clone(),
        analysis,
    })
}

/// Extract file paths from a stack trace
fn extract_files_from_stack(stack: &str) -> Vec<String> {
    let mut files = Vec::new();

    // Common patterns for file paths in stack traces
    let patterns = [
        // JavaScript/TypeScript: at Function (file.js:10:5)
        regex::Regex::new(r"\(([^)]+\.[jt]sx?):?\d*:?\d*\)").unwrap(),
        // JavaScript/TypeScript: at file.js:10:5
        regex::Regex::new(r"at\s+([^\s]+\.[jt]sx?):\d+").unwrap(),
        // Python: File "path/to/file.py", line 10
        regex::Regex::new(r#"File "([^"]+\.py)""#).unwrap(),
        // Java: at com.example.Class.method(File.java:10)
        regex::Regex::new(r"\(([^)]+\.java):?\d*\)").unwrap(),
        // Go: file.go:10
        regex::Regex::new(r"([^\s]+\.go):\d+").unwrap(),
        // Rust: src/main.rs:10
        regex::Regex::new(r"([^\s]+\.rs):\d+").unwrap(),
    ];

    for pattern in &patterns {
        for cap in pattern.captures_iter(stack) {
            if let Some(path) = cap.get(1) {
                let path = path.as_str();
                // Clean up the path
                let path = path
                    .trim_start_matches('/')
                    .trim_start_matches("./")
                    .replace("\\", "/");

                // Skip node_modules and other vendor paths
                if !path.contains("node_modules")
                    && !path.contains("vendor")
                    && !path.contains("site-packages")
                {
                    files.push(path);
                }
            }
        }
    }

    files
}

/// Extract search terms from an error message
fn extract_search_terms(message: &str) -> Vec<String> {
    let mut terms = Vec::new();

    // Look for identifiers (camelCase, PascalCase, snake_case)
    let identifier_re = regex::Regex::new(r"\b([A-Z][a-z]+[A-Z][a-zA-Z]*|[a-z]+_[a-z_]+|[A-Z][a-zA-Z]+)\b").unwrap();

    for cap in identifier_re.captures_iter(message) {
        if let Some(term) = cap.get(1) {
            let term = term.as_str();
            if term.len() > 3 && term.len() < 50 {
                terms.push(term.to_string());
            }
        }
    }

    // Deduplicate and limit
    terms.sort();
    terms.dedup();
    terms.truncate(5);

    terms
}

/// Truncate content to a maximum length
fn truncate_content(content: &str, max_len: usize) -> String {
    if content.len() <= max_len {
        content.to_string()
    } else {
        format!("{}...\n[truncated]", &content[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_files_from_stack() {
        let stack = r#"
TypeError: Cannot read property 'userId' of undefined
    at UserController.getProfile (src/controllers/user.ts:45:23)
    at Router.handle (/app/node_modules/express/lib/router/index.js:174:12)
    at processTicksAndRejections (internal/process/task_queues.js:93:5)
"#;

        let files = extract_files_from_stack(stack);
        assert!(files.contains(&"src/controllers/user.ts".to_string()));
        assert!(!files.iter().any(|f| f.contains("node_modules")));
    }

    #[test]
    fn test_extract_search_terms() {
        let message = "NullPointerException in UserService.getUser when accessing userProfile";
        let terms = extract_search_terms(message);
        assert!(terms.contains(&"UserService".to_string()));
        assert!(terms.contains(&"userProfile".to_string()));
    }
}
