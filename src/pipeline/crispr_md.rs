//! CRISPR.md file management
//!
//! Maintains a CRISPR.md file in each monitored repository containing:
//! 1. Special repository context (manually edited by developers)
//! 2. Last 10 fixes made by CRISPR
//! 3. Service description (extracted by LLM or manually edited)

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

use super::repo::RepoManager;

pub const CRISPR_MD_PATH: &str = "CRISPR.md";

/// The structure of information tracked in CRISPR.md
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrisprContext {
    /// Special things about this repo that CRISPR needs to know
    pub special_notes: String,
    /// What this service does conceptually
    pub service_description: String,
    /// Last 10 fixes made by CRISPR (most recent first)
    pub recent_fixes: Vec<FixRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixRecord {
    pub timestamp: DateTime<Utc>,
    pub incident_id: String,
    pub error_summary: String,
    pub fix_summary: String,
    pub pr_url: Option<String>,
    pub pr_merged: bool,
}

/// Template for new CRISPR.md files
const TEMPLATE: &str = r#"# CRISPR Context

This file is maintained by [CRISPR](https://github.com/your-org/crispr) to help with automated error fixing.
Feel free to edit sections 1 and 3 to provide better context for the LLM.

---

## 1. Special Notes

<!-- What special things does this repo have that CRISPR needs to know about?
     Examples: unusual build process, specific coding conventions, dependencies
     that shouldn't be modified, areas that need human review, etc. -->



---

## 2. Recent Fixes

<!-- This section is automatically updated by CRISPR. Do not edit manually. -->

_No fixes recorded yet._

---

## 3. Service Description

<!-- What does this service do, conceptually?
     This helps CRISPR understand the purpose of the code when generating fixes. -->


"#;

pub struct CrisprMdManager<'a> {
    repo_manager: &'a RepoManager,
}

impl<'a> CrisprMdManager<'a> {
    pub fn new(repo_manager: &'a RepoManager) -> Self {
        Self { repo_manager }
    }

    /// Read the CRISPR.md context from a repository
    /// Creates the file from template if it doesn't exist
    pub fn read_context(&self, owner: &str, repo: &str) -> Result<CrisprContext> {
        let content = match self.repo_manager.read_file_optional(owner, repo, CRISPR_MD_PATH)? {
            Some(content) => content,
            None => {
                // Create from template
                info!(owner, repo, "Creating CRISPR.md from template");
                self.repo_manager.write_file(owner, repo, CRISPR_MD_PATH, TEMPLATE)?;
                TEMPLATE.to_string()
            }
        };

        self.parse_markdown(&content)
    }

    /// Parse CRISPR.md content into structured context
    fn parse_markdown(&self, content: &str) -> Result<CrisprContext> {
        let mut ctx = CrisprContext::default();

        // Extract section 1: Special Notes
        if let Some(special_start) = content.find("## 1. Special Notes") {
            if let Some(special_end) = content[special_start..].find("## 2.") {
                let section = &content[special_start..special_start + special_end];
                ctx.special_notes = self.extract_section_content(section);
            }
        }

        // Extract section 3: Service Description
        if let Some(desc_start) = content.find("## 3. Service Description") {
            let section = &content[desc_start..];
            ctx.service_description = self.extract_section_content(section);
        }

        // Extract section 2: Recent Fixes (parse the list)
        if let Some(fixes_start) = content.find("## 2. Recent Fixes") {
            if let Some(fixes_end) = content[fixes_start..].find("## 3.") {
                let section = &content[fixes_start..fixes_start + fixes_end];
                ctx.recent_fixes = self.parse_fixes_section(section);
            }
        }

        Ok(ctx)
    }

    /// Extract content from a section, removing comments and headers
    fn extract_section_content(&self, section: &str) -> String {
        let mut lines: Vec<&str> = section.lines().collect();

        // Remove the header line
        if !lines.is_empty() && lines[0].starts_with("##") {
            lines.remove(0);
        }

        // Remove HTML comments
        let content = lines.join("\n");
        let mut result = String::new();
        let mut in_comment = false;

        for line in content.lines() {
            if line.contains("<!--") {
                in_comment = true;
            }
            if !in_comment {
                result.push_str(line);
                result.push('\n');
            }
            if line.contains("-->") {
                in_comment = false;
            }
        }

        result.trim().to_string()
    }

    /// Parse the fixes section into structured records
    fn parse_fixes_section(&self, section: &str) -> Vec<FixRecord> {
        let mut fixes = vec![];

        // Look for fix entries in the format:
        // ### [timestamp] error_summary
        // Fix: fix_summary
        // PR: pr_url (merged/open)

        let mut current_fix: Option<FixRecord> = None;

        for line in section.lines() {
            if line.starts_with("### ") {
                // Save previous fix if exists
                if let Some(fix) = current_fix.take() {
                    fixes.push(fix);
                }

                // Parse new fix header: ### [2024-01-15T10:30:00Z] Error summary
                if let Some(bracket_start) = line.find('[') {
                    if let Some(bracket_end) = line.find(']') {
                        let timestamp_str = &line[bracket_start + 1..bracket_end];
                        let error_summary = line[bracket_end + 1..].trim().to_string();

                        if let Ok(timestamp) = DateTime::parse_from_rfc3339(timestamp_str) {
                            current_fix = Some(FixRecord {
                                timestamp: timestamp.with_timezone(&Utc),
                                incident_id: String::new(),
                                error_summary,
                                fix_summary: String::new(),
                                pr_url: None,
                                pr_merged: false,
                            });
                        }
                    }
                }
            } else if let Some(ref mut fix) = current_fix {
                if line.starts_with("**Fix:**") || line.starts_with("Fix:") {
                    fix.fix_summary = line
                        .trim_start_matches("**Fix:**")
                        .trim_start_matches("Fix:")
                        .trim()
                        .to_string();
                } else if line.starts_with("**PR:**") || line.starts_with("PR:") {
                    let pr_part = line
                        .trim_start_matches("**PR:**")
                        .trim_start_matches("PR:")
                        .trim();

                    // Extract URL from markdown link [text](url) or plain URL
                    if let Some(url_start) = pr_part.find('(') {
                        if let Some(url_end) = pr_part.find(')') {
                            fix.pr_url = Some(pr_part[url_start + 1..url_end].to_string());
                        }
                    } else if pr_part.starts_with("http") {
                        let url = pr_part.split_whitespace().next().unwrap_or("");
                        fix.pr_url = Some(url.to_string());
                    }

                    fix.pr_merged = pr_part.contains("merged") || pr_part.contains("Merged");
                } else if line.starts_with("**Incident:**") || line.starts_with("Incident:") {
                    fix.incident_id = line
                        .trim_start_matches("**Incident:**")
                        .trim_start_matches("Incident:")
                        .trim()
                        .to_string();
                }
            }
        }

        // Don't forget the last fix
        if let Some(fix) = current_fix {
            fixes.push(fix);
        }

        fixes
    }

    /// Add a new fix record to the CRISPR.md file
    pub fn record_fix(
        &self,
        owner: &str,
        repo: &str,
        incident_id: &str,
        error_summary: &str,
        fix_summary: &str,
        pr_url: Option<&str>,
    ) -> Result<()> {
        let mut ctx = self.read_context(owner, repo)?;

        // Add new fix at the beginning
        ctx.recent_fixes.insert(
            0,
            FixRecord {
                timestamp: Utc::now(),
                incident_id: incident_id.to_string(),
                error_summary: error_summary.to_string(),
                fix_summary: fix_summary.to_string(),
                pr_url: pr_url.map(String::from),
                pr_merged: false,
            },
        );

        // Keep only last 10 fixes
        ctx.recent_fixes.truncate(10);

        // Write back
        self.write_context(owner, repo, &ctx)
    }

    /// Update PR merge status for a fix
    pub fn mark_pr_merged(&self, owner: &str, repo: &str, pr_url: &str) -> Result<()> {
        let mut ctx = self.read_context(owner, repo)?;

        for fix in &mut ctx.recent_fixes {
            if fix.pr_url.as_deref() == Some(pr_url) {
                fix.pr_merged = true;
            }
        }

        self.write_context(owner, repo, &ctx)
    }

    /// Write the context back to CRISPR.md
    fn write_context(&self, owner: &str, repo: &str, ctx: &CrisprContext) -> Result<()> {
        let content = self.render_markdown(ctx);
        self.repo_manager
            .write_file(owner, repo, CRISPR_MD_PATH, &content)
            .with_context(|| format!("Failed to write CRISPR.md for {}/{}", owner, repo))
    }

    /// Render context to markdown format
    fn render_markdown(&self, ctx: &CrisprContext) -> String {
        let mut md = String::new();

        md.push_str("# CRISPR Context\n\n");
        md.push_str("This file is maintained by [CRISPR](https://github.com/your-org/crispr) to help with automated error fixing.\n");
        md.push_str("Feel free to edit sections 1 and 3 to provide better context for the LLM.\n\n");
        md.push_str("---\n\n");

        // Section 1: Special Notes
        md.push_str("## 1. Special Notes\n\n");
        md.push_str("<!-- What special things does this repo have that CRISPR needs to know about?\n");
        md.push_str("     Examples: unusual build process, specific coding conventions, dependencies\n");
        md.push_str("     that shouldn't be modified, areas that need human review, etc. -->\n\n");
        if ctx.special_notes.is_empty() {
            md.push_str("\n");
        } else {
            md.push_str(&ctx.special_notes);
            md.push_str("\n");
        }
        md.push_str("\n---\n\n");

        // Section 2: Recent Fixes
        md.push_str("## 2. Recent Fixes\n\n");
        md.push_str("<!-- This section is automatically updated by CRISPR. Do not edit manually. -->\n\n");

        if ctx.recent_fixes.is_empty() {
            md.push_str("_No fixes recorded yet._\n");
        } else {
            for fix in &ctx.recent_fixes {
                md.push_str(&format!(
                    "### [{}] {}\n",
                    fix.timestamp.to_rfc3339(),
                    fix.error_summary
                ));
                md.push_str(&format!("**Incident:** {}\n", fix.incident_id));
                md.push_str(&format!("**Fix:** {}\n", fix.fix_summary));
                if let Some(url) = &fix.pr_url {
                    let status = if fix.pr_merged { "Merged" } else { "Open" };
                    md.push_str(&format!("**PR:** [{}]({}) ({})\n", url, url, status));
                }
                md.push('\n');
            }
        }
        md.push_str("\n---\n\n");

        // Section 3: Service Description
        md.push_str("## 3. Service Description\n\n");
        md.push_str("<!-- What does this service do, conceptually?\n");
        md.push_str("     This helps CRISPR understand the purpose of the code when generating fixes. -->\n\n");
        if ctx.service_description.is_empty() {
            md.push_str("\n");
        } else {
            md.push_str(&ctx.service_description);
            md.push_str("\n");
        }

        md
    }

    /// Get the context as a formatted string for LLM prompts
    pub fn get_context_for_prompt(&self, owner: &str, repo: &str) -> Result<String> {
        let ctx = self.read_context(owner, repo)?;

        let mut prompt = String::new();

        prompt.push_str("=== CRISPR CONTEXT FOR THIS REPOSITORY ===\n\n");

        if !ctx.special_notes.is_empty() {
            prompt.push_str("## Special Notes from Developers:\n");
            prompt.push_str(&ctx.special_notes);
            prompt.push_str("\n\n");
        }

        if !ctx.service_description.is_empty() {
            prompt.push_str("## Service Description:\n");
            prompt.push_str(&ctx.service_description);
            prompt.push_str("\n\n");
        }

        if !ctx.recent_fixes.is_empty() {
            prompt.push_str("## Recent Fixes by CRISPR (for pattern reference):\n");
            for (i, fix) in ctx.recent_fixes.iter().take(5).enumerate() {
                prompt.push_str(&format!(
                    "{}. {} -> {}{}\n",
                    i + 1,
                    fix.error_summary,
                    fix.fix_summary,
                    if fix.pr_merged { " (merged)" } else { "" }
                ));
            }
            prompt.push('\n');
        }

        prompt.push_str("===========================================\n");

        Ok(prompt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fixes_section() {
        let section = r#"## 2. Recent Fixes

### [2024-01-15T10:30:00Z] TypeError: Cannot read property 'foo' of undefined
**Incident:** abc123
**Fix:** Added null check before accessing property
**PR:** [https://github.com/org/repo/pull/42](https://github.com/org/repo/pull/42) (Merged)

### [2024-01-14T08:00:00Z] ReferenceError: bar is not defined
**Incident:** def456
**Fix:** Imported missing module
**PR:** [https://github.com/org/repo/pull/41](https://github.com/org/repo/pull/41) (Open)
"#;

        let manager = CrisprMdManager {
            repo_manager: unsafe { std::mem::zeroed() }, // Don't actually use in test
        };

        let fixes = manager.parse_fixes_section(section);

        assert_eq!(fixes.len(), 2);
        assert_eq!(
            fixes[0].error_summary,
            "TypeError: Cannot read property 'foo' of undefined"
        );
        assert_eq!(fixes[0].fix_summary, "Added null check before accessing property");
        assert!(fixes[0].pr_merged);
        assert!(!fixes[1].pr_merged);
    }
}
