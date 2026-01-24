//! Server-Sent Events (SSE) for real-time updates
//!
//! Provides a streaming endpoint at GET /api/v1/events that broadcasts
//! incident and stats updates to connected clients.

use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::debug;
use uuid::Uuid;

use super::AppState;

/// Event types that can be broadcast to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SseEvent {
    /// New incident created
    #[serde(rename = "incident:created")]
    IncidentCreated(IncidentCreatedEvent),

    /// Incident status changed
    #[serde(rename = "incident:updated")]
    IncidentUpdated(IncidentUpdatedEvent),

    /// Stats updated (periodic or after changes)
    #[serde(rename = "stats:updated")]
    StatsUpdated(StatsUpdatedEvent),

    /// Heartbeat to keep connection alive
    #[serde(rename = "heartbeat")]
    Heartbeat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentCreatedEvent {
    pub id: Uuid,
    pub project_id: Uuid,
    pub message: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentUpdatedEvent {
    pub id: Uuid,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsUpdatedEvent {
    pub total: i64,
    pub by_state: Vec<StateCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCount {
    pub state: String,
    pub count: i64,
}

/// Event broadcaster - manages subscriptions and broadcasting
#[derive(Clone)]
pub struct EventBroadcaster {
    sender: broadcast::Sender<SseEvent>,
}

impl EventBroadcaster {
    /// Create a new event broadcaster with the specified capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<SseEvent> {
        self.sender.subscribe()
    }

    /// Broadcast an event to all subscribers
    pub fn broadcast(&self, event: SseEvent) {
        // Ignore send errors - they just mean no one is listening
        let _ = self.sender.send(event);
    }

    /// Broadcast incident created event
    pub fn incident_created(&self, id: Uuid, project_id: Uuid, message: &str, status: &str) {
        self.broadcast(SseEvent::IncidentCreated(IncidentCreatedEvent {
            id,
            project_id,
            message: message.to_string(),
            status: status.to_string(),
        }));
        debug!(incident_id = %id, "Broadcast incident:created event");
    }

    /// Broadcast incident updated event
    pub fn incident_updated(
        &self,
        id: Uuid,
        status: &str,
        pr_url: Option<&str>,
        pr_number: Option<i32>,
    ) {
        self.broadcast(SseEvent::IncidentUpdated(IncidentUpdatedEvent {
            id,
            status: status.to_string(),
            pr_url: pr_url.map(String::from),
            pr_number,
        }));
        debug!(incident_id = %id, status = %status, "Broadcast incident:updated event");
    }

    /// Broadcast stats updated event
    pub fn stats_updated(&self, total: i64, by_state: Vec<(String, i64)>) {
        self.broadcast(SseEvent::StatsUpdated(StatsUpdatedEvent {
            total,
            by_state: by_state
                .into_iter()
                .map(|(state, count)| StateCount { state, count })
                .collect(),
        }));
        debug!(total = total, "Broadcast stats:updated event");
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new(256) // Default capacity of 256 events
    }
}

/// GET /api/v1/events - SSE endpoint for real-time updates
pub async fn handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.events.subscribe();

    // Convert broadcast receiver to a stream
    let stream = BroadcastStream::new(receiver)
        .filter_map(|result| {
            match result {
                Ok(event) => {
                    // Serialize the event
                    match serde_json::to_string(&event) {
                        Ok(json) => {
                            let event_type = match &event {
                                SseEvent::IncidentCreated(_) => "incident:created",
                                SseEvent::IncidentUpdated(_) => "incident:updated",
                                SseEvent::StatsUpdated(_) => "stats:updated",
                                SseEvent::Heartbeat => "heartbeat",
                            };
                            Some(Ok(Event::default().event(event_type).data(json)))
                        }
                        Err(_) => None,
                    }
                }
                Err(_) => None, // Skip lagged messages
            }
        });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("heartbeat"),
    )
}

/// Helper function to fetch and broadcast current stats
pub async fn broadcast_stats(state: &AppState) {
    if let Ok(stats) = fetch_stats(&state.db).await {
        state.events.stats_updated(stats.0, stats.1);
    }
}

/// Fetch current stats from database
async fn fetch_stats(db: &sqlx::PgPool) -> Result<(i64, Vec<(String, i64)>), sqlx::Error> {
    let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM incidents")
        .fetch_one(db)
        .await?
        .unwrap_or(0);

    let by_state = sqlx::query!(
        r#"
        SELECT state as "state!", COUNT(*)::bigint as "count!"
        FROM incidents
        GROUP BY state
        ORDER BY "count!" DESC
        "#
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.state, r.count))
    .collect();

    Ok((total, by_state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = SseEvent::IncidentCreated(IncidentCreatedEvent {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            message: "Test error".to_string(),
            status: "detected".to_string(),
        });

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("incident:created"));
        assert!(json.contains("Test error"));
    }

    #[test]
    fn test_broadcaster_default_capacity() {
        let broadcaster = EventBroadcaster::default();
        let _rx = broadcaster.subscribe();
        // Should be able to send at least one message
        broadcaster.broadcast(SseEvent::Heartbeat);
    }
}
