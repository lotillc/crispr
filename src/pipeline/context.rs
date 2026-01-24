//! Context Gathering Pipeline Stage
//!
//! Fetches relevant code from the repository to provide context for fix generation.
//!
//! This module implements intelligent context gathering:
//! - Parses stack traces to extract file paths and line numbers
//! - Fetches surrounding context (configurable lines around error locations)
//! - Discovers related files (models, services, handlers with same name)
//! - Follows imports to find dependent modules
//! - Includes CRISPR.md context file if present
//! - Enforces configurable size limits to avoid overwhelming the LLM

use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

use crate::integrations::github::GitHubClient;
use crate::integrations::llm::TrackedLlm;
use crate::store::models::Project;

use super::crispr_md::CrisprMdManager;
use super::repo::RepoManager;
use super::triage::TriageResult;

/// Default maximum context size in bytes (50KB)
pub const DEFAULT_MAX_CONTEXT_SIZE: usize = 50 * 1024;

/// Default number of lines to include around error locations
pub const DEFAULT_CONTEXT_LINES: usize = 50;

/// Configuration for context gathering
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Maximum total context size in bytes
    pub max_context_size: usize,
    /// Number of lines to include before and after error locations
    pub context_lines: usize,
    /// Whether to follow imports to find related files
    pub follow_imports: bool,
    /// Whether to look for related files (models, services, etc.)
    pub find_related_files: bool,
    /// Whether to include CRISPR.md context
    pub include_crispr_context: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_context_size: DEFAULT_MAX_CONTEXT_SIZE,
            context_lines: DEFAULT_CONTEXT_LINES,
            follow_imports: true,
            find_related_files: true,
            include_crispr_context: true,
        }
    }
}

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
    /// CRISPR.md context if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crispr_context: Option<String>,
    /// Total context size in bytes
    pub context_size_bytes: usize,
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
    /// Line numbers highlighted from stack trace (if any)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub error_lines: Vec<usize>,
    /// Whether this file was truncated to fit context limits
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

/// Parsed stack frame with file and line information
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub file_path: String,
    pub line_number: Option<usize>,
    pub column: Option<usize>,
    pub function_name: Option<String>,
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
    gather_with_config(
        llm,
        github,
        project,
        triage,
        sample,
        incident_id,
        ContextConfig::default(),
    )
    .await
}

