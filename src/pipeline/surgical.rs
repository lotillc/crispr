//! Surgical Coordination Module
//!
//! Provides per-repo serialization, pre-surgery diff checks, and fresh pull
//! operations to prevent conflicts when fixing errors in the same repository.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use git2::Repository;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::repo::RepoManager;
use crate::integrations::llm::TrackedLlm;
use crate::store::models::Project;

/// Duration for surgical lock (30 minutes by default)
const LOCK_DURATION_MINUTES: i64 = 30;

/// Result of attempting to acquire a surgical lock
#[derive(Debug)]
pub enum LockResult {
    /// Lock was successfully acquired
    Acquired(RepoSurgicalLock),
    /// Another worker holds the lock
    Blocked {
        holder_incident_id: Uuid,
        queue_position: i32,
    },
    /// Lock was held by a dead worker, retry after cleanup
    RetryAfterCleanup,
}

/// Represents an active surgical lock on a repository
#[derive(Debug, Clone)]
pub struct RepoSurgicalLock {
    pub id: Uuid,
    pub project_id: Uuid,
    pub incident_id: Uuid,
    pub worker_id: String,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Result of checking recent diffs before surgery
#[derive(Debug)]
pub enum DiffCheckResult {
    /// Safe to proceed with fix
    SafeToProceed,
    /// Recent commits may have already fixed this - wait for verification
    PendingVerification {
        reason: String,
        relevant_commits: Vec<String>,
    },
    /// An overlapping fix PR exists
    OverlappingFix {
        related_incident_id: Uuid,
        pr_url: String,
    },
}

/// Information about a recent commit that might be related
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelevantCommit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: DateTime<Utc>,
    pub files_changed: Vec<String>,
}

