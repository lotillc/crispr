use std::sync::Arc;

use crate::config::Config;
use crate::integrations::{github::GitHubClient, llm::LlmManager, slack::SlackClient};
use crate::patterns::PatternMatcher;
use crate::store::{object::SampleStore, DbPool};

pub mod auth;
pub mod events;
pub mod grpc;
pub mod incidents;
pub mod ingest;
pub mod issue_trackers;
pub mod metrics;
pub mod patterns;
pub mod projects;
pub mod rate_limit;
pub mod router;
pub mod settings;
pub mod state;
pub mod webhooks;

pub use events::EventBroadcaster;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
    pub storage: SampleStore,
    pub github: Arc<GitHubClient>,
    pub slack: Arc<SlackClient>,
    pub llm: Arc<LlmManager>,
    pub events: EventBroadcaster,
    pub patterns: Arc<PatternMatcher>,
}
