//! API Contract Detection Module
//!
//! Detects when fixes modify API contract files and analyzes for breaking changes.
//! API contracts include OpenAPI specs, Protocol Buffers, GraphQL schemas, routes, etc.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::integrations::llm::TrackedLlm;

/// File patterns that typically define API contracts
const API_FILE_PATTERNS: &[&str] = &[
    // OpenAPI / Swagger
    "openapi.yaml",
    "openapi.json",
    "openapi.yml",
    "swagger.yaml",
    "swagger.json",
    "swagger.yml",
    // Protocol Buffers
    "*.proto",
    // GraphQL
    "schema.graphql",
    "*.graphql",
    "schema.gql",
    // Route definitions
    "**/routes/*",
    "**/routes.*",
    "**/api/**/routes.*",
    "**/router.*",
    // API controllers
    "**/controllers/*",
    "**/api/**/*Controller.*",
    "**/api/**/*controller.*",
    // Schema definitions
    "**/schema/*",
    "**/schemas/*",
    // TypeScript/JavaScript API types
    "**/api/types.*",
    "**/types/api.*",
    // REST API definitions
    "**/endpoints/*",
    "**/api/**/index.*",
];

/// Additional patterns based on common conventions
const API_KEYWORDS_IN_PATH: &[&str] = &[
    "/api/",
    "/routes/",
    "/endpoints/",
    "/controllers/",
    "/handlers/",
    "/graphql/",
    "/grpc/",
    "/rpc/",
    "/proto/",
];

/// Result of breaking change analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingChangeAnalysis {
    pub is_breaking: bool,
    pub confidence: f32,
    pub changes: Vec<ApiChange>,
    pub summary: String,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiChange {
    pub change_type: ApiChangeType,
    pub path: Option<String>,
    pub field: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub is_breaking: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiChangeType {
    EndpointRemoved,
    EndpointRenamed,
    ParameterRemoved,
    ParameterRequired,
    TypeChanged,
    ResponseChanged,
    MethodChanged,
    FieldRemoved,
    FieldRenamed,
    EnumValueRemoved,
    SchemaChanged,
    Other,
}

/// Detect the type of API contract file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFileType {
    OpenApi,
    Protobuf,
    GraphQL,
    Routes,
    Schema,
    Controller,
    Types,
    Other,
}

impl ApiFileType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenApi => "openapi",
            Self::Protobuf => "protobuf",
            Self::GraphQL => "graphql",
            Self::Routes => "routes",
            Self::Schema => "schema",
            Self::Controller => "controller",
            Self::Types => "types",
            Self::Other => "other",
        }
    }
}

/// Check if a file path matches API contract patterns
pub fn is_api_contract_file(path: &str) -> bool {
    let path_lower = path.to_lowercase();

    // Check for keyword patterns in path
    for keyword in API_KEYWORDS_IN_PATH {
        if path_lower.contains(keyword) {
            return true;
        }
    }

    // Check for specific file patterns
    let filename = path.split('/').last().unwrap_or(path).to_lowercase();

    // OpenAPI / Swagger
    if filename.starts_with("openapi") || filename.starts_with("swagger") {
        return true;
    }

    // Protocol Buffers
    if filename.ends_with(".proto") {
        return true;
    }

    // GraphQL
    if filename.ends_with(".graphql") || filename.ends_with(".gql") {
        return true;
    }

    // Schema files
    if filename.contains("schema") {
        return true;
    }

    // Route definitions
    if filename.contains("route") || filename.contains("router") {
        return true;
    }

    // Controller files
    if filename.contains("controller") {
        return true;
    }

    false
}

/// Detect the type of API file from its path
pub fn detect_api_file_type(path: &str) -> Option<ApiFileType> {
    if !is_api_contract_file(path) {
        return None;
    }

    let path_lower = path.to_lowercase();
    let filename = path.split('/').last().unwrap_or(path).to_lowercase();

    if filename.starts_with("openapi") || filename.starts_with("swagger") {
        Some(ApiFileType::OpenApi)
    } else if filename.ends_with(".proto") {
        Some(ApiFileType::Protobuf)
    } else if filename.ends_with(".graphql") || filename.ends_with(".gql") {
        Some(ApiFileType::GraphQL)
    } else if path_lower.contains("/routes") || filename.contains("route") {
        Some(ApiFileType::Routes)
    } else if path_lower.contains("/schema") || filename.contains("schema") {
        Some(ApiFileType::Schema)
    } else if path_lower.contains("/controller") || filename.contains("controller") {
        Some(ApiFileType::Controller)
    } else if filename.contains("types") || path_lower.contains("/types/") {
        Some(ApiFileType::Types)
    } else {
        Some(ApiFileType::Other)
    }
}

