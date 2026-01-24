//! Mock implementations for external services
//!
//! Provides mock versions of:
//! - LLM providers (for triage and fix generation)
//! - GitHub client (for repository operations)
//! - Slack client (for notifications)

use std::collections::HashMap;

use anyhow::Result;
use serde_json::json;
use tokio::sync::RwLock;

/// Mock LLM Manager
///
/// Provides canned responses for triage and fix generation.
/// Can be configured to return specific responses for testing different scenarios.
pub struct MockLlmManager {
    /// Triage responses keyed by error message substring
    triage_responses: RwLock<HashMap<String, MockTriageResponse>>,
    /// Fix responses keyed by error category
    fix_responses: RwLock<HashMap<String, MockFixResponse>>,
    /// Default triage response
    default_triage: MockTriageResponse,
    /// Default fix response
    default_fix: MockFixResponse,
    /// Track all requests made
    requests: RwLock<Vec<MockLlmRequest>>,
}

#[derive(Debug, Clone)]
pub struct MockLlmRequest {
    pub role: String,
    pub operation: String,
    pub prompt_preview: String,
}

#[derive(Debug, Clone)]
pub struct MockTriageResponse {
    pub fixable: bool,
    pub confidence: f32,
    pub reason: String,
    pub category: String,
    pub suggested_files: Vec<String>,
    pub description: String,
    pub fix_approach: Option<String>,
}

