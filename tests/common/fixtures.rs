//! Test fixtures for CRISPR integration tests
//!
//! Provides sample data for testing various scenarios.

use serde_json::{json, Value};
use uuid::Uuid;

/// Sample error payloads for different error types
pub struct ErrorFixtures;

impl ErrorFixtures {
    /// Null reference error in TypeScript
    pub fn typescript_null_reference() -> Value {
        json!({
            "message": "TypeError: Cannot read property 'userId' of undefined",
            "stack_trace": r#"TypeError: Cannot read property 'userId' of undefined
    at UserController.getProfile (src/controllers/user.ts:45:23)
    at Router.handle (/app/node_modules/express/lib/router/index.js:174:12)
    at processTicksAndRejections (internal/process/task_queues.js:93:5)"#,
            "project": "acme/webapp",
            "level": "ERROR",
            "attributes": {
                "request_id": "req-12345",
                "user_agent": "Mozilla/5.0"
            }
        })
    }

    /// Type error in Python
    pub fn python_type_error() -> Value {
        json!({
            "message": "TypeError: unsupported operand type(s) for +: 'NoneType' and 'str'",
            "stack_trace": r#"Traceback (most recent call last):
  File "/app/handlers/user.py", line 42, in get_user
    return user_service.fetch(user_id)
  File "/app/services/user_service.py", line 15, in fetch
    full_name = user.first_name + " " + user.last_name
TypeError: unsupported operand type(s) for +: 'NoneType' and 'str'"#,
            "project": "acme/backend",
            "level": "ERROR"
        })
    }

    /// Array index out of bounds in Java
    pub fn java_array_index() -> Value {
        json!({
            "message": "java.lang.ArrayIndexOutOfBoundsException: Index 5 out of bounds for length 3",
            "stack_trace": r#"java.lang.ArrayIndexOutOfBoundsException: Index 5 out of bounds for length 3
    at com.example.service.UserService.getUser(UserService.java:42)
    at com.example.controller.UserController.handle(UserController.java:28)
    at org.springframework.web.servlet.FrameworkServlet.service(FrameworkServlet.java:897)"#,
            "project": "acme/java-service",
            "level": "ERROR"
        })
    }

    /// Import error in JavaScript
    pub fn javascript_import_error() -> Value {
        json!({
            "message": "Error: Cannot find module './utils/helpers'",
            "stack_trace": r#"Error: Cannot find module './utils/helpers'
    at require (internal/modules/cjs/loader.js:834:27)
    at Object.<anonymous> (src/handlers/user.js:3:18)
    at Module._compile (internal/modules/cjs/loader.js:945:30)"#,
            "project": "acme/node-app",
            "level": "ERROR"
        })
    }

    /// Configuration error (unfixable)
    pub fn configuration_error() -> Value {
        json!({
            "message": "Error: DATABASE_URL environment variable not set",
            "stack_trace": r#"Error: DATABASE_URL environment variable not set
    at validateConfig (src/config.js:15:11)
    at Object.<anonymous> (src/index.js:8:1)"#,
            "project": "acme/webapp",
            "level": "FATAL",
            "attributes": {
                "environment": "production"
            }
        })
    }

    /// External service error (unfixable)
    pub fn external_service_error() -> Value {
        json!({
            "message": "Error: Connection refused to database at postgres://db:5432",
            "stack_trace": r#"Error: Connection refused
    at TCPConnectWrap.afterConnect [as oncomplete] (net.js:1141:16)"#,
            "project": "acme/webapp",
            "level": "ERROR"
        })
    }

    /// Go panic
    pub fn go_panic() -> Value {
        json!({
            "message": "panic: runtime error: index out of range [3] with length 2",
            "stack_trace": r#"panic: runtime error: index out of range [3] with length 2

goroutine 1 [running]:
main.handleRequest(0xc0000b4000, 0x1)
    /app/handlers/user.go:45 +0x1a5
main.main()
    /app/main.go:23 +0x2e"#,
            "project": "acme/go-service",
            "level": "ERROR"
        })
    }

    /// Rust panic
    pub fn rust_panic() -> Value {
        json!({
            "message": "thread 'main' panicked at 'called `Option::unwrap()` on a `None` value', src/handlers/user.rs:42:5",
            "stack_trace": r#"thread 'main' panicked at src/handlers/user.rs:42:5:
called `Option::unwrap()` on a `None` value
stack backtrace:
   0: src/main.rs:15:10"#,
            "project": "acme/rust-service",
            "level": "ERROR"
        })
    }
}