/// Analyze a diff for breaking API changes
pub async fn analyze_breaking_changes(
    llm: &TrackedLlm,
    diff: &str,
    file_path: &str,
    incident_id: Option<uuid::Uuid>,
    project_id: Option<uuid::Uuid>,
) -> Result<BreakingChangeAnalysis> {
    let file_type = detect_api_file_type(file_path).unwrap_or(ApiFileType::Other);

    let system_prompt = r#"You are an API compatibility expert analyzing code changes for breaking changes.

A breaking change is any modification that could cause existing API consumers to fail:
- Removing endpoints, fields, or enum values
- Changing parameter types or making optional params required
- Renaming endpoints, fields, or methods
- Changing response structures
- Modifying authentication requirements

Non-breaking changes include:
- Adding new optional fields or endpoints
- Adding new enum values (in most cases)
- Deprecation notices without removal
- Documentation changes
- Internal refactoring that doesn't affect the API surface

Consider the file type when analyzing - different API definition formats have different conventions.

Respond in JSON format only."#;

    let prompt = format!(
        r#"Analyze this diff for breaking API changes.

File: {}
File Type: {}

Diff:
```
{}
```

Respond with a JSON object containing:
- "is_breaking": boolean - are there any breaking changes?
- "confidence": number 0-1 - how confident in this assessment
- "changes": array of change objects, each with:
  - "change_type": string - endpoint_removed, endpoint_renamed, parameter_removed, parameter_required, type_changed, response_changed, method_changed, field_removed, field_renamed, enum_value_removed, schema_changed, other
  - "path": string or null - API path/endpoint affected
  - "field": string or null - field name if applicable
  - "old_value": string or null - previous value
  - "new_value": string or null - new value
  - "is_breaking": boolean - is this specific change breaking?
  - "description": string - description of the change
- "summary": string - overall summary of the changes
- "recommendations": array of strings - suggestions for handling these changes

JSON response:"#,
        file_path,
        file_type.as_str(),
        diff
    );

    let response = llm
        .complete(
            &prompt,
            Some(system_prompt),
            "api_contract_analysis",
            incident_id,
            project_id,
        )
        .await?;

    parse_breaking_change_response(&response.content)
}

/// Parse the LLM response for breaking change analysis
fn parse_breaking_change_response(content: &str) -> Result<BreakingChangeAnalysis> {
    let json_str = if content.contains("```json") {
        content
            .split("```json")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .unwrap_or(content)
            .trim()
    } else if content.contains("```") {
        content
            .split("```")
            .nth(1)
            .unwrap_or(content)
            .trim()
    } else {
        content.trim()
    };

    match serde_json::from_str(json_str) {
        Ok(result) => Ok(result),
        Err(e) => {
            if let Some(start) = json_str.find('{') {
                if let Some(end) = json_str.rfind('}') {
                    let json_obj = &json_str[start..=end];
                    return Ok(serde_json::from_str(json_obj)?);
                }
            }
            // Return a safe default if parsing fails
            Ok(BreakingChangeAnalysis {
                is_breaking: false,
                confidence: 0.0,
                changes: vec![],
                summary: format!("Failed to analyze: {}", e),
                recommendations: vec!["Manual review recommended".to_string()],
            })
        }
    }
}

/// Filter changed files to find API contract files
pub fn filter_api_contract_files(changed_files: &[String]) -> Vec<(String, ApiFileType)> {
    changed_files
        .iter()
        .filter_map(|path| {
            detect_api_file_type(path).map(|file_type| (path.clone(), file_type))
        })
        .collect()
}

/// Quick check if any files in a list are API contracts (without full analysis)
pub fn has_api_contract_files(changed_files: &[String]) -> bool {
    changed_files.iter().any(|f| is_api_contract_file(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_api_contract_file() {
        // Should match
        assert!(is_api_contract_file("openapi.yaml"));
        assert!(is_api_contract_file("swagger.json"));
        assert!(is_api_contract_file("src/api/routes/users.ts"));
        assert!(is_api_contract_file("proto/service.proto"));
        assert!(is_api_contract_file("schema.graphql"));
        assert!(is_api_contract_file("src/controllers/userController.js"));

        // Should not match
        assert!(!is_api_contract_file("src/utils/helpers.ts"));
        assert!(!is_api_contract_file("README.md"));
        assert!(!is_api_contract_file("package.json"));
    }

    #[test]
    fn test_detect_api_file_type() {
        assert_eq!(
            detect_api_file_type("openapi.yaml"),
            Some(ApiFileType::OpenApi)
        );
        assert_eq!(
            detect_api_file_type("service.proto"),
            Some(ApiFileType::Protobuf)
        );
        assert_eq!(
            detect_api_file_type("schema.graphql"),
            Some(ApiFileType::GraphQL)
        );
        assert_eq!(
            detect_api_file_type("src/routes/api.ts"),
            Some(ApiFileType::Routes)
        );
        assert_eq!(detect_api_file_type("src/utils.ts"), None);
    }

    #[test]
    fn test_filter_api_contract_files() {
        let files = vec![
            "src/utils.ts".to_string(),
            "openapi.yaml".to_string(),
            "src/routes/users.ts".to_string(),
            "README.md".to_string(),
        ];

        let api_files = filter_api_contract_files(&files);
        assert_eq!(api_files.len(), 2);
        assert!(api_files.iter().any(|(p, _)| p == "openapi.yaml"));
        assert!(api_files.iter().any(|(p, _)| p == "src/routes/users.ts"));
    }
}
