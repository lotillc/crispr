//! Background Worker
//!
//! Processes incidents asynchronously through the pipeline:
//! detected → triaging → fixing → pr_open
//!
//! Implements a distributed claiming system where each worker:
//! 1. Registers itself on startup
//! 2. Claims incidents before processing (preventing duplicate work)
//! 3. Sends heartbeats to indicate it's still alive
//! 4. Releases stale claims from dead workers

use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::api::metrics::{self, StageTimer};
use crate::api::AppState;
use crate::integrations::issue_trackers::{self, IncidentStateChange};
use crate::pipeline::{
    context, crispr_md::CrisprMdManager, fix, pr, repo::RepoManager, surgical, triage,
    verification, DiffCheckResult, LockResult,
};
use crate::store::models::{Incident, Project};

/// Worker instance that processes incidents
pub struct Worker {
    state: AppState,
    worker_id: String,
    repo_manager: Arc<RepoManager>,
}

impl Worker {
    /// Create a new worker
    pub fn new(state: AppState) -> Self {
        let worker_id = state.config.worker.id.clone();
        let repos_dir = &state.config.pipeline.repos_dir;

        // Get GitHub token for repo access
        let github_token = state
            .config
            .github
            .personal_access_token
            .clone();

        let repo_manager = Arc::new(RepoManager::new(repos_dir, github_token));

        Self {
            state,
            worker_id,
            repo_manager,
        }
    }

    /// Run the background worker loop
    pub async fn run(&self) {
        info!(worker_id = %self.worker_id, "Background worker starting");

        // Register this worker
        if let Err(e) = self.register().await {
            error!(error = %e, "Failed to register worker");
            return;
        }

        // Start heartbeat task
        let heartbeat_state = self.state.clone();
        let heartbeat_worker_id = self.worker_id.clone();
        let heartbeat_interval = self.state.config.worker.heartbeat_interval_seconds;
        tokio::spawn(async move {
            heartbeat_loop(&heartbeat_state, &heartbeat_worker_id, heartbeat_interval).await;
        });

        // Start stale claim cleanup task
        let cleanup_state = self.state.clone();
        let claim_timeout = self.state.config.worker.claim_timeout_minutes;
        tokio::spawn(async move {
            cleanup_stale_claims_loop(&cleanup_state, claim_timeout).await;
        });

        // Start verification loop
        let verification_state = self.state.clone();
        tokio::spawn(async move {
            verification_loop(&verification_state).await;
        });

        // Main processing loop
        let mut ticker = interval(Duration::from_secs(10));

        loop {
            ticker.tick().await;

            if let Err(e) = self.process_pending().await {
                error!(error = %e, "Worker error");
            }
        }
    }

    /// Register this worker in the database
    async fn register(&self) -> anyhow::Result<()> {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        sqlx::query!(
            r#"
            INSERT INTO workers (id, hostname, started_at, last_heartbeat)
            VALUES ($1, $2, NOW(), NOW())
            ON CONFLICT (id) DO UPDATE SET
                hostname = $2,
                started_at = NOW(),
                last_heartbeat = NOW(),
                active_incident_id = NULL
            "#,
            &self.worker_id,
            hostname
        )
        .execute(&self.state.db)
        .await?;

        info!(worker_id = %self.worker_id, hostname, "Worker registered");
        Ok(())
    }