/// Gather context for fix generation with custom configuration
pub async fn gather_with_config(
    llm: &TrackedLlm,
    github: &Arc<GitHubClient>,
    project: &Project,
    triage: &TriageResult,
    sample: &serde_json::Value,
    incident_id: Uuid,
    config: ContextConfig,
) -> Result<FixContext> {
    let owner = &project.github_owner;
    let repo = &project.github_repo;
    let default_branch = project.default_branch.as_deref().unwrap_or("main");

    // Parse stack trace to get files with line numbers
    let stack_frames = if let Some(stack) = sample.get("stack_trace").and_then(|v| v.as_str()) {
        parse_stack_trace(stack)
    } else {
        vec![]
    };

    // Build a map of file paths to their error lines
    let mut file_error_lines: HashMap<String, Vec<usize>> = HashMap::new();
    for frame in &stack_frames {
        if let Some(line) = frame.line_number {
            file_error_lines
                .entry(frame.file_path.clone())
                .or_default()
                .push(line);
        }
    }

    // Collect files to fetch, starting with stack trace files
    let mut files_to_fetch: HashSet<String> = stack_frames
        .iter()
        .map(|f| f.file_path.clone())
        .collect();

    // Add suggested files from triage
    for path in &triage.suggested_files {
        files_to_fetch.insert(path.clone());
    }

    // Find related files (models, services, handlers)
    if config.find_related_files {
        let related = find_related_file_paths(&files_to_fetch);
        for path in related {
            files_to_fetch.insert(path);
        }
    }

    // Limit initial file count
    let files_to_fetch: Vec<String> = files_to_fetch.into_iter().take(15).collect();

    // Track total context size
    let mut total_size: usize = 0;
    let mut files = Vec::new();

    // Fetch file contents
    for path in &files_to_fetch {
        if total_size >= config.max_context_size {
            tracing::info!(
                path = %path,
                total_size,
                max_size = config.max_context_size,
                "Skipping file due to context size limit"
            );
            break;
        }

        match github.get_file(owner, repo, path, Some(default_branch)).await {
            Ok(content) => {
                if let Ok(decoded) = content.decoded_content() {
                    let error_lines = file_error_lines.get(path).cloned().unwrap_or_default();

                    // Extract surrounding context if we have error lines
                    let (processed_content, truncated) = if !error_lines.is_empty() {
                        extract_surrounding_context(
                            &decoded,
                            &error_lines,
                            config.context_lines,
                            config.max_context_size.saturating_sub(total_size),
                        )
                    } else {
                        // For files without specific error lines, include whole file if possible
                        let remaining = config.max_context_size.saturating_sub(total_size);
                        if decoded.len() <= remaining {
                            (decoded.clone(), false)
                        } else {
                            (truncate_content(&decoded, remaining), true)
                        }
                    };

                    total_size += processed_content.len();

                    files.push(SourceFile {
                        path: path.clone(),
                        content: processed_content,
                        sha: content.sha,
                        error_lines,
                        truncated,
                    });
                }
            }
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "Failed to fetch file");
            }
        }
    }

    // Follow imports if enabled and we have budget
    if config.follow_imports && total_size < config.max_context_size {
        let import_paths = extract_imports_from_files(&files, project.language.as_deref());
        for path in import_paths.iter().take(5) {
            if total_size >= config.max_context_size {
                break;
            }
            // Skip if we already have this file
            if files.iter().any(|f| &f.path == path) {
                continue;
            }

            match github.get_file(owner, repo, path, Some(default_branch)).await {
                Ok(content) => {
                    if let Ok(decoded) = content.decoded_content() {
                        let remaining = config.max_context_size.saturating_sub(total_size);
                        let (processed_content, truncated) = if decoded.len() <= remaining {
                            (decoded, false)
                        } else {
                            (truncate_content(&decoded, remaining), true)
                        };

                        total_size += processed_content.len();

                        files.push(SourceFile {
                            path: path.clone(),
                            content: processed_content,
                            sha: content.sha,
                            error_lines: vec![],
                            truncated,
                        });
                    }
                }
                Err(e) => {
                    tracing::debug!(path = %path, error = %e, "Failed to fetch imported file");
                }
            }
        }
    }

    // If no files found, try to find related files via GitHub search
    if files.is_empty() {
        if let Some(message) = sample.get("message").and_then(|v| v.as_str()) {
            let search_terms = extract_search_terms(message);
            for term in search_terms.iter().take(3) {
                if let Ok(search_result) = github.search_code(owner, repo, term).await {
                    for item in search_result.items.iter().take(3) {
                        if total_size >= config.max_context_size {
                            break;
                        }
                        if let Ok(content) = github.get_file(owner, repo, &item.path, Some(default_branch)).await {
                            if let Ok(decoded) = content.decoded_content() {
                                let remaining = config.max_context_size.saturating_sub(total_size);
                                let (processed_content, truncated) = if decoded.len() <= remaining {
                                    (decoded, false)
                                } else {
                                    (truncate_content(&decoded, remaining), true)
                                };

                                total_size += processed_content.len();

                                files.push(SourceFile {
                                    path: item.path.clone(),
                                    content: processed_content,
                                    sha: content.sha,
                                    error_lines: vec![],
                                    truncated,
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
            .map(|f| {
                let line_info = if !f.error_lines.is_empty() {
                    format!(" (error at lines: {:?})", f.error_lines)
                } else {
                    String::new()
                };
                format!("### {}{}\n```\n{}\n```", f.path, line_info, truncate_content(&f.content, 2000))
            })
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
        crispr_context: None, // Not available in GitHub-only mode
        context_size_bytes: total_size,
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
    gather_from_local_with_config(
        llm,
        repo_manager,
        project,
        triage,
        sample,
        incident_id,
        crispr_context,
        ContextConfig::default(),
    )
    .await
}

/// Gather context for fix generation using local repository clone with custom configuration
pub async fn gather_from_local_with_config(
    llm: &TrackedLlm,
    repo_manager: &RepoManager,
    project: &Project,
    triage: &TriageResult,
    sample: &serde_json::Value,
    incident_id: Uuid,
    crispr_context: &str,
    config: ContextConfig,
) -> Result<FixContext> {
    let owner = &project.github_owner;
    let repo = &project.github_repo;
    let default_branch = project.default_branch.as_deref().unwrap_or("main");

    // Ensure repo is cloned and up to date
    repo_manager.ensure_repo(owner, repo)?;

    // Parse stack trace to get files with line numbers
    let stack_frames = if let Some(stack) = sample.get("stack_trace").and_then(|v| v.as_str()) {
        parse_stack_trace(stack)
    } else {
        vec![]
    };

    // Build a map of file paths to their error lines
    let mut file_error_lines: HashMap<String, Vec<usize>> = HashMap::new();
    for frame in &stack_frames {
        if let Some(line) = frame.line_number {
            file_error_lines
                .entry(frame.file_path.clone())
                .or_default()
                .push(line);
        }
    }

    // Collect files to fetch, starting with stack trace files
    let mut files_to_fetch: HashSet<String> = stack_frames
        .iter()
        .map(|f| f.file_path.clone())
        .collect();

    // Add suggested files from triage
    for path in &triage.suggested_files {
        files_to_fetch.insert(path.clone());
    }

    // Find related files (models, services, handlers)
    if config.find_related_files {
        let related = find_related_file_paths(&files_to_fetch);
        for path in related {
            files_to_fetch.insert(path);
        }
    }

    // Track total context size, reserve space for CRISPR context
    let crispr_context_size = if config.include_crispr_context && !crispr_context.is_empty() {
        crispr_context.len()
    } else {
        0
    };
    let file_budget = config.max_context_size.saturating_sub(crispr_context_size);
    let mut total_size: usize = 0;
    let mut files = Vec::new();

    // Prioritize files: stack trace files first (sorted by frame order), then others
    let stack_file_order: Vec<String> = stack_frames.iter().map(|f| f.file_path.clone()).collect();
    let mut ordered_files: Vec<String> = vec![];

    // Add stack trace files in order they appear
    for path in &stack_file_order {
        if files_to_fetch.contains(path) && !ordered_files.contains(path) {
            ordered_files.push(path.clone());
        }
    }

    // Add remaining files
    for path in &files_to_fetch {
        if !ordered_files.contains(path) {
            ordered_files.push(path.clone());
        }
    }

    // Limit initial file count
    ordered_files.truncate(15);

    // Fetch file contents from local clone
    for path in &ordered_files {
        if total_size >= file_budget {
            tracing::info!(
                path = %path,
                total_size,
                max_size = file_budget,
                "Skipping file due to context size limit"
            );
            break;
        }

        match repo_manager.read_file(owner, repo, path) {
            Ok(content) => {
                let error_lines = file_error_lines.get(path).cloned().unwrap_or_default();

                // Extract surrounding context if we have error lines
                let (processed_content, truncated) = if !error_lines.is_empty() {
                    extract_surrounding_context(
                        &content,
                        &error_lines,
                        config.context_lines,
                        file_budget.saturating_sub(total_size),
                    )
                } else {
                    // For files without specific error lines, include whole file if possible
                    let remaining = file_budget.saturating_sub(total_size);
                    if content.len() <= remaining {
                        (content.clone(), false)
                    } else {
                        (truncate_content(&content, remaining), true)
                    }
                };

                // Generate a pseudo-SHA from the content
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(content.as_bytes());
                let sha = format!("{:x}", hasher.finalize())[..7].to_string();

                total_size += processed_content.len();

                files.push(SourceFile {
                    path: path.clone(),
                    content: processed_content,
                    sha,
                    error_lines,
                    truncated,
                });
            }
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "Failed to read file from local repo");
            }
        }
    }

    // Follow imports if enabled and we have budget
    if config.follow_imports && total_size < file_budget {
        let import_paths = extract_imports_from_files(&files, project.language.as_deref());
        for path in import_paths.iter().take(5) {
            if total_size >= file_budget {
                break;
            }
            // Skip if we already have this file
            if files.iter().any(|f| &f.path == path) {
                continue;
            }

            match repo_manager.read_file(owner, repo, path) {
                Ok(content) => {
                    let remaining = file_budget.saturating_sub(total_size);
                    let (processed_content, truncated) = if content.len() <= remaining {
                        (content.clone(), false)
                    } else {
                        (truncate_content(&content, remaining), true)
                    };

                    use sha2::{Sha256, Digest};
                    let mut hasher = Sha256::new();
                    hasher.update(content.as_bytes());
                    let sha = format!("{:x}", hasher.finalize())[..7].to_string();

                    total_size += processed_content.len();

                    files.push(SourceFile {
                        path: path.clone(),
                        content: processed_content,
                        sha,
                        error_lines: vec![],
                        truncated,
                    });
                }
                Err(e) => {
                    tracing::debug!(path = %path, error = %e, "Failed to read imported file");
                }
            }
        }
    }

    // If no files found, try to find related files by searching the local repo
    if files.is_empty() {
        if let Some(message) = sample.get("message").and_then(|v| v.as_str()) {
            let search_terms = extract_search_terms(message);
            for term in search_terms.iter().take(3) {
                if total_size >= file_budget {
                    break;
                }
                // Simple local file search using glob patterns
                let repo_path = repo_manager.repo_path(owner, repo);
                let pattern = format!("{}/**/*", repo_path.display());
                if let Ok(entries) = glob::glob(&pattern) {
                    for entry in entries.flatten().take(3) {
                        if total_size >= file_budget {
                            break;
                        }
                        if let Ok(content) = std::fs::read_to_string(&entry) {
                            if content.contains(term) {
                                if let Ok(rel_path) = entry.strip_prefix(&repo_path) {
                                    let remaining = file_budget.saturating_sub(total_size);
                                    let (processed_content, truncated) = if content.len() <= remaining {
                                        (content.clone(), false)
                                    } else {
                                        (truncate_content(&content, remaining), true)
                                    };

                                    use sha2::{Sha256, Digest};
                                    let mut hasher = Sha256::new();
                                    hasher.update(content.as_bytes());
                                    let sha = format!("{:x}", hasher.finalize())[..7].to_string();

                                    total_size += processed_content.len();

                                    files.push(SourceFile {
                                        path: rel_path.to_string_lossy().to_string(),
                                        content: processed_content,
                                        sha,
                                        error_lines: vec![],
                                        truncated,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Include CRISPR.md context if enabled
    let final_crispr_context = if config.include_crispr_context && !crispr_context.is_empty() {
        Some(crispr_context.to_string())
    } else {
        // Try to read CRISPR.md if not provided
        if config.include_crispr_context {
            let crispr_manager = CrisprMdManager::new(repo_manager);
            match crispr_manager.get_context_for_prompt(owner, repo) {
                Ok(ctx) if !ctx.is_empty() => Some(ctx),
                _ => None,
            }
        } else {
            None
        }
    };

    // Get LLM analysis of the context if we have files
    let analysis = if !files.is_empty() {
        let files_summary: String = files
            .iter()
            .map(|f| {
                let line_info = if !f.error_lines.is_empty() {
                    format!(" (error at lines: {:?})", f.error_lines)
                } else {
                    String::new()
                };
                format!("### {}{}\n```\n{}\n```", f.path, line_info, truncate_content(&f.content, 2000))
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let context_section = if let Some(ref ctx) = final_crispr_context {
            format!("\n{}\n", ctx)
        } else {
            String::new()
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

    let total_context_size = total_size + crispr_context_size;

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
        crispr_context: final_crispr_context,
        context_size_bytes: total_context_size,
    })
}

/// Parse a stack trace and extract structured stack frames with file paths and line numbers
pub fn parse_stack_trace(stack: &str) -> Vec<StackFrame> {
    let mut frames = Vec::new();
    let mut seen_paths = HashSet::new();

    // JavaScript/TypeScript patterns
    // at Function (file.js:10:5) or at file.js:10:5 or at Object.<anonymous> (file.ts:42:13)
    let js_paren_re = Regex::new(r"at\s+(?:([^\s(]+)\s+)?\(([^)]+\.[jt]sx?):(\d+)(?::(\d+))?\)").unwrap();
    let js_direct_re = Regex::new(r"at\s+([^\s]+\.[jt]sx?):(\d+)(?::(\d+))?").unwrap();

    // Python: File "path/to/file.py", line 10, in function_name
    let python_re = Regex::new(r#"File "([^"]+\.py)", line (\d+)(?:, in (\w+))?"#).unwrap();

    // Java/Kotlin: at com.example.Class.method(File.java:10)
    let java_re = Regex::new(r"at\s+([^\s(]+)\(([^)]+\.(java|kt)):(\d+)\)").unwrap();

    // Go: /path/to/file.go:10 or file.go:10 +0x...
    let go_re = Regex::new(r"([^\s]+\.go):(\d+)(?:\s+\+0x)?").unwrap();

    // Rust: at src/main.rs:10:5 or thread 'main' panicked at src/main.rs:10:5
    let rust_re = Regex::new(r"(?:at\s+)?([^\s]+\.rs):(\d+)(?::(\d+))?").unwrap();

    // Ruby: from /path/to/file.rb:10:in `method_name'
    let ruby_re = Regex::new(r"from\s+([^\s:]+\.rb):(\d+)(?::in\s+`([^']+)')?").unwrap();

    // PHP: #0 /path/to/file.php(10): function()
    let php_re = Regex::new(r"#\d+\s+([^\s(]+\.php)\((\d+)\)").unwrap();

    // C#/.NET: at Namespace.Class.Method(args) in /path/to/File.cs:line 10
    let csharp_re = Regex::new(r"at\s+([^\s]+)\([^)]*\)\s+in\s+([^:]+\.cs):line\s+(\d+)").unwrap();

    for line in stack.lines() {
        // Try JavaScript/TypeScript with parentheses
        if let Some(cap) = js_paren_re.captures(line) {
            let file_path = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let cleaned = clean_file_path(file_path);
            if should_include_path(&cleaned) && !seen_paths.contains(&cleaned) {
                seen_paths.insert(cleaned.clone());
                frames.push(StackFrame {
                    file_path: cleaned,
                    line_number: cap.get(3).and_then(|m| m.as_str().parse().ok()),
                    column: cap.get(4).and_then(|m| m.as_str().parse().ok()),
                    function_name: cap.get(1).map(|m| m.as_str().to_string()),
                });
            }
            continue;
        }

        // Try JavaScript/TypeScript direct
        if let Some(cap) = js_direct_re.captures(line) {
            let file_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let cleaned = clean_file_path(file_path);
            if should_include_path(&cleaned) && !seen_paths.contains(&cleaned) {
                seen_paths.insert(cleaned.clone());
                frames.push(StackFrame {
                    file_path: cleaned,
                    line_number: cap.get(2).and_then(|m| m.as_str().parse().ok()),
                    column: cap.get(3).and_then(|m| m.as_str().parse().ok()),
                    function_name: None,
                });
            }
            continue;
        }

        // Try Python
        if let Some(cap) = python_re.captures(line) {
            let file_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let cleaned = clean_file_path(file_path);
            if should_include_path(&cleaned) && !seen_paths.contains(&cleaned) {
                seen_paths.insert(cleaned.clone());
                frames.push(StackFrame {
                    file_path: cleaned,
                    line_number: cap.get(2).and_then(|m| m.as_str().parse().ok()),
                    column: None,
                    function_name: cap.get(3).map(|m| m.as_str().to_string()),
                });
            }
            continue;
        }

        // Try Java/Kotlin
        if let Some(cap) = java_re.captures(line) {
            let file_path = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let cleaned = clean_file_path(file_path);
            if should_include_path(&cleaned) && !seen_paths.contains(&cleaned) {
                seen_paths.insert(cleaned.clone());
                frames.push(StackFrame {
                    file_path: cleaned,
                    line_number: cap.get(4).and_then(|m| m.as_str().parse().ok()),
                    column: None,
                    function_name: cap.get(1).map(|m| m.as_str().to_string()),
                });
            }
            continue;
        }

        // Try Go
        if let Some(cap) = go_re.captures(line) {
            let file_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let cleaned = clean_file_path(file_path);
            if should_include_path(&cleaned) && !seen_paths.contains(&cleaned) {
                seen_paths.insert(cleaned.clone());
                frames.push(StackFrame {
                    file_path: cleaned,
                    line_number: cap.get(2).and_then(|m| m.as_str().parse().ok()),
                    column: None,
                    function_name: None,
                });
            }
            continue;
        }

        // Try Rust
        if let Some(cap) = rust_re.captures(line) {
            let file_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let cleaned = clean_file_path(file_path);
            if should_include_path(&cleaned) && !seen_paths.contains(&cleaned) {
                seen_paths.insert(cleaned.clone());
                frames.push(StackFrame {
                    file_path: cleaned,
                    line_number: cap.get(2).and_then(|m| m.as_str().parse().ok()),
                    column: cap.get(3).and_then(|m| m.as_str().parse().ok()),
                    function_name: None,
                });
            }
            continue;
        }

        // Try Ruby
        if let Some(cap) = ruby_re.captures(line) {
            let file_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let cleaned = clean_file_path(file_path);
            if should_include_path(&cleaned) && !seen_paths.contains(&cleaned) {
                seen_paths.insert(cleaned.clone());
                frames.push(StackFrame {
                    file_path: cleaned,
                    line_number: cap.get(2).and_then(|m| m.as_str().parse().ok()),
                    column: None,
                    function_name: cap.get(3).map(|m| m.as_str().to_string()),
                });
            }
            continue;
        }

        // Try PHP
        if let Some(cap) = php_re.captures(line) {
            let file_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let cleaned = clean_file_path(file_path);
            if should_include_path(&cleaned) && !seen_paths.contains(&cleaned) {
                seen_paths.insert(cleaned.clone());
                frames.push(StackFrame {
                    file_path: cleaned,
                    line_number: cap.get(2).and_then(|m| m.as_str().parse().ok()),
                    column: None,
                    function_name: None,
                });
            }
            continue;
        }

        // Try C#/.NET
        if let Some(cap) = csharp_re.captures(line) {
            let file_path = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let cleaned = clean_file_path(file_path);
            if should_include_path(&cleaned) && !seen_paths.contains(&cleaned) {
                seen_paths.insert(cleaned.clone());
                frames.push(StackFrame {
                    file_path: cleaned,
                    line_number: cap.get(3).and_then(|m| m.as_str().parse().ok()),
                    column: None,
                    function_name: cap.get(1).map(|m| m.as_str().to_string()),
                });
            }
        }
    }

    frames
}

/// Clean up a file path extracted from stack trace
fn clean_file_path(path: &str) -> String {
    path.trim_start_matches('/')
        .trim_start_matches("./")
        .replace('\\', "/")
        .to_string()
}

/// Check if a path should be included (not a vendor/third-party path)
fn should_include_path(path: &str) -> bool {
    let excluded_patterns = [
        "node_modules",
        "vendor",
        "site-packages",
        ".pyenv",
        ".virtualenv",
        "venv/",
        "__pycache__",
        ".gem",
        "gems/",
        ".cargo/registry",
        ".rustup",
        "internal/",  // Go internal packages
        "<anonymous>",
        "<eval>",
        "native",
    ];

    !excluded_patterns.iter().any(|p| path.contains(p)) && !path.is_empty()
}

/// Extract file paths from a stack trace (legacy function for compatibility)
fn extract_files_from_stack(stack: &str) -> Vec<String> {
    parse_stack_trace(stack)
        .into_iter()
        .map(|f| f.file_path)
        .collect()
}

/// Extract search terms from an error message
fn extract_search_terms(message: &str) -> Vec<String> {
    let mut terms = Vec::new();

    // Look for identifiers (camelCase, PascalCase, snake_case)
    let identifier_re = Regex::new(r"\b([A-Z][a-z]+[A-Z][a-zA-Z]*|[a-z]+_[a-z_]+|[A-Z][a-zA-Z]+)\b").unwrap();

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

/// Find related file paths based on common naming patterns
/// e.g., handlers/user.ts -> models/user.ts, services/user.ts
fn find_related_file_paths(existing_paths: &HashSet<String>) -> Vec<String> {
    let mut related = Vec::new();

    // Common directory pairs that often contain related code
    let related_dirs = [
        ("handlers", vec!["models", "services", "repositories", "controllers"]),
        ("controllers", vec!["models", "services", "repositories", "handlers"]),
        ("routes", vec!["controllers", "handlers", "middleware"]),
        ("api", vec!["services", "models", "handlers"]),
        ("views", vec!["models", "controllers", "templates"]),
        ("services", vec!["models", "repositories", "handlers"]),
        ("repositories", vec!["models", "entities"]),
        ("models", vec!["schemas", "types", "interfaces"]),
        ("components", vec!["hooks", "utils", "types", "contexts"]),
        ("pages", vec!["components", "hooks", "api"]),
        ("src", vec!["lib", "utils"]),
    ];

    for path in existing_paths {
        for (dir, related_list) in &related_dirs {
            if path.contains(&format!("/{}/", dir)) || path.starts_with(&format!("{}/", dir)) {
                // Extract the filename
                if let Some(filename) = path.split('/').next_back() {
                    // Try to find related files
                    for related_dir in related_list {
                        let new_path = path.replace(&format!("/{}/", dir), &format!("/{}/", related_dir));
                        if new_path != *path && !existing_paths.contains(&new_path) {
                            related.push(new_path.clone());
                        }

                        // Also try replacing at the start
                        if path.starts_with(&format!("{}/", dir)) {
                            let new_path = path.replacen(&format!("{}/", dir), &format!("{}/", related_dir), 1);
                            if new_path != *path && !existing_paths.contains(&new_path) {
                                related.push(new_path);
                            }
                        }
                    }

                    // Also check for index files in the same directory
                    if !filename.starts_with("index.") {
                        let dir_path = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
                        let extensions = ["ts", "js", "tsx", "jsx", "py", "rb"];
                        for ext in extensions {
                            let index_path = format!("{}/index.{}", dir_path, ext);
                            if !existing_paths.contains(&index_path) {
                                related.push(index_path);
                            }
                        }
                    }
                }
            }
        }
    }

    // Deduplicate
    related.sort();
    related.dedup();
    related.truncate(10);

    related
}

/// Extract import paths from source files and resolve them to file paths
fn extract_imports_from_files(files: &[SourceFile], language: Option<&str>) -> Vec<String> {
    let mut imports = HashSet::new();

    for file in files {
        let file_imports = extract_imports_from_content(&file.content, &file.path, language);
        for import in file_imports {
            imports.insert(import);
        }
    }

    imports.into_iter().collect()
}

/// Extract import paths from file content
fn extract_imports_from_content(content: &str, file_path: &str, language: Option<&str>) -> Vec<String> {
    let mut imports = Vec::new();

    let lang = language.unwrap_or_else(|| {
        // Infer language from file extension
        if file_path.ends_with(".ts") || file_path.ends_with(".tsx") {
            "typescript"
        } else if file_path.ends_with(".js") || file_path.ends_with(".jsx") {
            "javascript"
        } else if file_path.ends_with(".py") {
            "python"
        } else if file_path.ends_with(".go") {
            "go"
        } else if file_path.ends_with(".rs") {
            "rust"
        } else if file_path.ends_with(".rb") {
            "ruby"
        } else {
            "unknown"
        }
    });

    let dir_path = file_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");

    match lang.to_lowercase().as_str() {
        "typescript" | "javascript" | "js" | "ts" => {
            // ES6 imports: import X from './path' or import { X } from './path'
            let import_re = Regex::new(r#"import\s+(?:[^'"]+\s+from\s+)?['"]([^'"]+)['"]"#).unwrap();
            // require: const X = require('./path')
            let require_re = Regex::new(r#"require\s*\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap();

            for cap in import_re.captures_iter(content) {
                if let Some(path) = cap.get(1) {
                    if let Some(resolved) = resolve_js_import(path.as_str(), dir_path) {
                        imports.push(resolved);
                    }
                }
            }

            for cap in require_re.captures_iter(content) {
                if let Some(path) = cap.get(1) {
                    if let Some(resolved) = resolve_js_import(path.as_str(), dir_path) {
                        imports.push(resolved);
                    }
                }
            }
        }
        "python" | "py" => {
            // from X import Y or import X
            let from_import_re = Regex::new(r"from\s+([^\s]+)\s+import").unwrap();
            let import_re = Regex::new(r"^import\s+([^\s,]+)").unwrap();

            for cap in from_import_re.captures_iter(content) {
                if let Some(module) = cap.get(1) {
                    if let Some(resolved) = resolve_python_import(module.as_str(), dir_path) {
                        imports.push(resolved);
                    }
                }
            }

            for cap in import_re.captures_iter(content) {
                if let Some(module) = cap.get(1) {
                    if let Some(resolved) = resolve_python_import(module.as_str(), dir_path) {
                        imports.push(resolved);
                    }
                }
            }
        }
        "go" | "golang" => {
            // import "path" or import ( "path" )
            let import_re = Regex::new(r#"import\s+(?:\(\s*)?["']([^"']+)["']"#).unwrap();

            for cap in import_re.captures_iter(content) {
                if let Some(path) = cap.get(1) {
                    // Only include local imports (not standard library)
                    let import_path = path.as_str();
                    if import_path.contains('/') && !import_path.starts_with("github.com/") {
                        imports.push(format!("{}.go", import_path));
                    }
                }
            }
        }
        "rust" | "rs" => {
            // use crate::path or mod path
            let use_re = Regex::new(r"use\s+crate::([^;{]+)").unwrap();
            let mod_re = Regex::new(r"mod\s+(\w+)\s*;").unwrap();

            for cap in use_re.captures_iter(content) {
                if let Some(path) = cap.get(1) {
                    let module_path = path.as_str().split("::").next().unwrap_or("");
                    imports.push(format!("src/{}.rs", module_path.replace("::", "/")));
                    imports.push(format!("src/{}/mod.rs", module_path.replace("::", "/")));
                }
            }

            for cap in mod_re.captures_iter(content) {
                if let Some(module) = cap.get(1) {
                    let module_name = module.as_str();
                    imports.push(format!("{}/{}.rs", dir_path, module_name));
                    imports.push(format!("{}/{}/mod.rs", dir_path, module_name));
                }
            }
        }
        _ => {}
    }

    imports
}

/// Resolve a JavaScript/TypeScript import path to a file path
fn resolve_js_import(import_path: &str, current_dir: &str) -> Option<String> {
    // Skip external packages
    if !import_path.starts_with('.') && !import_path.starts_with('/') {
        return None;
    }

    let extensions = ["ts", "tsx", "js", "jsx"];

    // Resolve relative path
    let resolved = if import_path.starts_with('.') {
        let parts: Vec<&str> = import_path.split('/').collect();
        let mut current: Vec<&str> = current_dir.split('/').collect();

        for part in parts {
            match part {
                "." => {}
                ".." => {
                    current.pop();
                }
                _ => {
                    current.push(part);
                }
            }
        }
        current.join("/")
    } else {
        import_path.trim_start_matches('/').to_string()
    };

    // Check if path already has extension
    if extensions.iter().any(|ext| resolved.ends_with(&format!(".{}", ext))) {
        return Some(resolved);
    }

    // Try adding extensions
    if let Some(ext) = extensions.into_iter().next() {
        let with_ext = format!("{}.{}", resolved, ext);
        return Some(with_ext);
    }

    None
}

/// Resolve a Python import path to a file path
fn resolve_python_import(import_path: &str, current_dir: &str) -> Option<String> {
    // Skip standard library modules
    let stdlib = ["os", "sys", "json", "re", "datetime", "collections", "typing", "abc", "functools"];
    let first_part = import_path.split('.').next().unwrap_or("");
    if stdlib.contains(&first_part) {
        return None;
    }

    // Convert module path to file path
    let file_path = import_path.replace('.', "/");

    // For relative imports
    if import_path.starts_with('.') {
        let dots = import_path.chars().take_while(|c| *c == '.').count();
        let remainder = &import_path[dots..];
        let mut parts: Vec<String> = current_dir.split('/').map(String::from).collect();

        for _ in 0..dots {
            parts.pop();
        }

        if !remainder.is_empty() {
            parts.push(remainder.replace('.', "/"));
        }

        let base = parts.join("/");
        Some(format!("{}.py", base))
    } else {
        Some(format!("{}.py", file_path))
    }
}

/// Extract surrounding context around error lines
fn extract_surrounding_context(
    content: &str,
    error_lines: &[usize],
    context_lines: usize,
    max_size: usize,
) -> (String, bool) {
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if content.len() <= max_size {
        // If whole file fits, return it with line markers
        return (content.to_string(), false);
    }

    // Merge overlapping ranges
    let mut ranges: Vec<(usize, usize)> = error_lines
        .iter()
        .map(|&line| {
            let start = line.saturating_sub(context_lines).max(1);
            let end = (line + context_lines).min(total_lines);
            (start, end)
        })
        .collect();

    ranges.sort_by_key(|r| r.0);

    // Merge overlapping ranges
    let mut merged_ranges: Vec<(usize, usize)> = vec![];
    for range in ranges {
        if let Some(last) = merged_ranges.last_mut() {
            if range.0 <= last.1 + 1 {
                last.1 = last.1.max(range.1);
            } else {
                merged_ranges.push(range);
            }
        } else {
            merged_ranges.push(range);
        }
    }

    // Build output with context markers
    let mut output = String::new();
    let mut truncated = false;

    for (i, (start, end)) in merged_ranges.iter().enumerate() {
        if output.len() >= max_size {
            truncated = true;
            break;
        }

        if i > 0 {
            output.push_str("\n... [lines omitted] ...\n\n");
        } else if *start > 1 {
            output.push_str(&format!("... [lines 1-{} omitted] ...\n\n", start - 1));
        }

        // Add line numbers for context
        for line_num in *start..=*end {
            if line_num > 0 && line_num <= total_lines {
                let line = lines[line_num - 1];
                let marker = if error_lines.contains(&line_num) { ">>> " } else { "    " };
                let formatted = format!("{}{:4} | {}\n", marker, line_num, line);

                if output.len() + formatted.len() > max_size {
                    truncated = true;
                    break;
                }
                output.push_str(&formatted);
            }
        }
    }

    if let Some((_, last_end)) = merged_ranges.last() {
        if *last_end < total_lines {
            output.push_str(&format!("\n... [lines {}-{} omitted] ...\n", last_end + 1, total_lines));
        }
    }

    (output, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stack_trace_javascript() {
        let stack = r#"
TypeError: Cannot read property 'userId' of undefined
    at UserController.getProfile (src/controllers/user.ts:45:23)
    at Router.handle (/app/node_modules/express/lib/router/index.js:174:12)
    at processTicksAndRejections (internal/process/task_queues.js:93:5)
"#;

        let frames = parse_stack_trace(stack);
        assert_eq!(frames.len(), 1); // Only one non-vendor file
        assert_eq!(frames[0].file_path, "src/controllers/user.ts");
        assert_eq!(frames[0].line_number, Some(45));
        assert_eq!(frames[0].column, Some(23));
        assert_eq!(frames[0].function_name, Some("UserController.getProfile".to_string()));
    }

    #[test]
    fn test_parse_stack_trace_python() {
        let stack = r#"
Traceback (most recent call last):
  File "/app/handlers/user.py", line 42, in get_user
    return user_service.fetch(user_id)
  File "/app/services/user_service.py", line 15, in fetch
    return self.repo.find_by_id(user_id)
  File "/usr/lib/python3.8/site-packages/sqlalchemy/orm/query.py", line 123, in one
    raise NoResultFound
"#;

        let frames = parse_stack_trace(stack);
        assert_eq!(frames.len(), 2); // Excludes site-packages
        assert_eq!(frames[0].file_path, "app/handlers/user.py");
        assert_eq!(frames[0].line_number, Some(42));
        assert_eq!(frames[0].function_name, Some("get_user".to_string()));
        assert_eq!(frames[1].file_path, "app/services/user_service.py");
        assert_eq!(frames[1].line_number, Some(15));
    }

    #[test]
    fn test_parse_stack_trace_java() {
        let stack = r#"
java.lang.NullPointerException
    at com.example.service.UserService.getUser(UserService.java:42)
    at com.example.controller.UserController.handle(UserController.java:28)
    at org.springframework.web.servlet.FrameworkServlet.service(FrameworkServlet.java:897)
"#;

        let frames = parse_stack_trace(stack);
        assert!(frames.len() >= 2);
        assert!(frames.iter().any(|f| f.file_path == "UserService.java" && f.line_number == Some(42)));
        assert!(frames.iter().any(|f| f.file_path == "UserController.java" && f.line_number == Some(28)));
    }

    #[test]
    fn test_parse_stack_trace_go() {
        let stack = r#"
goroutine 1 [running]:
main.handleRequest(0xc0000b4000, 0x1)
    /app/handlers/user.go:45 +0x1a5
main.main()
    /app/main.go:23 +0x2e
"#;

        let frames = parse_stack_trace(stack);
        assert_eq!(frames.len(), 2);
        assert!(frames.iter().any(|f| f.file_path == "app/handlers/user.go" && f.line_number == Some(45)));
        assert!(frames.iter().any(|f| f.file_path == "app/main.go" && f.line_number == Some(23)));
    }

    #[test]
    fn test_parse_stack_trace_rust() {
        let stack = r#"
thread 'main' panicked at src/handlers/user.rs:42:5:
called `Option::unwrap()` on a `None` value
stack backtrace:
   0: src/main.rs:15:10
"#;

        let frames = parse_stack_trace(stack);
        assert!(frames.iter().any(|f| f.file_path == "src/handlers/user.rs" && f.line_number == Some(42)));
        assert!(frames.iter().any(|f| f.file_path == "src/main.rs" && f.line_number == Some(15)));
    }

    #[test]
    fn test_extract_files_from_stack_legacy() {
        let stack = r#"
TypeError: Cannot read property 'userId' of undefined
    at UserController.getProfile (src/controllers/user.ts:45:23)
    at Router.handle (/app/node_modules/express/lib/router/index.js:174:12)
"#;

        let files = extract_files_from_stack(stack);
        assert!(files.contains(&"src/controllers/user.ts".to_string()));
        assert!(!files.iter().any(|f| f.contains("node_modules")));
    }

    #[test]
    fn test_extract_search_terms() {
        let message = "NullPointerException in UserService.getUser when accessing user_profile";
        let terms = extract_search_terms(message);
        assert!(terms.contains(&"UserService".to_string()));
        // user_profile matches snake_case pattern
        assert!(terms.contains(&"user_profile".to_string()));
    }

    #[test]
    fn test_find_related_file_paths() {
        let mut paths = HashSet::new();
        paths.insert("handlers/user.ts".to_string());
        paths.insert("controllers/auth.ts".to_string());

        let related = find_related_file_paths(&paths);

        // Should find related files in different directories
        // handlers/user.ts -> models/user.ts, services/user.ts
        assert!(related.iter().any(|p| p.contains("models") || p.contains("services") || p.contains("index")));
    }

    #[test]
    fn test_extract_imports_js() {
        let content = r#"
import { User } from './models/user';
import * as utils from '../utils/helpers';
import axios from 'axios';
const config = require('./config');
"#;

        let imports = extract_imports_from_content(content, "src/handlers/user.ts", Some("typescript"));

        // Should include relative imports, not external packages
        assert!(imports.iter().any(|p| p.contains("models/user")));
        assert!(imports.iter().any(|p| p.contains("utils/helpers")));
        assert!(imports.iter().any(|p| p.contains("config")));
        assert!(!imports.iter().any(|p| p.contains("axios")));
    }

    #[test]
    fn test_extract_imports_python() {
        let content = r#"
from .models import User
from ..services.user_service import UserService
import os
import json
from myapp.utils import helpers
"#;

        let imports = extract_imports_from_content(content, "src/handlers/user.py", Some("python"));

        // Should include local imports, not stdlib
        assert!(imports.iter().any(|p| p.contains("models") || p.contains("services") || p.contains("utils")));
        assert!(!imports.iter().any(|p| p == "os.py" || p == "json.py"));
    }

    #[test]
    fn test_extract_surrounding_context() {
        let content = (1..=100).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let error_lines = vec![50];

        let (result, truncated) = extract_surrounding_context(&content, &error_lines, 5, 10000);

        // Should include context around line 50
        assert!(result.contains("line 45") || result.contains("45 |"));
        assert!(result.contains("line 50") || result.contains("50 |"));
        assert!(result.contains("line 55") || result.contains("55 |"));
        assert!(!truncated);
    }

    #[test]
    fn test_extract_surrounding_context_multiple_errors() {
        // Create content large enough that it won't fit entirely
        let content = (1..=200).map(|i| format!("line {} with some additional text to make it longer", i)).collect::<Vec<_>>().join("\n");
        let error_lines = vec![20, 180];

        // Use a smaller max_size to force truncation and omission markers
        let (result, _) = extract_surrounding_context(&content, &error_lines, 5, 2000);

        // Should include at least the first error location
        assert!(result.contains("20 |") || result.contains("line 20"));
        // When content is larger than max_size and we have gaps between error regions,
        // we should see omission markers
        assert!(result.contains("omitted") || result.contains("..."));
    }

    #[test]
    fn test_extract_surrounding_context_with_limit() {
        let content = (1..=1000).map(|i| format!("line {} with some extra content to make it longer", i)).collect::<Vec<_>>().join("\n");
        let error_lines = vec![500];

        let (result, truncated) = extract_surrounding_context(&content, &error_lines, 50, 1000);

        // Should respect size limit
        assert!(result.len() <= 1100); // Allow some buffer for markers
        assert!(truncated);
    }

    #[test]
    fn test_context_config_default() {
        let config = ContextConfig::default();
        assert_eq!(config.max_context_size, DEFAULT_MAX_CONTEXT_SIZE);
        assert_eq!(config.context_lines, DEFAULT_CONTEXT_LINES);
        assert!(config.follow_imports);
        assert!(config.find_related_files);
        assert!(config.include_crispr_context);
    }

    #[test]
    fn test_clean_file_path() {
        assert_eq!(clean_file_path("/app/src/file.ts"), "app/src/file.ts");
        assert_eq!(clean_file_path("./src/file.ts"), "src/file.ts");
        assert_eq!(clean_file_path("src\\file.ts"), "src/file.ts");
    }

    #[test]
    fn test_should_include_path() {
        assert!(should_include_path("src/handlers/user.ts"));
        assert!(!should_include_path("node_modules/express/index.js"));
        assert!(!should_include_path("vendor/autoload.php"));
        assert!(!should_include_path(".pyenv/versions/3.8/lib/os.py"));
        assert!(!should_include_path("<anonymous>"));
    }
}
