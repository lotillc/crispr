//! Prometheus Metrics
//!
//! Exposes metrics for monitoring CRISPR operations.

use prometheus::{
    self, Counter, CounterVec, Encoder, HistogramOpts, HistogramVec, Opts, Registry, TextEncoder,
};
use std::sync::OnceLock;
use std::time::Instant;

/// Global metrics registry
static REGISTRY: OnceLock<Metrics> = OnceLock::new();

/// Metrics collection for CRISPR
pub struct Metrics {
    registry: Registry,

    /// Total incidents by status
    pub incidents_total: CounterVec,

    /// Total ingest requests
    pub ingest_requests_total: Counter,

    /// Pipeline stage duration in seconds
    pub pipeline_duration_seconds: HistogramVec,

    /// Total LLM requests by provider and purpose
    pub llm_requests_total: CounterVec,

    /// Total LLM tokens by provider and direction
    pub llm_tokens_total: CounterVec,
}

impl Metrics {
    fn new() -> Self {
        let registry = Registry::new();

        // crispr_incidents_total
        let incidents_total = CounterVec::new(
            Opts::new("crispr_incidents_total", "Total number of incidents by status"),
            &["status"],
        )
        .expect("Failed to create incidents_total metric");
        registry
            .register(Box::new(incidents_total.clone()))
            .expect("Failed to register incidents_total");

        // crispr_ingest_requests_total
        let ingest_requests_total = Counter::new(
            "crispr_ingest_requests_total",
            "Total number of ingest requests",
        )
        .expect("Failed to create ingest_requests_total metric");
        registry
            .register(Box::new(ingest_requests_total.clone()))
            .expect("Failed to register ingest_requests_total");

        // crispr_pipeline_duration_seconds
        let pipeline_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "crispr_pipeline_duration_seconds",
                "Duration of pipeline stages in seconds",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]),
            &["stage"],
        )
        .expect("Failed to create pipeline_duration_seconds metric");
        registry
            .register(Box::new(pipeline_duration_seconds.clone()))
            .expect("Failed to register pipeline_duration_seconds");

        // crispr_llm_requests_total
        let llm_requests_total = CounterVec::new(
            Opts::new("crispr_llm_requests_total", "Total LLM requests by provider and purpose"),
            &["provider", "purpose"],
        )
        .expect("Failed to create llm_requests_total metric");
        registry
            .register(Box::new(llm_requests_total.clone()))
            .expect("Failed to register llm_requests_total");

        // crispr_llm_tokens_total
        let llm_tokens_total = CounterVec::new(
            Opts::new("crispr_llm_tokens_total", "Total LLM tokens by provider and direction"),
            &["provider", "direction"],
        )
        .expect("Failed to create llm_tokens_total metric");
        registry
            .register(Box::new(llm_tokens_total.clone()))
            .expect("Failed to register llm_tokens_total");

        Self {
            registry,
            incidents_total,
            ingest_requests_total,
            pipeline_duration_seconds,
            llm_requests_total,
            llm_tokens_total,
        }
    }

    /// Encode all metrics as Prometheus text format
    pub fn encode(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

/// Get the global metrics instance
pub fn metrics() -> &'static Metrics {
    REGISTRY.get_or_init(Metrics::new)
}

/// Record an incident status
pub fn record_incident(status: &str) {
    metrics().incidents_total.with_label_values(&[status]).inc();
}

/// Record an ingest request
pub fn record_ingest() {
    metrics().ingest_requests_total.inc();
}

/// Record LLM request
pub fn record_llm_request(provider: &str, purpose: &str) {
    metrics()
        .llm_requests_total
        .with_label_values(&[provider, purpose])
        .inc();
}

/// Record LLM tokens
pub fn record_llm_tokens(provider: &str, input_tokens: i32, output_tokens: i32) {
    metrics()
        .llm_tokens_total
        .with_label_values(&[provider, "input"])
        .inc_by(input_tokens as f64);
    metrics()
        .llm_tokens_total
        .with_label_values(&[provider, "output"])
        .inc_by(output_tokens as f64);
}

/// Timer for measuring pipeline stage duration
pub struct StageTimer {
    stage: String,
    start: Instant,
}

impl StageTimer {
    /// Start timing a pipeline stage
    pub fn start(stage: &str) -> Self {
        Self {
            stage: stage.to_string(),
            start: Instant::now(),
        }
    }

    /// Stop timing and record the duration
    pub fn stop(self) {
        let duration = self.start.elapsed().as_secs_f64();
        metrics()
            .pipeline_duration_seconds
            .with_label_values(&[&self.stage])
            .observe(duration);
    }
}

/// Handler for /metrics endpoint
pub async fn handler() -> String {
    metrics().encode()
}