    /// Process pending incidents
    async fn process_pending(&self) -> anyhow::Result<()> {
        // Check if LLM is configured
        if !self.state.llm.is_configured().await {
            return Ok(());
        }

        // Check if GitHub is connected
        if !self.state.github.is_connected().await {
            return Ok(());
        }

        // Try to claim and process an incident
        if let Some(incident) = self.claim_incident().await? {
            let result = self.process_incident(&incident).await;

            // Release the claim
            self.release_claim(&incident).await?;

            if let Err(e) = result {
                error!(incident_id = %incident.id, error = %e, "Failed to process incident");

                // Update state to fix_failed
                let attempts = incident.fix_attempts.unwrap_or(0) + 1;
                sqlx::query!(
                    r#"
                    UPDATE incidents
                    SET state = 'fix_failed',
                        fix_attempts = $1,
                        updated_at = NOW()
                    WHERE id = $2
                    "#,
                    attempts,
                    incident.id
                )
                .execute(&self.state.db)
                .await?;

                // Log the failure
                log_activity(
                    &self.state.db,
                    incident.id,
                    Some(incident.project_id),
                    "fix_failed",
                    &serde_json::json!({
                        "error": e.to_string(),
                        "attempt": attempts,
                        "worker_id": self.worker_id,
                    }),
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Attempt to claim an incident for processing
    async fn claim_incident(&self) -> anyhow::Result<Option<Incident>> {
        // Use a transaction to atomically claim an incident
        let mut tx = self.state.db.begin().await?;

        // Find an unclaimed incident ready for processing
        // Exclude incidents where the repo has an active surgical lock
        let incident = sqlx::query_as!(
            Incident,
            r#"
            SELECT i.id, i.project_id, i.fingerprint, i.title, i.state, i.severity,
                   i.occurrence_count, i.first_seen, i.last_seen, i.cooldown_until,
                   i.sample_location, i.fix_attempts, i.pr_url, i.pr_number, i.pr_state,
                   i.tests_generated, i.test_count, i.matched_pattern_id, i.pattern_auto_triaged,
                   i.queued_at, i.queue_position, i.verification_round, i.verification_status,
                   i.verified_at, i.pr_merged_at, i.verification_duration_hours,
                   i.verification_started_at, i.verification_ends_at,
                   i.recurrence_count, i.recurrence_detected_at,
                   i.created_at, i.updated_at
            FROM incidents i
            WHERE i.state = 'detected'
            AND (i.cooldown_until IS NULL OR i.cooldown_until < NOW())
            AND (i.claimed_by IS NULL OR i.claimed_at < NOW() - INTERVAL '15 minutes')
            AND NOT EXISTS (
                SELECT 1 FROM repo_surgical_locks rsl
                WHERE rsl.project_id = i.project_id
                AND rsl.expires_at > NOW()
            )
            ORDER BY i.last_seen DESC
            LIMIT 1
            FOR UPDATE SKIP LOCKED
            "#
        )
        .fetch_optional(&mut *tx)
        .await?;

        let Some(incident) = incident else {
            return Ok(None);
        };

        // Claim it
        sqlx::query!(
            r#"
            UPDATE incidents
            SET claimed_by = $1,
                claimed_at = NOW()
            WHERE id = $2
            "#,
            &self.worker_id,
            incident.id
        )
        .execute(&mut *tx)
        .await?;

        // Update worker's active incident
        sqlx::query!(
            "UPDATE workers SET active_incident_id = $1, last_heartbeat = NOW() WHERE id = $2",
            incident.id,
            &self.worker_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        info!(
            worker_id = %self.worker_id,
            incident_id = %incident.id,
            "Claimed incident for processing"
        );

        Ok(Some(incident))
    }

    /// Release claim on an incident
    async fn release_claim(&self, incident: &Incident) -> anyhow::Result<()> {
        sqlx::query!(
            "UPDATE incidents SET claimed_by = NULL, claimed_at = NULL WHERE id = $1",
            incident.id
        )
        .execute(&self.state.db)
        .await?;

        sqlx::query!(
            "UPDATE workers SET active_incident_id = NULL WHERE id = $1",
            &self.worker_id
        )
        .execute(&self.state.db)
        .await?;

        debug!(incident_id = %incident.id, "Released claim");
        Ok(())
    }

    /// Process a single incident through the pipeline
    async fn process_incident(&self, incident: &Incident) -> anyhow::Result<()> {
        let project = sqlx::query_as!(
            Project,
            r#"
            SELECT id, github_owner, github_repo, default_branch, language,
                   enabled, auto_registered, settings, created_at, updated_at
            FROM projects WHERE id = $1
            "#,
            incident.project_id
        )
        .fetch_one(&self.state.db)
        .await?;

        // Check if project is enabled
        if !project.enabled.unwrap_or(true) {
            info!(incident_id = %incident.id, "Project disabled, skipping");
            return Ok(());
        }

        // Check max attempts
        let max_attempts = self.state.config.pipeline.max_fix_attempts as i32;
        if incident.fix_attempts.unwrap_or(0) >= max_attempts {
            warn!(incident_id = %incident.id, "Max fix attempts reached, entering cooldown");
            set_cooldown(&self.state, incident).await?;
            return Ok(());
        }

        // Check dry run mode
        if self.state.config.safety.dry_run {
            info!(incident_id = %incident.id, "Dry run mode, skipping fix");
            return Ok(());
        }

        info!(
            incident_id = %incident.id,
            worker_id = %self.worker_id,
            project = %format!("{}/{}", project.github_owner, project.github_repo),
            "Processing incident"
        );

        // === ACQUIRE SURGICAL LOCK ===
        let surgical_lock = match surgical::acquire_lock(
            &self.state.db,
            project.id,
            incident.id,
            &self.worker_id,
        )
        .await?
        {
            LockResult::Acquired(lock) => {
                info!(
                    incident_id = %incident.id,
                    lock_id = %lock.id,
                    "Acquired surgical lock"
                );
                lock
            }
            LockResult::Blocked { holder_incident_id, queue_position } => {
                info!(
                    incident_id = %incident.id,
                    holder = %holder_incident_id,
                    queue_position = queue_position,
                    "Repo is locked by another incident, queued"
                );
                log_activity(
                    &self.state.db,
                    incident.id,
                    Some(project.id),
                    "queued",
                    &serde_json::json!({
                        "holder_incident_id": holder_incident_id,
                        "queue_position": queue_position,
                    }),
                )
                .await?;
                return Ok(());
            }
            LockResult::RetryAfterCleanup => {
                debug!(incident_id = %incident.id, "Stale lock cleaned up, will retry");
                return Ok(());
            }
        };

        // From here on, we must release the lock when done
        let result = self.process_incident_with_lock(incident, &project, &surgical_lock).await;

        // Always release the surgical lock
        if let Err(e) = surgical::release_lock(&self.state.db, surgical_lock.id).await {
            error!(error = %e, lock_id = %surgical_lock.id, "Failed to release surgical lock");
        }

        result
    }

    /// Process an incident after acquiring the surgical lock
    async fn process_incident_with_lock(
        &self,
        incident: &Incident,
        project: &Project,
        surgical_lock: &surgical::RepoSurgicalLock,
    ) -> anyhow::Result<()> {
        let default_branch = project.default_branch.as_deref().unwrap_or("main");

        // === FRESH PULL BEFORE SURGERY ===
        info!(
            incident_id = %incident.id,
            repo = %format!("{}/{}", project.github_owner, project.github_repo),
            "Performing fresh pull before surgery"
        );
        let new_head = surgical::fresh_pull_before_surgery(
            &self.repo_manager,
            &project.github_owner,
            &project.github_repo,
            default_branch,
        )
        .await?;
        debug!(incident_id = %incident.id, head = %new_head, "Fresh pull complete");

        // === READ CRISPR.md CONTEXT ===
        let crispr_md = CrisprMdManager::new(&self.repo_manager);
        let crispr_context = crispr_md
            .get_context_for_prompt(&project.github_owner, &project.github_repo)?;

        // Get LLM clients
        let reader = self
            .state
            .llm
            .reader()
            .await
            .ok_or_else(|| anyhow::anyhow!("LLM reader not configured"))?;
        let writer = self
            .state
            .llm
            .writer()
            .await
            .ok_or_else(|| anyhow::anyhow!("LLM writer not configured"))?;

        // Load sample
        let sample_location = incident
            .sample_location
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No sample location"))?;
        let sample_bytes = self.state.storage.get(sample_location).await?;
        let sample: serde_json::Value = serde_json::from_slice(&sample_bytes)?;

        // === PATTERN MATCHING (Pre-triage) ===
        // Try to match against known patterns first to avoid LLM calls
        let pattern_matches = self.state.patterns.match_sample(&sample).await;

        // Record all pattern matches for analytics
        for pattern_match in &pattern_matches {
            if let Err(e) = self.state.patterns.record_match(pattern_match, incident.id).await {
                warn!(error = %e, "Failed to record pattern match");
            }
        }

        // Try auto-triage with pattern matching
        let auto_triage_result = self.state.patterns.auto_triage(&sample).await;

        // === TRIAGE ===
        set_state(&self.state, incident.id, "triaging").await?;

        // Determine if we should use auto-triage or LLM triage
        let triage_result = if let Some(auto_result) = auto_triage_result {
            // Use pattern-based auto-triage
            info!(
                incident_id = %incident.id,
                pattern = ?auto_result.matched_pattern.as_ref().map(|p| &p.pattern_name),
                category = %auto_result.category,
                "Using pattern-based auto-triage (skipping LLM)"
            );

            // Update incident with pattern info
            if let Some(ref pattern_match) = auto_result.matched_pattern {
                if let Err(e) = self.state.patterns
                    .update_incident_pattern(incident.id, pattern_match, true)
                    .await
                {
                    warn!(error = %e, "Failed to update incident pattern info");
                }
            }

            log_activity(
                &self.state.db,
                incident.id,
                Some(project.id),
                "pattern_auto_triaged",
                &serde_json::json!({
                    "worker_id": self.worker_id,
                    "pattern_name": auto_result.matched_pattern.as_ref().map(|p| &p.pattern_name),
                    "category": auto_result.category,
                    "fixable": auto_result.fixable,
                }),
            )
            .await?;

            // Convert AutoTriageResult to TriageResult
            triage::TriageResult {
                fixable: auto_result.fixable,
                confidence: auto_result.confidence,
                reason: auto_result.reason,
                category: auto_result.category,
                suggested_files: auto_result.suggested_files,
                description: auto_result.description,
                fix_approach: auto_result.fix_approach,
            }
        } else {
            // Fall back to LLM-based triage
            log_activity(
                &self.state.db,
                incident.id,
                Some(project.id),
                "triaging",
                &serde_json::json!({
                    "worker_id": self.worker_id,
                    "pattern_matches": pattern_matches.len(),
                }),
            )
            .await?;

            // Include CRISPR.md context in triage
            let triage_timer = StageTimer::start("triage");
            let result = triage::run_with_context(&reader, &sample, incident.id, project.id, &crispr_context)
                .await?;
            triage_timer.stop();

            // If patterns matched but didn't auto-triage, update incident with best match
            if let Some(best_match) = pattern_matches.first() {
                if let Err(e) = self.state.patterns
                    .update_incident_pattern(incident.id, best_match, false)
                    .await
                {
                    warn!(error = %e, "Failed to update incident pattern info");
                }
            }

            result
        };

        if !triage_result.fixable {
            info!(
                incident_id = %incident.id,
                reason = %triage_result.reason,
                category = %triage_result.category,
                "Not fixable"
            );
            set_state(&self.state, incident.id, "unfixable").await?;
            log_activity(
                &self.state.db,
                incident.id,
                Some(project.id),
                "unfixable",
                &serde_json::json!({
                    "reason": triage_result.reason,
                    "category": triage_result.category,
                }),
            )
            .await?;
            // Sync to issue trackers
            sync_to_issue_trackers(&self.state, incident, project, Some("triaging"), "unfixable").await;
            return Ok(());
        }

        info!(
            incident_id = %incident.id,
            category = %triage_result.category,
            confidence = triage_result.confidence,
            "Triage complete, fixable"
        );

        // === PRE-SURGERY DIFF CHECK ===
        // Check if recent commits might have already fixed this issue
        let reader = self
            .state
            .llm
            .reader()
            .await
            .ok_or_else(|| anyhow::anyhow!("LLM reader not configured"))?;

        let diff_check_result = surgical::check_recent_diffs(
            &self.state.db,
            &reader,
            &self.repo_manager,
            project,
            incident.id,
            incident.title.as_deref().unwrap_or("Unknown error"),
            &triage_result.suggested_files,
        )
        .await?;

        match diff_check_result {
            DiffCheckResult::SafeToProceed => {
                debug!(incident_id = %incident.id, "Diff check passed, safe to proceed");
            }
            DiffCheckResult::PendingVerification { reason, relevant_commits } => {
                info!(
                    incident_id = %incident.id,
                    reason = %reason,
                    commits = ?relevant_commits,
                    "Setting to pending_verification due to recent commits"
                );
                set_state(&self.state, incident.id, "pending_verification").await?;
                log_activity(
                    &self.state.db,
                    incident.id,
                    Some(project.id),
                    "pending_verification",
                    &serde_json::json!({
                        "reason": reason,
                        "relevant_commits": relevant_commits,
                    }),
                )
                .await?;
                // Start verification to see if the error stops occurring
                verification::start_verification(&self.state.db, incident.id).await?;
                return Ok(());
            }
            DiffCheckResult::OverlappingFix { related_incident_id, pr_url } => {
                info!(
                    incident_id = %incident.id,
                    related_incident_id = %related_incident_id,
                    pr_url = %pr_url,
                    "Overlapping fix PR exists, setting to pending_verification"
                );
                set_state(&self.state, incident.id, "pending_verification").await?;
                log_activity(
                    &self.state.db,
                    incident.id,
                    Some(project.id),
                    "overlapping_fix",
                    &serde_json::json!({
                        "related_incident_id": related_incident_id,
                        "pr_url": pr_url,
                    }),
                )
                .await?;
                return Ok(());
            }
        }

        // Extend the lock since we're about to do the heavy lifting
        surgical::extend_lock(&self.state.db, surgical_lock.id).await?;

        // === CONTEXT GATHERING ===
        set_state(&self.state, incident.id, "fixing").await?;
        log_activity(
            &self.state.db,
            incident.id,
            Some(project.id),
            "gathering_context",
            &serde_json::json!({
                "suggested_files": triage_result.suggested_files,
            }),
        )
        .await?;

        // Gather context using local repo
        let context_timer = StageTimer::start("context");
        let context = context::gather_from_local(
            &reader,
            &self.repo_manager,
            project,
            &triage_result,
            &sample,
            incident.id,
            &crispr_context,
        )
        .await?;
        context_timer.stop();

        info!(
            incident_id = %incident.id,
            files_count = context.files.len(),
            "Context gathered"
        );

        // === FIX GENERATION ===
        log_activity(
            &self.state.db,
            incident.id,
            Some(project.id),
            "generating_fix",
            &serde_json::json!({
                "files": context.files.iter().map(|f| &f.path).collect::<Vec<_>>(),
            }),
        )
        .await?;

        let fix_timer = StageTimer::start("fix");
        let generated_fix = fix::generate(&writer, &context, incident.id, project.id).await?;
        fix_timer.stop();

        // Store generated tests in the database
        if generated_fix.has_tests() {
            for test in &generated_fix.generated_tests {
                sqlx::query!(
                    r#"
                    INSERT INTO generated_tests (id, incident_id, file_path, content, description, framework, included_in_pr)
                    VALUES ($1, $2, $3, $4, $5, $6, true)
                    "#,
                    Uuid::new_v4(),
                    incident.id,
                    test.path,
                    test.content,
                    test.description,
                    test.framework,
                )
                .execute(&self.state.db)
                .await?;
            }

            // Update incident test_count and tests_generated
            sqlx::query!(
                r#"
                UPDATE incidents
                SET tests_generated = true,
                    test_count = $1,
                    updated_at = NOW()
                WHERE id = $2
                "#,
                generated_fix.test_count() as i32,
                incident.id
            )
            .execute(&self.state.db)
            .await?;

            info!(
                incident_id = %incident.id,
                test_count = generated_fix.test_count(),
                "Stored generated tests in database"
            );
        }

        // Log fix details including multi-file information
        if generated_fix.is_multi_file() {
            info!(
                incident_id = %incident.id,
                changes_count = generated_fix.file_count(),
                files = ?generated_fix.affected_files(),
                confidence = generated_fix.confidence,
                tests_generated = generated_fix.test_count(),
                "Multi-file fix generated"
            );
        } else {
            info!(
                incident_id = %incident.id,
                changes_count = generated_fix.file_count(),
                confidence = generated_fix.confidence,
                tests_generated = generated_fix.test_count(),
                "Fix generated"
            );
        }

        // === PR CREATION ===
        set_state(&self.state, incident.id, "pr_opening").await?;
        log_activity(
            &self.state.db,
            incident.id,
            Some(project.id),
            "creating_pr",
            &serde_json::json!({
                "changes": generated_fix.changes.iter().map(|c| &c.path).collect::<Vec<_>>(),
                "is_multi_file": generated_fix.is_multi_file(),
                "file_count": generated_fix.file_count(),
                "test_count": generated_fix.test_count(),
                "confidence": generated_fix.confidence,
            }),
        )
        .await?;

        // Create PR using local repo (include all tests by default)
        let pr_timer = StageTimer::start("pr");
        let pr_result = pr::create_from_local(
            &self.state.github,
            &self.repo_manager,
            &writer,
            project,
            incident,
            &generated_fix,
            &self.state.config.safety.pr_branch_prefix,
            None, // Include all tests by default
        )
        .await?;
        pr_timer.stop();

        // Update CRISPR.md with the fix record
        let error_summary = incident.title.clone().unwrap_or_else(|| "Unknown error".to_string());
        let fix_summary = generated_fix.explanation.clone();
        crispr_md.record_fix(
            &project.github_owner,
            &project.github_repo,
            &incident.id.to_string(),
            &error_summary,
            &fix_summary,
            Some(&pr_result.url),
        )?;

        // Update incident with PR info
        sqlx::query!(
            r#"
            UPDATE incidents
            SET state = 'pr_open',
                pr_url = $1,
                pr_number = $2,
                pr_state = 'open',
                fix_attempts = fix_attempts + 1,
                updated_at = NOW()
            WHERE id = $3
            "#,
            pr_result.url,
            pr_result.number,
            incident.id
        )
        .execute(&self.state.db)
        .await?;

        log_activity(
            &self.state.db,
            incident.id,
            Some(project.id),
            "pr_opened",
            &serde_json::json!({
                "url": pr_result.url,
                "number": pr_result.number,
                "branch": pr_result.branch,
                "worker_id": self.worker_id,
            }),
        )
        .await?;

        info!(
            incident_id = %incident.id,
            pr_url = %pr_result.url,
            pr_number = pr_result.number,
            "PR opened successfully"
        );

        // Sync to issue trackers with updated PR info
        let mut incident_with_pr = incident.clone();
        incident_with_pr.pr_url = Some(pr_result.url.clone());
        incident_with_pr.pr_number = Some(pr_result.number);
        sync_to_issue_trackers(&self.state, &incident_with_pr, project, Some("pr_opening"), "pr_open").await;

        // Broadcast PR created event via SSE
        self.state.events.incident_updated(
            incident.id,
            "pr_open",
            Some(&pr_result.url),
            Some(pr_result.number),
        );
        crate::api::events::broadcast_stats(&self.state).await;

        // Notify Slack if connected
        if self.state.slack.is_connected().await {
            let project_name = format!("{}/{}", project.github_owner, project.github_repo);
            let error_msg = incident.title.as_deref().unwrap_or("Unknown error");
            if let Err(e) = self
                .state
                .slack
                .notify_pr_opened(
                    &incident.id,
                    &pr_result.url,
                    &pr_result.title,
                    pr_result.number,
                    &project_name,
                    error_msg,
                    generated_fix.confidence,
                )
                .await
            {
                warn!(error = %e, "Failed to send Slack notification");
            }
        }

        Ok(())
    }
}

/// Heartbeat loop - periodically update the worker's last_heartbeat
async fn heartbeat_loop(state: &AppState, worker_id: &str, interval_secs: u32) {
    let mut ticker = interval(Duration::from_secs(interval_secs as u64));

    loop {
        ticker.tick().await;

        if let Err(e) = sqlx::query!(
            "UPDATE workers SET last_heartbeat = NOW() WHERE id = $1",
            worker_id
        )
        .execute(&state.db)
        .await
        {
            warn!(error = %e, "Failed to send heartbeat");
        }
    }
}

/// Cleanup stale claims from dead workers
async fn cleanup_stale_claims_loop(state: &AppState, timeout_minutes: u32) {
    let mut ticker = interval(Duration::from_secs(60)); // Check every minute

    loop {
        ticker.tick().await;

        // Release claims from workers that haven't sent a heartbeat
        let timeout_interval = chrono::Duration::minutes(timeout_minutes as i64);
        let cutoff = chrono::Utc::now() - timeout_interval;

        let result: Result<sqlx::postgres::PgQueryResult, sqlx::Error> = sqlx::query!(
            r#"
            UPDATE incidents
            SET claimed_by = NULL, claimed_at = NULL
            WHERE claimed_by IS NOT NULL
            AND claimed_by IN (
                SELECT id FROM workers
                WHERE last_heartbeat < $1
            )
            "#,
            cutoff
        )
        .execute(&state.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => {
                info!(
                    count = r.rows_affected(),
                    "Released stale claims from dead workers"
                );
            }
            Err(e) => {
                warn!(error = %e, "Failed to cleanup stale claims");
            }
            _ => {}
        }

        // Also cleanup old oauth states
        let _ = sqlx::query!("DELETE FROM oauth_states WHERE expires_at < NOW()")
            .execute(&state.db)
            .await;

        // Cleanup expired surgical locks
        if let Err(e) = surgical::cleanup_expired_locks(&state.db).await {
            warn!(error = %e, "Failed to cleanup expired surgical locks");
        }
    }
}

/// Verification loop - checks and advances verification rounds
async fn verification_loop(state: &AppState) {
    let mut ticker = interval(Duration::from_secs(300)); // Check every 5 minutes

    loop {
        ticker.tick().await;

        // Get all incidents in verification
        let verifying = match verification::get_verifying_incidents(&state.db).await {
            Ok(ids) => ids,
            Err(e) => {
                warn!(error = %e, "Failed to get verifying incidents");
                continue;
            }
        };

        for incident_id in verifying {
            match verification::advance_verification(&state.db, incident_id).await {
                Ok(complete) => {
                    if complete {
                        // Broadcast the state change and sync to issue trackers
                        let incident_data = sqlx::query!(
                            r#"
                            SELECT i.id, i.project_id, i.fingerprint, i.title, i.state,
                                   i.verification_status, i.pr_url, i.pr_number, i.fix_attempts,
                                   i.verification_duration_hours,
                                   p.github_owner, p.github_repo
                            FROM incidents i
                            JOIN projects p ON p.id = i.project_id
                            WHERE i.id = $1
                            "#,
                            incident_id
                        )
                        .fetch_optional(&state.db)
                        .await;

                        if let Ok(Some(i)) = incident_data {
                            let new_state = i.state.as_deref().unwrap_or("verified");
                            state.events.incident_updated(incident_id, new_state, None, None);
                            crate::api::events::broadcast_stats(state).await;

                            // Log verification completion
                            let status = i.verification_status.as_deref().unwrap_or("unknown");
                            info!(
                                incident_id = %incident_id,
                                status = status,
                                "Verification completed"
                            );

                            // Sync to issue trackers
                            let event = IncidentStateChange {
                                incident_id: i.id,
                                project_id: i.project_id,
                                fingerprint: i.fingerprint,
                                title: i.title,
                                old_state: Some("verifying".to_string()),
                                new_state: new_state.to_string(),
                                message: None,
                                pr_url: i.pr_url,
                                pr_number: i.pr_number,
                                fix_attempts: i.fix_attempts.unwrap_or(0),
                                verification_duration_hours: i.verification_duration_hours,
                                occurred_at: chrono::Utc::now(),
                                github_owner: i.github_owner,
                                github_repo: i.github_repo,
                            };
                            if let Err(e) = issue_trackers::sync_incident_state(&state.db, &state.github, &event).await {
                                warn!(error = %e, incident_id = %incident_id, "Failed to sync verification to issue trackers");
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        incident_id = %incident_id,
                        "Failed to advance verification"
                    );
                }
            }
        }
    }
}

/// Legacy run function for backwards compatibility
pub async fn run(state: AppState) {
    let worker = Worker::new(state);
    worker.run().await;
}

/// Set incident state and broadcast via SSE
async fn set_state(state: &AppState, incident_id: Uuid, new_state: &str) -> anyhow::Result<()> {
    sqlx::query!(
        "UPDATE incidents SET state = $1, updated_at = NOW() WHERE id = $2",
        new_state,
        incident_id
    )
    .execute(&state.db)
    .await?;

    // Record metric
    metrics::record_incident(new_state);

    // Broadcast state change via SSE
    state.events.incident_updated(incident_id, new_state, None, None);
    crate::api::events::broadcast_stats(state).await;

    Ok(())
}

/// Set incident to cooldown
async fn set_cooldown(state: &AppState, incident: &Incident) -> anyhow::Result<()> {
    let cooldown_hours = state.config.pipeline.cooldown_hours as i64;
    let cooldown_until = chrono::Utc::now() + chrono::Duration::hours(cooldown_hours);

    sqlx::query!(
        r#"
        UPDATE incidents
        SET state = 'cooldown',
            cooldown_until = $1,
            updated_at = NOW()
        WHERE id = $2
        "#,
        cooldown_until,
        incident.id
    )
    .execute(&state.db)
    .await?;

    log_activity(
        &state.db,
        incident.id,
        Some(incident.project_id),
        "cooldown",
        &serde_json::json!({
            "until": cooldown_until.to_rfc3339(),
            "hours": cooldown_hours,
        }),
    )
    .await?;

    Ok(())
}

/// Log an activity entry
async fn log_activity(
    db: &sqlx::PgPool,
    incident_id: Uuid,
    project_id: Option<Uuid>,
    action: &str,
    details: &serde_json::Value,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO activity_log (id, incident_id, project_id, action, details)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        Uuid::new_v4(),
        incident_id,
        project_id,
        action,
        details,
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Sync incident state change to external issue trackers
async fn sync_to_issue_trackers(
    state: &AppState,
    incident: &Incident,
    project: &Project,
    old_state: Option<&str>,
    new_state: &str,
) {
    let event = IncidentStateChange {
        incident_id: incident.id,
        project_id: project.id,
        fingerprint: incident.fingerprint.clone(),
        title: incident.title.clone(),
        old_state: old_state.map(String::from),
        new_state: new_state.to_string(),
        message: incident.title.clone(),
        pr_url: incident.pr_url.clone(),
        pr_number: incident.pr_number,
        fix_attempts: incident.fix_attempts.unwrap_or(0),
        verification_duration_hours: incident.verification_duration_hours,
        occurred_at: chrono::Utc::now(),
        github_owner: project.github_owner.clone(),
        github_repo: project.github_repo.clone(),
    };

    if let Err(e) = issue_trackers::sync_incident_state(&state.db, &state.github, &event).await {
        warn!(
            error = %e,
            incident_id = %incident.id,
            new_state = new_state,
            "Failed to sync to issue trackers"
        );
    }
}
