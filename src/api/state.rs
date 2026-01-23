use crate::config::Config;
use crate::store::DbPool;

/// Application state shared across handlers
/// Will be expanded in Phase 2
#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
}
