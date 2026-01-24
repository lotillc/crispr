// OTLP gRPC ingest server
// Receives OpenTelemetry logs via gRPC and converts them to our ingest format

use std::net::SocketAddr;

use opentelemetry_proto::tonic::collector::logs::v1::{
    logs_service_server::{LogsService, LogsServiceServer},
    ExportLogsServiceRequest, ExportLogsServiceResponse,
};
use opentelemetry_proto::tonic::logs::v1::SeverityNumber;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::AppState;
use crate::pipeline::fingerprint;

/// OTLP Log Service implementation
/// Receives logs via gRPC and processes error-level logs as incidents
pub struct OtlpLogService {
    state: AppState,
}

impl OtlpLogService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Extract severity level string from OTLP severity number
    fn severity_to_level(severity: i32) -> &'static str {
        match SeverityNumber::try_from(severity) {
            Ok(SeverityNumber::Fatal)
            | Ok(SeverityNumber::Fatal2)
            | Ok(SeverityNumber::Fatal3)
            | Ok(SeverityNumber::Fatal4) => "FATAL",
            Ok(SeverityNumber::Error)
            | Ok(SeverityNumber::Error2)
            | Ok(SeverityNumber::Error3)
            | Ok(SeverityNumber::Error4) => "ERROR",
            Ok(SeverityNumber::Warn)
            | Ok(SeverityNumber::Warn2)
            | Ok(SeverityNumber::Warn3)
            | Ok(SeverityNumber::Warn4) => "WARN",
            Ok(SeverityNumber::Info)
            | Ok(SeverityNumber::Info2)
            | Ok(SeverityNumber::Info3)
            | Ok(SeverityNumber::Info4) => "INFO",
            Ok(SeverityNumber::Debug)
            | Ok(SeverityNumber::Debug2)
            | Ok(SeverityNumber::Debug3)
            | Ok(SeverityNumber::Debug4) => "DEBUG",
            Ok(SeverityNumber::Trace)
            | Ok(SeverityNumber::Trace2)
            | Ok(SeverityNumber::Trace3)
            | Ok(SeverityNumber::Trace4) => "TRACE",
            _ => "UNKNOWN",
        }
    }

    /// Check if severity indicates an error-level log
    fn is_error_level(severity: i32) -> bool {
        matches!(
            SeverityNumber::try_from(severity),
            Ok(SeverityNumber::Error)
                | Ok(SeverityNumber::Error2)
                | Ok(SeverityNumber::Error3)
                | Ok(SeverityNumber::Error4)
                | Ok(SeverityNumber::Fatal)
                | Ok(SeverityNumber::Fatal2)
                | Ok(SeverityNumber::Fatal3)
                | Ok(SeverityNumber::Fatal4)
        )
    }

    /// Extract string value from OTLP AnyValue
    fn extract_string_value(
        value: &opentelemetry_proto::tonic::common::v1::AnyValue,
    ) -> Option<String> {
        use opentelemetry_proto::tonic::common::v1::any_value::Value;
        match &value.value {
            Some(Value::StringValue(s)) => Some(s.clone()),
            Some(Value::IntValue(i)) => Some(i.to_string()),
            Some(Value::DoubleValue(d)) => Some(d.to_string()),
            Some(Value::BoolValue(b)) => Some(b.to_string()),
            _ => None,
        }
    }

    /// Extract project identifier from resource/log attributes
    /// Looks for common attributes like service.name, deployment.environment, etc.
    fn extract_project(
        resource_attrs: &[opentelemetry_proto::tonic::common::v1::KeyValue],
        log_attrs: &[opentelemetry_proto::tonic::common::v1::KeyValue],
    ) -> Option<String> {
        // First check log attributes for explicit project
        for attr in log_attrs {
            if attr.key == "project" || attr.key == "crispr.project" {
                if let Some(ref v) = attr.value {
                    if let Some(s) = Self::extract_string_value(v) {
                        return Some(s);
                    }
                }
            }
        }

        // Then check resource attributes
        let mut service_name: Option<String> = None;
        let mut service_namespace: Option<String> = None;

        for attr in resource_attrs {
            if let Some(ref v) = attr.value {
                match attr.key.as_str() {
                    "project" | "crispr.project" => {
                        if let Some(s) = Self::extract_string_value(v) {
                            return Some(s);
                        }
                    }
                    "service.name" => {
                        service_name = Self::extract_string_value(v);
                    }
                    "service.namespace" => {
                        service_namespace = Self::extract_string_value(v);
                    }
                    _ => {}
                }
            }
        }

        // Try to construct project from service.namespace/service.name
        match (service_namespace, service_name) {
            (Some(ns), Some(name)) => Some(format!("{}/{}", ns, name)),
            (None, Some(name)) => {
                // Try to parse owner/repo from service name (e.g., "owner-repo" -> "owner/repo")
                if name.contains('/') {
                    Some(name)
                } else if let Some(idx) = name.find('-') {
                    let (owner, repo) = name.split_at(idx);
                    Some(format!("{}/{}", owner, &repo[1..]))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Extract message body from log record
    fn extract_message(log_record: &opentelemetry_proto::tonic::logs::v1::LogRecord) -> String {
        // First try the body field
        if let Some(ref body) = log_record.body {
            if let Some(s) = Self::extract_string_value(body) {
                if !s.is_empty() {
                    return s;
                }
            }
        }

        // Fallback to attributes
        for attr in &log_record.attributes {
            if attr.key == "message" || attr.key == "exception.message" {
                if let Some(ref v) = attr.value {
                    if let Some(s) = Self::extract_string_value(v) {
                        return s;
                    }
                }
            }
        }

        "Unknown error".to_string()
    }

    /// Extract stack trace from log attributes
    fn extract_stack_trace(
        attrs: &[opentelemetry_proto::tonic::common::v1::KeyValue],
    ) -> Option<String> {
        for attr in attrs {
            if attr.key == "exception.stacktrace"
                || attr.key == "stack_trace"
                || attr.key == "stacktrace"
            {
                if let Some(ref v) = attr.value {
                    return Self::extract_string_value(v);
                }
            }
        }
        None
    }

    /// Extract all attributes as a JSON object for storage
    fn extract_attributes(
        resource_attrs: &[opentelemetry_proto::tonic::common::v1::KeyValue],
        log_attrs: &[opentelemetry_proto::tonic::common::v1::KeyValue],
    ) -> serde_json::Value {
        let mut attrs = serde_json::Map::new();

        // Add resource attributes with "resource." prefix
        for attr in resource_attrs {
            if let Some(ref v) = attr.value {
                if let Some(s) = Self::extract_string_value(v) {
                    attrs.insert(
                        format!("resource.{}", attr.key),
                        serde_json::Value::String(s),
                    );
                }
            }
        }

        // Add log attributes
        for attr in log_attrs {
            if let Some(ref v) = attr.value {
                if let Some(s) = Self::extract_string_value(v) {
                    attrs.insert(attr.key.clone(), serde_json::Value::String(s));
                }
            }
        }

        serde_json::Value::Object(attrs)
    }

    /// Process a single log record and create/update incident if it's an error
    async fn process_log_record(
        &self,
        resource_attrs: &[opentelemetry_proto::tonic::common::v1::KeyValue],
        log_record: &opentelemetry_proto::tonic::logs::v1::LogRecord,
    ) -> Result<Option<Uuid>, Status> {
        let severity = log_record.severity_number;

        // Only process error-level logs
        if !Self::is_error_level(severity) {
            return Ok(None);
        }

        let level = Self::severity_to_level(severity);
        let message = Self::extract_message(log_record);
        let stack_trace = Self::extract_stack_trace(&log_record.attributes);
        let attributes = Self::extract_attributes(resource_attrs, &log_record.attributes);

        // Extract project identifier
        let project_key = match Self::extract_project(resource_attrs, &log_record.attributes) {
            Some(p) => p,
            None => {
                debug!("Skipping log record: cannot identify project");
                return Ok(None);
            }
        };

        // Parse owner/repo
        let parts: Vec<&str> = project_key.split('/').collect();
        if parts.len() != 2 {
            debug!(project = %project_key, "Skipping log record: invalid project format");
            return Ok(None);
        }
        let (owner, repo) = (parts[0].to_string(), parts[1].to_string());

        // Check safety rails
        if let Some(allowed) = &self.state.config.safety.allowed_repos {
            if !is_repo_allowed(&owner, &repo, allowed) {
                debug!(project = %project_key, "Skipping log record: repo not in allowed list");
                return Ok(None);
            }
        }

        // Auto-register or fetch project
        let project = sqlx::query_as!(
            crate::store::models::Project,
            r#"
            INSERT INTO projects (id, github_owner, github_repo, auto_registered)
            VALUES ($1, $2, $3, true)
            ON CONFLICT (github_owner, github_repo)
            DO UPDATE SET updated_at = NOW()
            RETURNING id, github_owner, github_repo, default_branch, language,
                      enabled, auto_registered, settings, created_at, updated_at
            "#,
            Uuid::new_v4(),
            owner,
            repo
        )
        .fetch_one(&self.state.db)
        .await
        .map_err(|e| {
            error!(error = %e, "Database error creating project");
            Status::internal("Database error")
        })?;

        if !project.enabled.unwrap_or(true) {
            debug!(project = %project_key, "Skipping log record: project disabled");
            return Ok(None);
        }

        // Build payload for fingerprinting
        let payload_json = serde_json::json!({
            "message": message,
            "stack_trace": stack_trace,
            "project": project_key,
            "level": level,
            "attributes": attributes,
        });

        // Fingerprint the error
        let fp = fingerprint::compute(&payload_json);

        // Upsert incident
        let incident = sqlx::query!(
            r#"
            INSERT INTO incidents (id, project_id, fingerprint, title, state, first_seen, last_seen, occurrence_count)
            VALUES ($1, $2, $3, $4, 'detected', NOW(), NOW(), 1)
            ON CONFLICT (project_id, fingerprint)
            DO UPDATE SET
                occurrence_count = incidents.occurrence_count + 1,
                last_seen = NOW(),
                state = CASE
                    WHEN incidents.state = 'cooldown' AND incidents.cooldown_until < NOW() THEN 'detected'
                    WHEN incidents.state = 'pr_merged' THEN 'detected'
                    WHEN incidents.state = 'pr_closed' THEN 'detected'
                    ELSE incidents.state
                END,
                updated_at = NOW()
            RETURNING id, occurrence_count,
                      (xmax = 0) as "is_new!"
            "#,
            Uuid::new_v4(),
            project.id,
            fp,
            truncate_title(&message),
        )
        .fetch_one(&self.state.db)
        .await
        .map_err(|e| {
            error!(error = %e, "Database error upserting incident");
            Status::internal("Database error")
        })?;

        let is_new = incident.is_new;
        let occurrence_count = incident.occurrence_count.unwrap_or(1);

        // Store sample if this is a new incident or we need more samples (first 3)
        if occurrence_count <= 3 {
            let sample_data = serde_json::to_vec(&payload_json).map_err(|e| {
                error!(error = %e, "JSON serialization error");
                Status::internal("Serialization error")
            })?;

            self.state
                .storage
                .put(incident.id, &sample_data)
                .await
                .map_err(|e| {
                    error!(error = %e, "Storage error");
                    Status::internal("Storage error")
                })?;
        }

        // Log activity
        sqlx::query!(
            r#"
            INSERT INTO activity_log (id, incident_id, project_id, action, details)
            VALUES ($1, $2, $3, 'ingested', $4)
            "#,
            Uuid::new_v4(),
            incident.id,
            project.id,
            serde_json::json!({
                "occurrence": occurrence_count,
                "level": level,
                "is_new": is_new,
                "source": "otlp_grpc",
            }),
        )
        .execute(&self.state.db)
        .await
        .map_err(|e| {
            error!(error = %e, "Database error logging activity");
            Status::internal("Database error")
        })?;

        if is_new {
            info!(
                incident_id = %incident.id,
                project = %project_key,
                fingerprint = %fp,
                source = "otlp_grpc",
                "New incident created via OTLP"
            );

            // Broadcast incident:created event via SSE
            self.state.events.incident_created(
                incident.id,
                project.id,
                &truncate_title(&message),
                "detected",
            );

            // Broadcast updated stats
            super::events::broadcast_stats(&self.state).await;
        } else {
            debug!(
                incident_id = %incident.id,
                project = %project_key,
                occurrence_count = occurrence_count,
                "Incident occurrence recorded via OTLP"
            );
        }

        Ok(Some(incident.id))
    }
}

#[tonic::async_trait]
impl LogsService for OtlpLogService {
    async fn export(
        &self,
        request: Request<ExportLogsServiceRequest>,
    ) -> Result<Response<ExportLogsServiceResponse>, Status> {
        let req = request.into_inner();

        let mut processed_count = 0u64;
        let mut error_count = 0u64;
        let mut incident_count = 0u64;

        // Process each resource logs
        for resource_logs in &req.resource_logs {
            // Extract resource attributes
            let resource_attrs = resource_logs
                .resource
                .as_ref()
                .map(|r| r.attributes.as_slice())
                .unwrap_or(&[]);

            // Process each scope logs
            for scope_logs in &resource_logs.scope_logs {
                // Process each log record
                for log_record in &scope_logs.log_records {
                    processed_count += 1;

                    match self.process_log_record(resource_attrs, log_record).await {
                        Ok(Some(_)) => {
                            incident_count += 1;
                        }
                        Ok(None) => {
                            // Log was filtered out (not an error or missing project)
                        }
                        Err(e) => {
                            error_count += 1;
                            warn!(error = %e, "Failed to process log record");
                        }
                    }
                }
            }
        }

        debug!(
            processed = processed_count,
            incidents = incident_count,
            errors = error_count,
            "OTLP export request processed"
        );

        // Record metrics
        super::metrics::record_grpc_ingest(processed_count, incident_count);

        Ok(Response::new(ExportLogsServiceResponse {
            partial_success: None,
        }))
    }
}

/// Check if repo is in allowed list (supports wildcards)
fn is_repo_allowed(owner: &str, repo: &str, allowed: &[String]) -> bool {
    let full = format!("{}/{}", owner, repo);
    allowed.iter().any(|pattern| {
        if pattern.contains('*') {
            let pattern = pattern.replace('.', r"\.");
            let pattern = pattern.replace('*', ".*");
            regex::Regex::new(&format!("^{}$", pattern))
                .map(|re| re.is_match(&full))
                .unwrap_or(false)
        } else {
            pattern == &full
        }
    })
}

/// Truncate message to create a title (max 200 chars)
fn truncate_title(msg: &str) -> String {
    let first_line = msg.lines().next().unwrap_or(msg);
    if first_line.len() > 200 {
        format!("{}...", &first_line[..197])
    } else {
        first_line.to_string()
    }
}

/// Start the gRPC server for OTLP log ingestion
pub async fn serve_grpc(state: AppState, addr: &str) -> Result<(), anyhow::Error> {
    let addr: SocketAddr = addr.parse()?;

    let service = OtlpLogService::new(state);

    info!(addr = %addr, "gRPC server starting (OTLP LogsService)");

    Server::builder()
        .add_service(LogsServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