/// Sample Sentry webhook payloads
pub struct SentryFixtures;

impl SentryFixtures {
    /// Issue created webhook
    pub fn issue_created() -> Value {
        json!({
            "action": "created",
            "actor": {
                "id": "user-123",
                "name": "Test User",
                "type": "user"
            },
            "data": {
                "issue": {
                    "id": "12345",
                    "title": "NullPointerException in UserService",
                    "culprit": "com.example.UserService.getUser",
                    "level": "error",
                    "status": "unresolved",
                    "platform": "java",
                    "project": {
                        "id": "1",
                        "name": "My Project",
                        "slug": "my-project"
                    },
                    "metadata": {
                        "type": "NullPointerException",
                        "value": "User not found",
                        "filename": "UserService.java",
                        "function": "getUser"
                    },
                    "count": 5
                }
            },
            "installation": {
                "uuid": "install-123"
            }
        })
    }

    /// Event with full stack trace
    pub fn event_with_stacktrace() -> Value {
        json!({
            "action": "triggered",
            "data": {
                "event": {
                    "event_id": "evt-12345",
                    "title": "TypeError: Cannot read property 'name' of undefined",
                    "message": "Cannot read property 'name' of undefined",
                    "level": "error",
                    "platform": "javascript",
                    "timestamp": 1700000000.123,
                    "tags": [
                        {"key": "browser", "value": "Chrome"},
                        {"key": "environment", "value": "production"}
                    ],
                    "exception": {
                        "values": [{
                            "type": "TypeError",
                            "value": "Cannot read property 'name' of undefined",
                            "stacktrace": {
                                "frames": [
                                    {
                                        "filename": "src/components/UserCard.tsx",
                                        "function": "UserCard",
                                        "lineno": 25,
                                        "colno": 18,
                                        "in_app": true,
                                        "context_line": "  const name = user.name.toUpperCase();"
                                    },
                                    {
                                        "filename": "src/pages/Profile.tsx",
                                        "function": "Profile",
                                        "lineno": 42,
                                        "colno": 10,
                                        "in_app": true
                                    }
                                ]
                            }
                        }]
                    }
                },
                "issue": {
                    "id": "12346",
                    "title": "TypeError: Cannot read property 'name' of undefined",
                    "project": {
                        "slug": "frontend-app"
                    }
                }
            }
        })
    }

    /// Warning level event (should be ignored)
    pub fn warning_event() -> Value {
        json!({
            "action": "created",
            "data": {
                "issue": {
                    "id": "99999",
                    "title": "Deprecation warning",
                    "level": "warning",
                    "project": {
                        "slug": "my-project"
                    }
                }
            }
        })
    }
}

/// Sample Datadog webhook payloads
pub struct DatadogFixtures;

impl DatadogFixtures {
    /// Error event
    pub fn error_event() -> Value {
        json!({
            "id": "event-123",
            "title": "High error rate on API endpoint /users",
            "text": "Error rate exceeded 5% threshold\n\nStack trace:\n  at handleRequest (src/api/users.ts:42:10)",
            "alert_type": "error",
            "date": 1700000000,
            "priority": "normal",
            "host": "api-server-1",
            "source_type_name": "monitor",
            "tags": [
                "project:acme/api",
                "env:production",
                "service:user-api"
            ],
            "aggregation_key": "api-errors",
            "link": "https://app.datadoghq.com/event/123"
        })
    }

    /// Monitor alert
    pub fn monitor_alert() -> Value {
        json!({
            "id": "monitor-alert-456",
            "title": "CPU usage critical on api-server-1",
            "alert_type": "error",
            "alert_status": "Alert",
            "date": 1700000000,
            "host": "api-server-1",
            "monitor": {
                "id": 123456,
                "name": "High CPU Monitor",
                "type": "metric alert",
                "query": "avg(last_5m):avg:system.cpu.user{host:api-server-1} > 90",
                "message": "CPU usage is critically high"
            },
            "tags": ["project:acme/infrastructure"]
        })
    }