/// Attempt to acquire a surgical lock for a repository
///
/// Only one incident can be actively fixed per repository at a time.
/// Returns Blocked if another worker has the lock, with queue position info.
pub async fn acquire_lock(
    db: &PgPool,
    project_id: Uuid,
    incident_id: Uuid,
    worker_id: &str,
) -> Result<LockResult> {
    let mut tx = db.begin().await?;

    // Check for existing lock (with row lock)
    let existing = sqlx::query!(
        r#"
        SELECT id, incident_id, worker_id, acquired_at, expires_at
        FROM repo_surgical_locks
        WHERE project_id = $1
        FOR UPDATE
        "#,
        project_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(lock) = existing {
        // Check if lock has expired
        let expires_at: DateTime<Utc> = lock.expires_at;
        if expires_at < Utc::now() {
            // Lock is stale - delete it and let caller retry
            sqlx::query!(
                "DELETE FROM repo_surgical_locks WHERE id = $1",
                lock.id
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;

            info!(
                project_id = %project_id,
                stale_lock_id = %lock.id,
                "Cleaned up expired surgical lock"
            );
            return Ok(LockResult::RetryAfterCleanup);
        }

        // Lock is held by another worker
        // Calculate queue position
        let queue_position = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM incidents
            WHERE project_id = $1
            AND state = 'detected'
            AND queued_at IS NOT NULL
            AND queued_at < (
                SELECT COALESCE(queued_at, NOW())
                FROM incidents WHERE id = $2
            )
            "#,
            project_id,
            incident_id
        )
        .fetch_one(&mut *tx)
        .await? as i32 + 1;

        // Update queue position for this incident
        sqlx::query!(
            r#"
            UPDATE incidents
            SET queued_at = COALESCE(queued_at, NOW()),
                queue_position = $1
            WHERE id = $2
            "#,
            queue_position,
            incident_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        debug!(
            incident_id = %incident_id,
            holder_incident_id = %lock.incident_id,
            queue_position = queue_position,
            "Incident queued, repo is locked"
        );

        return Ok(LockResult::Blocked {
            holder_incident_id: lock.incident_id,
            queue_position,
        });
    }

    // No existing lock - acquire one
    let lock_id = Uuid::new_v4();
    let now = Utc::now();
    let expires_at = now + Duration::minutes(LOCK_DURATION_MINUTES);

    sqlx::query!(
        r#"
        INSERT INTO repo_surgical_locks (id, project_id, incident_id, worker_id, acquired_at, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        lock_id,
        project_id,
        incident_id,
        worker_id,
        now,
        expires_at
    )
    .execute(&mut *tx)
    .await?;

    // Clear queue info since we got the lock
    sqlx::query!(
        "UPDATE incidents SET queued_at = NULL, queue_position = NULL WHERE id = $1",
        incident_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    info!(
        lock_id = %lock_id,
        project_id = %project_id,
        incident_id = %incident_id,
        worker_id = worker_id,
        expires_at = %expires_at,
        "Acquired surgical lock"
    );

    Ok(LockResult::Acquired(RepoSurgicalLock {
        id: lock_id,
        project_id,
        incident_id,
        worker_id: worker_id.to_string(),
        acquired_at: now,
        expires_at,
    }))
}

/// Release a surgical lock
pub async fn release_lock(db: &PgPool, lock_id: Uuid) -> Result<()> {
    let result = sqlx::query!(
        "DELETE FROM repo_surgical_locks WHERE id = $1",
        lock_id
    )
    .execute(db)
    .await?;

    if result.rows_affected() > 0 {
        info!(lock_id = %lock_id, "Released surgical lock");
    } else {
        warn!(lock_id = %lock_id, "Surgical lock not found during release");
    }

    Ok(())
}

/// Extend a surgical lock's expiration time
pub async fn extend_lock(db: &PgPool, lock_id: Uuid) -> Result<()> {
    let new_expires = Utc::now() + Duration::minutes(LOCK_DURATION_MINUTES);

    sqlx::query!(
        "UPDATE repo_surgical_locks SET expires_at = $1 WHERE id = $2",
        new_expires,
        lock_id
    )
    .execute(db)
    .await?;

    debug!(lock_id = %lock_id, new_expires = %new_expires, "Extended surgical lock");
    Ok(())
}

/// Clean up all expired surgical locks
pub async fn cleanup_expired_locks(db: &PgPool) -> Result<u64> {
    let result = sqlx::query!(
        "DELETE FROM repo_surgical_locks WHERE expires_at < NOW()"
    )
    .execute(db)
    .await?;

    let count = result.rows_affected();
    if count > 0 {
        info!(count = count, "Cleaned up expired surgical locks");
    }

    Ok(count)
}

/// Check recent diffs to see if the error might already be fixed
///
/// This prevents redundant fix attempts when:
/// 1. A human developer recently committed a fix
/// 2. Another incident's PR was recently merged for the same files
pub async fn check_recent_diffs(
    db: &PgPool,
    llm: &TrackedLlm,
    repo_manager: &RepoManager,
    project: &Project,
    incident_id: Uuid,
    error_title: &str,
    suggested_files: &[String],
) -> Result<DiffCheckResult> {
    // Check for overlapping open PRs first
    let overlapping = sqlx::query!(
        r#"
        SELECT i.id, i.pr_url, i.pr_state, i.title
        FROM incidents i
        WHERE i.project_id = $1
        AND i.id != $2
        AND i.pr_state = 'open'
        AND i.pr_url IS NOT NULL
        "#,
        project.id,
        incident_id
    )
    .fetch_all(db)
    .await?;

    // Check if any open PR might be fixing the same issue
    for pr in overlapping {
        // Query the PR diff to see if it touches our files
        // For now, we'll mark as overlapping if any PR is open for this project
        // A more sophisticated approach would analyze the actual files changed
        if pr.pr_url.is_some() {
            debug!(
                incident_id = %incident_id,
                related_incident_id = %pr.id,
                "Found open PR for same project"
            );

            // Record the relation
            let _ = sqlx::query!(
                r#"
                INSERT INTO incident_relations (id, incident_id, related_incident_id, relation_type, confidence)
                VALUES ($1, $2, $3, 'overlapping_fix', 0.5)
                ON CONFLICT (incident_id, related_incident_id, relation_type) DO NOTHING
                "#,
                Uuid::new_v4(),
                incident_id,
                pr.id
            )
            .execute(db)
            .await;
        }
    }

    // Get recent commits that might have fixed the issue
    let recent_commits = get_recent_commits(
        repo_manager,
        &project.github_owner,
        &project.github_repo,
        24, // Look back 24 hours
    )?;

    if recent_commits.is_empty() {
        return Ok(DiffCheckResult::SafeToProceed);
    }

    // Filter commits that touched suggested files
    let relevant_commits: Vec<_> = recent_commits
        .into_iter()
        .filter(|commit| {
            suggested_files.iter().any(|suggested| {
                commit.files_changed.iter().any(|changed| {
                    changed.contains(suggested) || suggested.contains(changed)
                })
            })
        })
        .collect();

    if relevant_commits.is_empty() {
        return Ok(DiffCheckResult::SafeToProceed);
    }

    // Ask LLM if any of these commits might have fixed the error
    let might_be_fixed = analyze_commits_for_fix(
        llm,
        error_title,
        &relevant_commits,
        incident_id,
        project.id,
    )
    .await?;

    if might_be_fixed {
        let commit_shas: Vec<String> = relevant_commits.iter().map(|c| c.sha.clone()).collect();
        info!(
            incident_id = %incident_id,
            commits = ?commit_shas,
            "Recent commits may have fixed this issue"
        );

        return Ok(DiffCheckResult::PendingVerification {
            reason: "Recent commits may have already fixed this issue".to_string(),
            relevant_commits: commit_shas,
        });
    }

    Ok(DiffCheckResult::SafeToProceed)
}

/// Get recent commits from the repository
fn get_recent_commits(
    repo_manager: &RepoManager,
    owner: &str,
    repo: &str,
    hours_back: i64,
) -> Result<Vec<RelevantCommit>> {
    let repo_path = repo_manager.repo_path(owner, repo);
    if !repo_path.exists() {
        return Ok(vec![]);
    }

    let git_repo = Repository::open(&repo_path)
        .context("Failed to open repository")?;

    let mut revwalk = git_repo.revwalk()?;
    revwalk.push_head()?;

    let cutoff = Utc::now() - Duration::hours(hours_back);
    let mut commits = Vec::new();

    for oid in revwalk.take(100) {
        // Limit to 100 commits
        let oid = oid?;
        let commit = git_repo.find_commit(oid)?;

        let commit_time = DateTime::from_timestamp(commit.time().seconds(), 0)
            .unwrap_or_else(Utc::now);

        if commit_time < cutoff {
            break;
        }

        // Get files changed in this commit
        let files_changed = get_commit_files(&git_repo, &commit)?;

        commits.push(RelevantCommit {
            sha: oid.to_string(),
            message: commit.message().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("unknown").to_string(),
            date: commit_time,
            files_changed,
        });
    }

    Ok(commits)
}

/// Get the list of files changed in a commit
fn get_commit_files(repo: &Repository, commit: &git2::Commit) -> Result<Vec<String>> {
    let tree = commit.tree()?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

    let mut files = Vec::new();
    diff.foreach(
        &mut |delta, _| {
            if let Some(path) = delta.new_file().path() {
                files.push(path.to_string_lossy().to_string());
            }
            true
        },
        None,
        None,
        None,
    )?;

    Ok(files)
}

/// Ask LLM if recent commits might have fixed the error
async fn analyze_commits_for_fix(
    llm: &TrackedLlm,
    error_title: &str,
    commits: &[RelevantCommit],
    incident_id: Uuid,
    project_id: Uuid,
) -> Result<bool> {
    if commits.is_empty() {
        return Ok(false);
    }

    let commits_summary: String = commits
        .iter()
        .take(5)
        .map(|c| format!("- {} ({}): {}", &c.sha[..8], c.author, c.message.lines().next().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        r#"Analyze if any of these recent commits might have fixed the following error.

ERROR: {}

RECENT COMMITS:
{}

Respond with ONLY "yes" or "no".
- Answer "yes" if any commit message or the files changed suggest it might address this error
- Answer "no" if the commits appear unrelated to this error

Response:"#,
        error_title,
        commits_summary
    );

    let response = llm
        .complete(&prompt, None, "diff_check", Some(incident_id), Some(project_id))
        .await?;

    let answer = response.content.trim().to_lowercase();
    Ok(answer.contains("yes"))
}

/// Perform a fresh pull before surgery to ensure we have the latest code
///
/// This does a git fetch followed by a hard reset to origin/main (or default branch)
pub async fn fresh_pull_before_surgery(
    repo_manager: &RepoManager,
    owner: &str,
    repo: &str,
    default_branch: &str,
) -> Result<String> {
    let repo_path = repo_manager.repo_path(owner, repo);

    // First ensure the repo exists
    if !repo_path.exists() {
        repo_manager.ensure_repo(owner, repo)?;
        return Ok("cloned".to_string());
    }

    let git_repo = Repository::open(&repo_path)
        .context("Failed to open repository for fresh pull")?;

    // Get the current HEAD sha before pull
    let old_head = git_repo.head()?.peel_to_commit()?.id().to_string();

    // Use the repo_manager's ensure_repo which does fetch + fast-forward
    repo_manager.ensure_repo(owner, repo)?;

    // Also checkout and reset to the default branch to handle any local changes
    let refname = format!("refs/heads/{}", default_branch);
    if let Ok(_reference) = git_repo.find_reference(&refname) {
        git_repo.set_head(&refname)?;
        git_repo.checkout_head(Some(
            git2::build::CheckoutBuilder::default()
                .force()
                .remove_untracked(true),
        ))?;
    }

    // Get new HEAD sha
    let new_head = git_repo.head()?.peel_to_commit()?.id().to_string();

    if old_head != new_head {
        info!(
            owner,
            repo,
            old_head = &old_head[..8],
            new_head = &new_head[..8],
            "Fresh pull completed with new commits"
        );
    } else {
        debug!(owner, repo, "Fresh pull completed, already up to date");
    }

    Ok(new_head)
}

/// Get the current surgical queue for a project
pub async fn get_queue(db: &PgPool, project_id: Uuid) -> Result<SurgicalQueue> {
    // Get current lock holder
    let current_lock = sqlx::query!(
        r#"
        SELECT rsl.id, rsl.incident_id, rsl.worker_id, rsl.acquired_at, rsl.expires_at,
               i.title as incident_title, i.fingerprint
        FROM repo_surgical_locks rsl
        JOIN incidents i ON i.id = rsl.incident_id
        WHERE rsl.project_id = $1
        "#,
        project_id
    )
    .fetch_optional(db)
    .await?;

    // Get waiting incidents
    let waiting = sqlx::query!(
        r#"
        SELECT id, title, fingerprint, queue_position, queued_at
        FROM incidents
        WHERE project_id = $1
        AND state = 'detected'
        AND queued_at IS NOT NULL
        ORDER BY queue_position ASC
        LIMIT 10
        "#,
        project_id
    )
    .fetch_all(db)
    .await?;

    Ok(SurgicalQueue {
        current_operation: current_lock.map(|l| CurrentOperation {
            lock_id: l.id,
            incident_id: l.incident_id,
            incident_title: l.incident_title,
            fingerprint: l.fingerprint,
            worker_id: l.worker_id,
            acquired_at: l.acquired_at,
            expires_at: l.expires_at,
        }),
        waiting: waiting
            .into_iter()
            .map(|w| QueuedIncident {
                incident_id: w.id,
                incident_title: w.title,
                fingerprint: w.fingerprint,
                queue_position: w.queue_position.unwrap_or(0),
                queued_at: w.queued_at,
            })
            .collect(),
    })
}

/// Current surgical operation
#[derive(Debug, Clone, Serialize)]
pub struct CurrentOperation {
    pub lock_id: Uuid,
    pub incident_id: Uuid,
    pub incident_title: Option<String>,
    pub fingerprint: String,
    pub worker_id: String,
    pub acquired_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Incident waiting in queue
#[derive(Debug, Clone, Serialize)]
pub struct QueuedIncident {
    pub incident_id: Uuid,
    pub incident_title: Option<String>,
    pub fingerprint: String,
    pub queue_position: i32,
    pub queued_at: Option<DateTime<Utc>>,
}

/// Surgical queue status for a project
#[derive(Debug, Clone, Serialize)]
pub struct SurgicalQueue {
    pub current_operation: Option<CurrentOperation>,
    pub waiting: Vec<QueuedIncident>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_duration() {
        assert_eq!(LOCK_DURATION_MINUTES, 30);
    }
}
