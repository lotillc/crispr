// OTLP gRPC ingest server
// Receives OpenTelemetry logs via gRPC and converts them to our ingest format

use std::net::SocketAddr;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{info, warn};

use super::AppState;

// For Phase 2, we'll implement a simplified gRPC server
// Full OTLP proto support will be added if needed

/// Placeholder for OTLP log service
/// In a full implementation, this would use the OTLP proto definitions
pub struct OtlpLogService {
    #[allow(dead_code)]
    state: AppState,
}

impl OtlpLogService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}

/// Start the gRPC server for OTLP log ingestion
pub async fn serve_grpc(state: AppState, addr: &str) -> Result<(), anyhow::Error> {
    let addr: SocketAddr = addr.parse()?;

    info!(addr = %addr, "gRPC server starting (OTLP placeholder)");

    // For now, we'll just start a basic gRPC server
    // Full OTLP implementation requires the proto definitions
    // which add significant complexity

    // In production, you would:
    // 1. Add opentelemetry-proto crate or compile protos with tonic-build
    // 2. Implement the LogsService trait
    // 3. Convert OTLP LogRecord to our IngestPayload format

    // For Phase 2, we provide HTTP ingest which is simpler and works well
    // OTLP support can be added in a future iteration

    info!(
        addr = %addr,
        "gRPC server ready (use HTTP POST /ingest for now)"
    );

    // Keep the task alive but don't actually serve
    // This is a placeholder that can be replaced with full OTLP support
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}
