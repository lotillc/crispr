//! Verification Rounds Module
//!
//! Implements post-deployment verification to ensure fixes don't regress.
//! Uses single-round verification with LLM-recommended duration per incident.
//!
//! The LLM recommends a monitoring duration (24h to 30 days) based on:
//! - Error pattern (transient vs persistent)
//! - Impact severity
//! - Whether it involves time-based behavior (billing cycles, cron jobs, etc.)
//!
//! If the error reoccurs during monitoring, the fix is marked as "recurred".
//! If monitoring passes without recurrence, the fix is marked as "verified".

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Default verification duration in hours (used if LLM doesn't recommend one)
const DEFAULT_VERIFICATION_HOURS: i32 = 24;

/// Status of a verification check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Currently monitoring, waiting for the round to complete
    Monitoring {
        round: i32,
        remaining_hours: i32,
        started_at: DateTime<Utc>,
    },
    /// All verification rounds passed
    Verified {
        completed_at: DateTime<Utc>,
        total_monitoring_hours: i32,
    },
    /// Error recurred during verification
    Recurred {
        round: i32,
        new_occurrences: i32,
        failed_at: DateTime<Utc>,
    },
    /// Not yet started verification
    NotStarted,
}

/// A verification round record
#[derive(Debug, Clone, Serialize)]
pub struct VerificationRound {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub round_number: i32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub occurrence_count_start: i32,
    pub occurrence_count_end: Option<i32>,
    pub duration_hours: i32,
}

