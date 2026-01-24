//! Test utilities for CRISPR integration tests
//!
//! Provides:
//! - Test database setup and teardown
//! - Mock LLM providers
//! - Mock GitHub client
//! - Test fixtures for incidents and projects

use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod fixtures;
pub mod mocks;

/// Test database configuration
const TEST_DATABASE_URL: &str = "postgres://crispr:crispr@localhost:5432/crispr_test";

/// Create a test database pool
///
/// Note: Requires a running PostgreSQL instance with the crispr_test database.
/// Run migrations before running tests.
pub async fn create_test_db() -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(TEST_DATABASE_URL)
        .await?;

    Ok(pool)
}

/// Clean up test data from the database
///
/// Truncates all tables except migrations table.
pub async fn cleanup_test_db(pool: &PgPool) -> Result<()> {
    // Delete in reverse dependency order
    sqlx::query("DELETE FROM activity_log").execute(pool).await?;
    sqlx::query("DELETE FROM samples").execute(pool).await?;
    sqlx::query("DELETE FROM generated_tests").execute(pool).await?;
    sqlx::query("DELETE FROM llm_usage").execute(pool).await?;
    sqlx::query("DELETE FROM worker_claims").execute(pool).await?;
    sqlx::query("DELETE FROM incidents").execute(pool).await?;
    sqlx::query("DELETE FROM projects").execute(pool).await?;
    sqlx::query("DELETE FROM llm_config").execute(pool).await?;
    sqlx::query("DELETE FROM github_connections").execute(pool).await?;
    sqlx::query("DELETE FROM slack_connections").execute(pool).await?;
    sqlx::query("DELETE FROM oauth_states").execute(pool).await?;
    sqlx::query("DELETE FROM instance_config").execute(pool).await?;

    Ok(())
}

/// Test context that holds all test resources
pub struct TestContext {
    pub db: PgPool,
    pub app_state: TestAppState,
}

impl TestContext {
    /// Create a new test context with clean database
    pub async fn new() -> Result<Self> {
        let db = create_test_db().await?;
        cleanup_test_db(&db).await?;

        let app_state = TestAppState::new(db.clone()).await?;

        Ok(Self { db, app_state })
    }

    /// Get the test router for HTTP testing
    pub fn router(&self) -> Router {
        self.app_state.router()
    }

    /// Create a test project
    pub async fn create_project(&self, owner: &str, repo: &str) -> Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query!(
            r#"
            INSERT INTO projects (id, github_owner, github_repo, default_branch, language, enabled)
            VALUES ($1, $2, $3, 'main', 'typescript', true)
            "#,
            id,
            owner,
            repo
        )
        .execute(&self.db)
        .await?;

        Ok(id)
    }

    /// Create a test incident
    pub async fn create_incident(
        &self,
        project_id: Uuid,
        title: &str,
        state: &str,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let fingerprint = format!("fp_{}", Uuid::new_v4());

        sqlx::query!(
            r#"
            INSERT INTO incidents (id, project_id, fingerprint, title, state, severity)
            VALUES ($1, $2, $3, $4, $5, 'medium')
            "#,
            id,
            project_id,
            fingerprint,
            title,
            state
        )
        .execute(&self.db)
        .await?;

        Ok(id)
    }

    /// Get incident state
    pub async fn get_incident_state(&self, id: Uuid) -> Result<String> {
        let state = sqlx::query_scalar!(
            "SELECT state FROM incidents WHERE id = $1",
            id
        )
        .fetch_one(&self.db)
        .await?
        .unwrap_or_else(|| "unknown".to_string());

        Ok(state)
    }
}

/// Simple event broadcaster for tests
#[derive(Clone)]
pub struct TestEventBroadcaster {
    sender: broadcast::Sender<String>,
}

impl TestEventBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn broadcast(&self, event: &str) {
        let _ = self.sender.send(event.to_string());
    }
}

impl Default for TestEventBroadcaster {
    fn default() -> Self {
        Self::new(64)
    }
}

/// Simplified test app state for testing
pub struct TestAppState {
    pub db: PgPool,
    pub mock_llm: Arc<mocks::MockLlmManager>,
    pub mock_github: Arc<mocks::MockGitHubClient>,
    pub events: TestEventBroadcaster,
    pub storage: TestSampleStore,
}

impl TestAppState {
    pub async fn new(db: PgPool) -> Result<Self> {
        Ok(Self {
            db: db.clone(),
            mock_llm: Arc::new(mocks::MockLlmManager::new()),
            mock_github: Arc::new(mocks::MockGitHubClient::new()),
            events: TestEventBroadcaster::new(64),
            storage: TestSampleStore::new(db),
        })
    }

    /// Create a test router with mock services
    pub fn router(&self) -> Router {
        // For basic HTTP tests, we'll create a minimal router
        // that tests the essential endpoints
        use axum::routing::get;

        Router::new()
            .route("/health", get(|| async { "OK" }))
            .route("/ready", get({
                let db = self.db.clone();
                move || {
                    let db = db.clone();
                    async move {
                        match sqlx::query("SELECT 1").execute(&db).await {
                            Ok(_) => "OK",
                            Err(_) => "ERROR",
                        }
                    }
                }
            }))
    }
}

/// Test sample storage (in-memory)
#[derive(Clone)]
pub struct TestSampleStore {
    db: PgPool,
    samples: Arc<RwLock<std::collections::HashMap<String, Vec<u8>>>>,
}

impl TestSampleStore {
    pub fn new(db: PgPool) -> Self {
        Self {
            db,
            samples: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn put(&self, incident_id: Uuid, data: &[u8]) -> Result<String> {
        let location = format!("mem:{}", Uuid::new_v4());
        self.samples.write().await.insert(location.clone(), data.to_vec());

        // Update incident with sample location
        sqlx::query!(
            "UPDATE incidents SET sample_location = $1 WHERE id = $2",
            &location,
            incident_id
        )
        .execute(&self.db)
        .await?;

        Ok(location)
    }

    pub async fn get(&self, location: &str) -> Result<Vec<u8>> {
        self.samples
            .read()
            .await
            .get(location)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Sample not found: {}", location))
    }
}

/// Helper to run async tests with timeout
pub async fn with_timeout<F, T>(duration: std::time::Duration, f: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    tokio::time::timeout(duration, f)
        .await
        .map_err(|_| anyhow::anyhow!("Test timed out"))?
}