    /// Log-based alert
    pub fn log_alert() -> Value {
        json!({
            "id": "log-alert-789",
            "title": "Error logs detected",
            "alert_type": "error",
            "date": 1700000000,
            "tags": ["service:payment-service", "repo:acme/payments"],
            "logs": [
                {
                    "id": "log-1",
                    "message": "Payment processing failed: Invalid card number",
                    "status": "error",
                    "timestamp": "2024-01-15T10:30:00Z",
                    "host": "payment-1",
                    "service": "payment-service",
                    "attributes": {
                        "transaction_id": "txn-123",
                        "error_code": "INVALID_CARD"
                    }
                }
            ]
        })
    }

    /// Info event (should be ignored)
    pub fn info_event() -> Value {
        json!({
            "id": "info-event",
            "title": "Deployment completed",
            "alert_type": "info",
            "date": 1700000000,
            "tags": ["deployment", "project:acme/api"]
        })
    }
}

/// Sample source code files for context testing
pub struct SourceCodeFixtures;

impl SourceCodeFixtures {
    /// TypeScript user handler with a bug
    pub fn typescript_user_handler() -> &'static str {
        r#"import { User } from '../models/user';
import { UserService } from '../services/user';

export class UserController {
  private userService: UserService;

  constructor() {
    this.userService = new UserService();
  }

  async getProfile(req: Request, res: Response) {
    const userId = req.params.id;
    const user = await this.userService.findById(userId);

    // BUG: user might be null
    const profile = {
      name: user.name,
      email: user.email,
      createdAt: user.createdAt
    };

    res.json(profile);
  }
}
"#
    }

    /// TypeScript user model
    pub fn typescript_user_model() -> &'static str {
        r#"export interface User {
  id: string;
  name: string;
  email: string;
  createdAt: Date;
  updatedAt: Date;
}
"#
    }

    /// Python user service with a bug
    pub fn python_user_service() -> &'static str {
        r#"from dataclasses import dataclass
from typing import Optional

@dataclass
class User:
    id: str
    first_name: Optional[str]
    last_name: Optional[str]
    email: str

class UserService:
    def __init__(self, repo):
        self.repo = repo

    def get_full_name(self, user_id: str) -> str:
        user = self.repo.find_by_id(user_id)
        # BUG: first_name or last_name might be None
        full_name = user.first_name + " " + user.last_name
        return full_name
"#
    }

    /// Go handler with panic potential
    pub fn go_user_handler() -> &'static str {
        r#"package handlers

import (
    "net/http"
    "github.com/acme/webapp/models"
)

func HandleGetUsers(w http.ResponseWriter, r *http.Request) {
    users := getUsers()
    // BUG: accessing index without bounds check
    firstUser := users[0]
    json.NewEncoder(w).Encode(firstUser)
}

func getUsers() []models.User {
    // Might return empty slice
    return nil
}
"#
    }
}

/// Sample ingest payloads
pub struct IngestFixtures;

impl IngestFixtures {
    /// Valid error ingest payload
    pub fn valid_error(project: &str) -> Value {
        json!({
            "project": project,
            "level": "ERROR",
            "message": "NullPointerException in UserService.getUser",
            "stack_trace": "at UserService.getUser(UserService.java:42)\nat Controller.handle(Controller.java:15)",
            "timestamp": "2024-01-15T10:30:00Z",
            "attributes": {
                "request_id": format!("req-{}", Uuid::new_v4()),
                "user_id": "user-123"
            }
        })
    }

    /// Warning level (should be rejected)
    pub fn warning_level(project: &str) -> Value {
        json!({
            "project": project,
            "level": "WARNING",
            "message": "Deprecated API usage",
            "timestamp": "2024-01-15T10:30:00Z"
        })
    }

    /// Missing project
    pub fn missing_project() -> Value {
        json!({
            "level": "ERROR",
            "message": "Some error message"
        })
    }

    /// With service name instead of project
    pub fn with_service_name() -> Value {
        json!({
            "service_name": "acme-webapp",
            "level": "ERROR",
            "message": "Error occurred",
            "stack_trace": "at handler.js:10:5"
        })
    }

    /// Kubernetes context
    pub fn kubernetes_error() -> Value {
        json!({
            "project": "acme/webapp",
            "service_name": "user-service",
            "kubernetes_namespace": "production",
            "kubernetes_pod": "user-service-abc123-xyz",
            "level": "ERROR",
            "message": "Pod crashed with OOMKilled",
            "attributes": {
                "exit_code": 137,
                "restart_count": 5
            }
        })
    }
}