/// Start verification for an incident after PR merge
///
/// Uses the LLM-recommended duration stored in verification_duration_hours,
/// or falls back to DEFAULT_VERIFICATION_HOURS (24h) if not set.
pub async fn start_verification(db: &PgPool, incident_id: Uuid) -> Result<VerificationRound> {
    // Get current occurrence count and LLM-recommended duration
    let incident = sqlx::query!(
        r#"
        SELECT occurrence_count, state, verification_status, verification_duration_hours
        FROM incidents
        WHERE id = $1
        "#,
        incident_id
    )
    .fetch_one(db)
    .await?;

    // Check if already verifying
    if incident.verification_status.as_deref() == Some("verifying") {
        return Err(anyhow!("Incident is already in verification"));
    }

    let occurrence_count = incident.occurrence_count.unwrap_or(0);
    let round_number = 1;
    // Use LLM-recommended duration or default to 24 hours
    let duration = incident.verification_duration_hours.unwrap_or(DEFAULT_VERIFICATION_HOURS);
    let round_id = Uuid::new_v4();
    let now = Utc::now();
    let ends_at = now + chrono::Duration::hours(duration as i64);

    let mut tx = db.begin().await?;

    // Create the first verification round
    sqlx::query!(
        r#"
        INSERT INTO verification_rounds (id, incident_id, round_number, started_at, status, occurrence_count_start, duration_hours)
        VALUES ($1, $2, $3, $4, 'monitoring', $5, $6)
        "#,
        round_id,
        incident_id,
        round_number,
        now,
        occurrence_count,
        duration
    )
    .execute(&mut *tx)
    .await?;

    // Update incident state with verification timing
    sqlx::query!(
        r#"
        UPDATE incidents
        SET verification_status = 'verifying',
            verification_round = 1,
            state = 'verifying',
            verification_started_at = $1,
            verification_ends_at = $2,
            updated_at = NOW()
        WHERE id = $3
        "#,
        now,
        ends_at,
        incident_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    info!(
        incident_id = %incident_id,
        duration_hours = duration,
        ends_at = %ends_at,
        occurrence_count = occurrence_count,
        "Started verification monitoring"
    );

    Ok(VerificationRound {
        id: round_id,
        incident_id,
        round_number,
        started_at: now,
        completed_at: None,
        status: "monitoring".to_string(),
        occurrence_count_start: occurrence_count,
        occurrence_count_end: None,
        duration_hours: duration,
    })
}

/// Check the verification status of an incident
pub async fn check_verification(db: &PgPool, incident_id: Uuid) -> Result<VerificationStatus> {
    // Get current round
    let round = sqlx::query!(
        r#"
        SELECT id, round_number, started_at, completed_at, status, occurrence_count_start, duration_hours
        FROM verification_rounds
        WHERE incident_id = $1 AND status = 'monitoring'
        ORDER BY round_number DESC
        LIMIT 1
        "#,
        incident_id
    )
    .fetch_optional(db)
    .await?;

    let Some(round) = round else {
        // Check if already verified or recurred
        let incident = sqlx::query!(
            "SELECT verification_status, verified_at, verification_round FROM incidents WHERE id = $1",
            incident_id
        )
        .fetch_one(db)
        .await?;

        return match incident.verification_status.as_deref() {
            Some("verified") => Ok(VerificationStatus::Verified {
                completed_at: incident.verified_at.unwrap_or_else(Utc::now),
                total_monitoring_hours: 0, // Will be set from round data
            }),
            Some("recurred") => {
                // Get the failed round info
                let failed_round = sqlx::query!(
                    "SELECT round_number, occurrence_count_end, completed_at FROM verification_rounds WHERE incident_id = $1 AND status = 'failed' ORDER BY round_number DESC LIMIT 1",
                    incident_id
                )
                .fetch_optional(db)
                .await?;

                if let Some(fr) = failed_round {
                    Ok(VerificationStatus::Recurred {
                        round: fr.round_number,
                        new_occurrences: fr.occurrence_count_end.unwrap_or(0),
                        failed_at: fr.completed_at.unwrap_or_else(Utc::now),
                    })
                } else {
                    Ok(VerificationStatus::Recurred {
                        round: incident.verification_round.unwrap_or(1),
                        new_occurrences: 0,
                        failed_at: Utc::now(),
                    })
                }
            }
            _ => Ok(VerificationStatus::NotStarted),
        };
    };

    let started_at: DateTime<Utc> = round.started_at.unwrap_or_else(Utc::now);
    let duration_hours = round.duration_hours;
    let elapsed = Utc::now() - started_at;
    let remaining_hours = duration_hours - elapsed.num_hours() as i32;

    if remaining_hours > 0 {
        Ok(VerificationStatus::Monitoring {
            round: round.round_number,
            remaining_hours,
            started_at,
        })
    } else {
        // Round duration has elapsed - need to process
        Ok(VerificationStatus::Monitoring {
            round: round.round_number,
            remaining_hours: 0,
            started_at,
        })
    }
}

/// Advance verification to the next round or complete it
///
/// This should be called periodically by the verification loop.
/// Returns true if verification is complete (either verified or recurred).
pub async fn advance_verification(db: &PgPool, incident_id: Uuid) -> Result<bool> {
    // Get current monitoring round
    let round = sqlx::query!(
        r#"
        SELECT id, round_number, started_at, occurrence_count_start, duration_hours
        FROM verification_rounds
        WHERE incident_id = $1 AND status = 'monitoring'
        ORDER BY round_number DESC
        LIMIT 1
        "#,
        incident_id
    )
    .fetch_optional(db)
    .await?;

    let Some(round) = round else {
        debug!(incident_id = %incident_id, "No active verification round");
        return Ok(true); // Already complete
    };

    let started_at: DateTime<Utc> = round.started_at.unwrap_or_else(Utc::now);
    let elapsed = Utc::now() - started_at;

    // Check if round duration has elapsed
    if elapsed.num_hours() < round.duration_hours as i64 {
        return Ok(false); // Still monitoring
    }

    // Get current occurrence count
    let incident = sqlx::query!(
        "SELECT occurrence_count FROM incidents WHERE id = $1",
        incident_id
    )
    .fetch_one(db)
    .await?;

    let current_count = incident.occurrence_count.unwrap_or(0);
    let start_count = round.occurrence_count_start;
    let new_occurrences = current_count - start_count;

    let mut tx = db.begin().await?;

    if new_occurrences > 0 {
        // Error recurred - mark as failed
        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE verification_rounds
            SET status = 'failed',
                completed_at = $1,
                occurrence_count_end = $2
            WHERE id = $3
            "#,
            now,
            current_count,
            round.id
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"
            UPDATE incidents
            SET verification_status = 'recurred',
                state = 'detected',
                updated_at = NOW()
            WHERE id = $1
            "#,
            incident_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        warn!(
            incident_id = %incident_id,
            round = round.round_number,
            new_occurrences = new_occurrences,
            "Verification failed - error recurred"
        );

        return Ok(true);
    }

    // Verification passed - mark as verified
    let now = Utc::now();

    // Mark the round as passed
    sqlx::query!(
        r#"
        UPDATE verification_rounds
        SET status = 'passed',
            completed_at = $1,
            occurrence_count_end = $2
        WHERE id = $3
        "#,
        now,
        current_count,
        round.id
    )
    .execute(&mut *tx)
    .await?;

    // Mark incident as verified
    sqlx::query!(
        r#"
        UPDATE incidents
        SET verification_status = 'verified',
            state = 'verified',
            verified_at = $1,
            updated_at = NOW()
        WHERE id = $2
        "#,
        now,
        incident_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    info!(
        incident_id = %incident_id,
        duration_hours = round.duration_hours,
        "Verification passed - fix verified"
    );

    Ok(true)
}

/// Get all verification rounds for an incident
pub async fn get_verification_rounds(db: &PgPool, incident_id: Uuid) -> Result<Vec<VerificationRound>> {
    let rows = sqlx::query!(
        r#"
        SELECT id, incident_id, round_number, started_at, completed_at, status,
               occurrence_count_start, occurrence_count_end, duration_hours
        FROM verification_rounds
        WHERE incident_id = $1
        ORDER BY round_number ASC
        "#,
        incident_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| VerificationRound {
            id: r.id,
            incident_id: r.incident_id,
            round_number: r.round_number,
            started_at: r.started_at.unwrap_or_else(Utc::now),
            completed_at: r.completed_at,
            status: r.status,
            occurrence_count_start: r.occurrence_count_start,
            occurrence_count_end: r.occurrence_count_end,
            duration_hours: r.duration_hours,
        })
        .collect())
}

/// Get all incidents currently in verification
pub async fn get_verifying_incidents(db: &PgPool) -> Result<Vec<Uuid>> {
    let rows = sqlx::query_scalar!(
        r#"
        SELECT id as "id!"
        FROM incidents
        WHERE verification_status = 'verifying'
        "#
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// Handle PR merge event - start verification
pub async fn handle_pr_merged(
    db: &PgPool,
    incident_id: Uuid,
    merged_at: DateTime<Utc>,
) -> Result<()> {
    let mut tx = db.begin().await?;

    // Update incident with merge info
    sqlx::query!(
        r#"
        UPDATE incidents
        SET pr_state = 'merged',
            pr_merged_at = $1,
            state = 'pr_merged',
            updated_at = NOW()
        WHERE id = $2
        "#,
        merged_at,
        incident_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    info!(incident_id = %incident_id, merged_at = %merged_at, "PR merged");

    // Start verification rounds
    start_verification(db, incident_id).await?;

    Ok(())
}

/// Handle PR closed without merge
pub async fn handle_pr_closed(db: &PgPool, incident_id: Uuid) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE incidents
        SET pr_state = 'closed',
            state = 'pr_closed',
            updated_at = NOW()
        WHERE id = $1
        "#,
        incident_id
    )
    .execute(db)
    .await?;

    info!(incident_id = %incident_id, "PR closed without merge");

    Ok(())
}

/// Summary of verification status for API response
#[derive(Debug, Clone, Serialize)]
pub struct VerificationSummary {
    pub incident_id: Uuid,
    pub status: String,
    pub current_round: Option<i32>,
    pub total_rounds: i32,
    pub rounds: Vec<VerificationRound>,
    pub started_at: Option<DateTime<Utc>>,
    pub verified_at: Option<DateTime<Utc>>,
}

/// Get a summary of verification for an incident
pub async fn get_verification_summary(db: &PgPool, incident_id: Uuid) -> Result<VerificationSummary> {
    let incident = sqlx::query!(
        r#"
        SELECT verification_status, verification_round, verified_at, verification_duration_hours
        FROM incidents
        WHERE id = $1
        "#,
        incident_id
    )
    .fetch_one(db)
    .await?;

    let rounds = get_verification_rounds(db, incident_id).await?;
    let started_at = rounds.first().map(|r| r.started_at);

    Ok(VerificationSummary {
        incident_id,
        status: incident.verification_status.unwrap_or_else(|| "not_started".to_string()),
        current_round: incident.verification_round,
        total_rounds: 1, // Single-round verification with per-incident duration
        rounds,
        started_at,
        verified_at: incident.verified_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_verification_hours() {
        assert_eq!(DEFAULT_VERIFICATION_HOURS, 24);
    }
}