impl Default for MockTriageResponse {
    fn default() -> Self {
        Self {
            fixable: true,
            confidence: 0.85,
            reason: "This appears to be a null reference error that can be fixed with a null check".to_string(),
            category: "null_reference".to_string(),
            suggested_files: vec!["src/handlers/user.ts".to_string()],
            description: "Null reference when accessing user property".to_string(),
            fix_approach: Some("Add null check before accessing the property".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MockFixResponse {
    pub changes: Vec<MockFileChange>,
    pub explanation: String,
    pub confidence: f32,
    pub risks: Vec<String>,
    pub suggested_tests: Vec<String>,
    pub generated_tests: Vec<MockGeneratedTest>,
}

#[derive(Debug, Clone)]
pub struct MockFileChange {
    pub path: String,
    pub content: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct MockGeneratedTest {
    pub path: String,
    pub content: String,
    pub description: String,
    pub framework: String,
}

impl Default for MockFixResponse {
    fn default() -> Self {
        Self {
            changes: vec![MockFileChange {
                path: "src/handlers/user.ts".to_string(),
                content: r#"export function getUser(id: string): User | null {
  const user = users.get(id);
  if (!user) {
    return null;
  }
  return user;
}"#
                .to_string(),
                description: "Added null check for user lookup".to_string(),
            }],
            explanation: "Added null check to prevent TypeError when user is not found".to_string(),
            confidence: 0.9,
            risks: vec!["Callers may need to handle null return value".to_string()],
            suggested_tests: vec!["npm test".to_string()],
            generated_tests: vec![MockGeneratedTest {
                path: "src/handlers/user.test.ts".to_string(),
                content: r#"import { getUser } from './user';

describe('getUser', () => {
  it('returns null for non-existent user', () => {
    expect(getUser('nonexistent')).toBeNull();
  });

  it('returns user when found', () => {
    const user = getUser('existing-id');
    expect(user).not.toBeNull();
  });
});"#
                    .to_string(),
                description: "Test null handling in getUser function".to_string(),
                framework: "jest".to_string(),
            }],
        }
    }
}

impl MockLlmManager {
    pub fn new() -> Self {
        Self {
            triage_responses: RwLock::new(HashMap::new()),
            fix_responses: RwLock::new(HashMap::new()),
            default_triage: MockTriageResponse::default(),
            default_fix: MockFixResponse::default(),
            requests: RwLock::new(Vec::new()),
        }
    }

    /// Configure a specific triage response for errors matching a pattern
    pub async fn set_triage_response(&self, pattern: &str, response: MockTriageResponse) {
        self.triage_responses.write().await.insert(pattern.to_string(), response);
    }

    /// Configure a specific fix response for a category
    pub async fn set_fix_response(&self, category: &str, response: MockFixResponse) {
        self.fix_responses.write().await.insert(category.to_string(), response);
    }

    /// Set default triage to unfixable
    pub fn with_unfixable_triage(mut self) -> Self {
        self.default_triage = MockTriageResponse {
            fixable: false,
            confidence: 0.9,
            reason: "This is a configuration error that cannot be fixed with code changes".to_string(),
            category: "configuration".to_string(),
            suggested_files: vec![],
            description: "Configuration error".to_string(),
            fix_approach: None,
        };
        self
    }

    /// Set default fix to fail
    pub fn with_failed_fix(mut self) -> Self {
        self.default_fix = MockFixResponse {
            changes: vec![],
            explanation: "Unable to generate a fix for this error".to_string(),
            confidence: 0.2,
            risks: vec!["No fix could be generated".to_string()],
            suggested_tests: vec![],
            generated_tests: vec![],
        };
        self
    }

    /// Get triage response as JSON (simulating LLM output)
    pub async fn get_triage_response(&self, error_message: &str) -> String {
        // Record the request
        self.requests.write().await.push(MockLlmRequest {
            role: "reader".to_string(),
            operation: "triage".to_string(),
            prompt_preview: error_message.chars().take(100).collect(),
        });

        // Find matching response or use default
        let responses = self.triage_responses.read().await;
        let response = responses
            .iter()
            .find(|(pattern, _)| error_message.contains(pattern.as_str()))
            .map(|(_, r)| r.clone())
            .unwrap_or_else(|| self.default_triage.clone());

        json!({
            "fixable": response.fixable,
            "confidence": response.confidence,
            "reason": response.reason,
            "category": response.category,
            "suggested_files": response.suggested_files,
            "description": response.description,
            "fix_approach": response.fix_approach
        })
        .to_string()
    }

    /// Get fix response as JSON (simulating LLM output)
    pub async fn get_fix_response(&self, category: &str) -> String {
        // Record the request
        self.requests.write().await.push(MockLlmRequest {
            role: "writer".to_string(),
            operation: "fix".to_string(),
            prompt_preview: category.to_string(),
        });

        // Find matching response or use default
        let responses = self.fix_responses.read().await;
        let response = responses
            .get(category)
            .cloned()
            .unwrap_or_else(|| self.default_fix.clone());

        let changes: Vec<serde_json::Value> = response
            .changes
            .iter()
            .map(|c| {
                json!({
                    "path": c.path,
                    "content": c.content,
                    "description": c.description
                })
            })
            .collect();

        let tests: Vec<serde_json::Value> = response
            .generated_tests
            .iter()
            .map(|t| {
                json!({
                    "path": t.path,
                    "content": t.content,
                    "description": t.description,
                    "framework": t.framework
                })
            })
            .collect();

        json!({
            "changes": changes,
            "explanation": response.explanation,
            "confidence": response.confidence,
            "risks": response.risks,
            "suggested_tests": response.suggested_tests,
            "generated_tests": tests
        })
        .to_string()
    }

    /// Get all recorded requests
    pub async fn get_requests(&self) -> Vec<MockLlmRequest> {
        self.requests.read().await.clone()
    }

    /// Clear recorded requests
    pub async fn clear_requests(&self) {
        self.requests.write().await.clear();
    }

    /// Check if LLM is "configured"
    pub fn is_configured(&self) -> bool {
        true
    }
}

/// Mock GitHub Client
///
/// Simulates GitHub API responses for repository operations.
pub struct MockGitHubClient {
    /// File contents keyed by "owner/repo/path"
    files: RwLock<HashMap<String, MockFileContent>>,
    /// Whether the client is connected
    connected: RwLock<bool>,
    /// Created branches
    branches: RwLock<Vec<MockBranch>>,
    /// Created PRs
    pull_requests: RwLock<Vec<MockPullRequest>>,
    /// Track API calls
    api_calls: RwLock<Vec<MockGitHubApiCall>>,
}

#[derive(Debug, Clone)]
pub struct MockFileContent {
    pub content: String,
    pub sha: String,
}

#[derive(Debug, Clone)]
pub struct MockBranch {
    pub owner: String,
    pub repo: String,
    pub name: String,
    pub sha: String,
}

#[derive(Debug, Clone)]
pub struct MockPullRequest {
    pub id: i64,
    pub number: i32,
    pub owner: String,
    pub repo: String,
    pub title: String,
    pub body: String,
    pub head: String,
    pub base: String,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct MockGitHubApiCall {
    pub method: String,
    pub endpoint: String,
}

impl MockGitHubClient {
    pub fn new() -> Self {
        Self {
            files: RwLock::new(HashMap::new()),
            connected: RwLock::new(true),
            branches: RwLock::new(Vec::new()),
            pull_requests: RwLock::new(Vec::new()),
            api_calls: RwLock::new(Vec::new()),
        }
    }

    /// Set connection status
    pub async fn set_connected(&self, connected: bool) {
        *self.connected.write().await = connected;
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }

    /// Add a file to the mock repository
    pub async fn add_file(&self, owner: &str, repo: &str, path: &str, content: &str) {
        let key = format!("{}/{}/{}", owner, repo, path);
        let sha = format!("{:x}", md5::compute(content.as_bytes()));
        self.files.write().await.insert(
            key,
            MockFileContent {
                content: content.to_string(),
                sha: sha[..7].to_string(),
            },
        );
    }

    /// Get a file from the mock repository
    pub async fn get_file(&self, owner: &str, repo: &str, path: &str) -> Option<MockFileContent> {
        self.api_calls.write().await.push(MockGitHubApiCall {
            method: "GET".to_string(),
            endpoint: format!("/repos/{}/{}/contents/{}", owner, repo, path),
        });

        let key = format!("{}/{}/{}", owner, repo, path);
        self.files.read().await.get(&key).cloned()
    }

    /// Create a branch
    pub async fn create_branch(
        &self,
        owner: &str,
        repo: &str,
        name: &str,
        from_sha: &str,
    ) -> Result<MockBranch> {
        self.api_calls.write().await.push(MockGitHubApiCall {
            method: "POST".to_string(),
            endpoint: format!("/repos/{}/{}/git/refs", owner, repo),
        });

        let branch = MockBranch {
            owner: owner.to_string(),
            repo: repo.to_string(),
            name: name.to_string(),
            sha: from_sha.to_string(),
        };
        self.branches.write().await.push(branch.clone());
        Ok(branch)
    }

    /// Create a pull request
    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<MockPullRequest> {
        self.api_calls.write().await.push(MockGitHubApiCall {
            method: "POST".to_string(),
            endpoint: format!("/repos/{}/{}/pulls", owner, repo),
        });

        let mut prs = self.pull_requests.write().await;
        let number = (prs.len() + 1) as i32;
        let pr = MockPullRequest {
            id: number as i64,
            number,
            owner: owner.to_string(),
            repo: repo.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            head: head.to_string(),
            base: base.to_string(),
            state: "open".to_string(),
        };
        prs.push(pr.clone());
        Ok(pr)
    }

    /// Get all created PRs
    pub async fn get_pull_requests(&self) -> Vec<MockPullRequest> {
        self.pull_requests.read().await.clone()
    }

    /// Get all API calls
    pub async fn get_api_calls(&self) -> Vec<MockGitHubApiCall> {
        self.api_calls.read().await.clone()
    }

    /// Clear API calls
    pub async fn clear_api_calls(&self) {
        self.api_calls.write().await.clear();
    }
}

/// Mock Slack Client
pub struct MockSlackClient {
    /// Messages sent
    messages: RwLock<Vec<MockSlackMessage>>,
    /// Whether connected
    connected: RwLock<bool>,
    /// Configured channel
    channel: RwLock<Option<String>>,
}

#[derive(Debug, Clone)]
pub struct MockSlackMessage {
    pub channel: String,
    pub text: String,
    pub blocks: Option<serde_json::Value>,
}

impl MockSlackClient {
    pub fn new() -> Self {
        Self {
            messages: RwLock::new(Vec::new()),
            connected: RwLock::new(false),
            channel: RwLock::new(None),
        }
    }

    /// Set connection status
    pub async fn set_connected(&self, connected: bool, channel: Option<&str>) {
        *self.connected.write().await = connected;
        *self.channel.write().await = channel.map(String::from);
    }

    /// Send a message (mock)
    pub async fn send_message(&self, channel: &str, text: &str) -> Result<()> {
        self.messages.write().await.push(MockSlackMessage {
            channel: channel.to_string(),
            text: text.to_string(),
            blocks: None,
        });
        Ok(())
    }

    /// Get all sent messages
    pub async fn get_messages(&self) -> Vec<MockSlackMessage> {
        self.messages.read().await.clone()
    }

    /// Clear messages
    pub async fn clear_messages(&self) {
        self.messages.write().await.clear();
    }
}

// Simple MD5 implementation for generating mock SHAs
mod md5 {
    use std::fmt;

    pub struct Digest([u8; 16]);

    impl fmt::LowerHex for Digest {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for byte in &self.0 {
                write!(f, "{:02x}", byte)?;
            }
            Ok(())
        }
    }

    pub fn compute(data: &[u8]) -> Digest {
        // Simple hash for testing - not cryptographically secure
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        let hash = hasher.finish();

        let mut result = [0u8; 16];
        result[..8].copy_from_slice(&hash.to_le_bytes());
        result[8..].copy_from_slice(&hash.to_be_bytes());
        Digest(result)
    }
}
