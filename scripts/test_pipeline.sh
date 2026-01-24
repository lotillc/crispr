#!/bin/bash
#
# CRISPR Pipeline Test Script
#
# Tests the full pipeline locally without needing external infrastructure.
# Creates a temp repo with intentional bugs, ingests errors, and monitors results.
#
# Usage:
#   ./scripts/test_pipeline.sh [setup|trigger|status|clean]
#
# Commands:
#   setup   - Create test repo with bugs and register it as a project
#   trigger - Ingest errors to CRISPR (run after setup)
#   status  - Check incident status
#   clean   - Remove test repo and project
#

set -e

CRISPR_URL="${CRISPR_URL:-http://localhost:8081}"
TEST_REPO_DIR="${TEST_REPO_DIR:-/tmp/crispr-test-repo}"
GITHUB_OWNER="${GITHUB_OWNER:-local}"
GITHUB_REPO="${GITHUB_REPO:-crispr-test}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() { echo -e "${GREEN}[CRISPR Test]${NC} $1"; }
warn() { echo -e "${YELLOW}[CRISPR Test]${NC} $1"; }
error() { echo -e "${RED}[CRISPR Test]${NC} $1"; }

# Check if CRISPR is running
check_crispr() {
    if ! curl -s "$CRISPR_URL/api/v1/status" > /dev/null 2>&1; then
        error "CRISPR is not running at $CRISPR_URL"
        echo "Start it with: cargo run"
        exit 1
    fi
    log "CRISPR is running at $CRISPR_URL"
}

# Create test repository with intentional bugs
setup() {
    log "Creating test repository at $TEST_REPO_DIR"

    rm -rf "$TEST_REPO_DIR"
    mkdir -p "$TEST_REPO_DIR"
    cd "$TEST_REPO_DIR"
    git init

    # Bug 1: Null reference error
    cat > src/handlers/user.ts << 'EOF'
// User handler with null reference bug
export function getUserId(req: any) {
    // BUG: No null check - will crash if req.user is undefined
    return req.user.id;
}

export function getUserEmail(req: any) {
    // BUG: Same issue
    return req.user.email.toLowerCase();
}
EOF

    # Bug 2: Array index out of bounds
    cat > src/utils/array.ts << 'EOF'
// Array utilities with bounds bug
export function getFirst<T>(items: T[]): T {
    // BUG: No check for empty array
    return items[0];
}

export function getLast<T>(items: T[]): T {
    // BUG: No check for empty array
    return items[items.length - 1];
}
EOF

    # Bug 3: Missing await
    cat > src/services/data.ts << 'EOF'
// Data service with async bug
async function fetchData(url: string): Promise<any> {
    const response = await fetch(url);
    return response.json();
}

export function getData(url: string) {
    // BUG: Missing await - returns Promise instead of data
    const data = fetchData(url);
    return data;
}
EOF

    # Bug 4: Type coercion issue
    cat > src/validators/amount.ts << 'EOF'
// Amount validator with type bug
export function validateAmount(amount: any): boolean {
    // BUG: String "0" is falsy but valid amount
    if (!amount) {
        throw new Error("Amount is required");
    }
    // BUG: No type coercion - "100" > 0 works but "100" + 1 = "1001"
    if (amount <= 0) {
        throw new Error("Amount must be positive");
    }
    return true;
}
EOF

    # Bug 5: JSON parse without error handling
    cat > src/api/client.ts << 'EOF'
// API client with parse bug
export async function fetchJson(url: string) {
    const response = await fetch(url);
    // BUG: No content-type check - HTML error pages will crash JSON.parse
    const data = await response.json();
    return data;
}
EOF

    mkdir -p src/handlers src/utils src/services src/validators src/api
    mv src/handlers/user.ts src/handlers/ 2>/dev/null || true
    mv src/utils/array.ts src/utils/ 2>/dev/null || true
    mv src/services/data.ts src/services/ 2>/dev/null || true
    mv src/validators/amount.ts src/validators/ 2>/dev/null || true
    mv src/api/client.ts src/api/ 2>/dev/null || true

    # Create proper directory structure
    mkdir -p src/handlers src/utils src/services src/validators src/api

    cat > src/handlers/user.ts << 'EOF'
// User handler with null reference bug
export function getUserId(req: any) {
    // BUG: No null check - will crash if req.user is undefined
    return req.user.id;
}

export function getUserEmail(req: any) {
    // BUG: Same issue
    return req.user.email.toLowerCase();
}
EOF

    cat > src/utils/array.ts << 'EOF'
// Array utilities with bounds bug
export function getFirst<T>(items: T[]): T {
    // BUG: No check for empty array
    return items[0];
}

export function getLast<T>(items: T[]): T {
    // BUG: No check for empty array
    return items[items.length - 1];
}
EOF

    cat > src/services/data.ts << 'EOF'
// Data service with async bug
async function fetchData(url: string): Promise<any> {
    const response = await fetch(url);
    return response.json();
}

export function getData(url: string) {
    // BUG: Missing await - returns Promise instead of data
    const data = fetchData(url);
    return data;
}
EOF

    cat > src/validators/amount.ts << 'EOF'
// Amount validator with type bug
export function validateAmount(amount: any): boolean {
    // BUG: String "0" is falsy but valid amount
    if (!amount) {
        throw new Error("Amount is required");
    }
    // BUG: No type coercion - "100" > 0 works but "100" + 1 = "1001"
    if (amount <= 0) {
        throw new Error("Amount must be positive");
    }
    return true;
}
EOF

    cat > src/api/client.ts << 'EOF'
// API client with parse bug
export async function fetchJson(url: string) {
    const response = await fetch(url);
    // BUG: No content-type check - HTML error pages will crash JSON.parse
    const data = await response.json();
    return data;
}
EOF

    cat > package.json << EOF
{
  "name": "crispr-test",
  "version": "1.0.0",
  "description": "Test repository for CRISPR pipeline testing"
}
EOF

    cat > CRISPR.md << 'EOF'
# CRISPR Context

## 1. Special Notes
This is a test repository with intentional bugs for testing the CRISPR pipeline.
All bugs are simple and should be fixable with null checks, type guards, or error handling.

## 2. Recent Fixes
<!-- Automatically maintained by CRISPR -->

## 3. Service Description
A test service with intentional bugs in common patterns:
- Null reference errors
- Array bounds issues
- Async/await mistakes
- Type coercion problems
- Missing error handling
EOF

    git add .
    git commit -m "Initial commit with intentional bugs"

    log "Test repo created with 5 buggy files"

    # Register project with CRISPR
    check_crispr
    log "Registering project with CRISPR..."

    RESULT=$(curl -s -X POST "$CRISPR_URL/api/v1/projects" \
        -H "Content-Type: application/json" \
        -d "{\"github_owner\": \"$GITHUB_OWNER\", \"github_repo\": \"$GITHUB_REPO\"}" 2>&1)

    if echo "$RESULT" | grep -q "already exists"; then
        warn "Project already registered"
    elif echo "$RESULT" | grep -q "id"; then
        log "Project registered successfully"
    else
        error "Failed to register project: $RESULT"
    fi

    echo ""
    log "Setup complete! Next steps:"
    echo "  1. Run: ./scripts/test_pipeline.sh trigger"
    echo "  2. Watch the UI at $CRISPR_URL"
    echo "  3. Check status: ./scripts/test_pipeline.sh status"
}

# Trigger errors by ingesting to CRISPR
trigger() {
    check_crispr
    log "Triggering test errors..."

    # Error 1: Null reference
    log "Ingesting: Null reference error"
    curl -s -X POST "$CRISPR_URL/ingest" \
        -H "Content-Type: application/json" \
        -d "{
            \"project\": \"$GITHUB_OWNER/$GITHUB_REPO\",
            \"level\": \"error\",
            \"message\": \"TypeError: Cannot read property 'id' of undefined\",
            \"stack_trace\": \"TypeError: Cannot read property 'id' of undefined\n    at getUserId (src/handlers/user.ts:4:21)\n    at handleRequest (src/server.ts:45:12)\"
        }" | jq -r '.message // .error' 2>/dev/null || echo "(response received)"

    # Error 2: Array bounds
    log "Ingesting: Array bounds error"
    curl -s -X POST "$CRISPR_URL/ingest" \
        -H "Content-Type: application/json" \
        -d "{
            \"project\": \"$GITHUB_OWNER/$GITHUB_REPO\",
            \"level\": \"error\",
            \"message\": \"TypeError: Cannot read property '0' of undefined\",
            \"stack_trace\": \"TypeError: Cannot read property '0' of undefined\n    at getFirst (src/utils/array.ts:4:18)\n    at processItems (src/handlers/items.ts:23:9)\"
        }" | jq -r '.message // .error' 2>/dev/null || echo "(response received)"

    # Error 3: Promise not awaited
    log "Ingesting: Unhandled promise"
    curl -s -X POST "$CRISPR_URL/ingest" \
        -H "Content-Type: application/json" \
        -d "{
            \"project\": \"$GITHUB_OWNER/$GITHUB_REPO\",
            \"level\": \"error\",
            \"message\": \"TypeError: data.map is not a function\",
            \"stack_trace\": \"TypeError: data.map is not a function\n    at processData (src/handlers/data.ts:12:15)\n    at getData (src/services/data.ts:10:5)\"
        }" | jq -r '.message // .error' 2>/dev/null || echo "(response received)"

    # Error 4: Validation type error
    log "Ingesting: Validation error"
    curl -s -X POST "$CRISPR_URL/ingest" \
        -H "Content-Type: application/json" \
        -d "{
            \"project\": \"$GITHUB_OWNER/$GITHUB_REPO\",
            \"level\": \"error\",
            \"message\": \"Error: Amount must be positive\",
            \"stack_trace\": \"Error: Amount must be positive\n    at validateAmount (src/validators/amount.ts:9:15)\n    at processPayment (src/handlers/payment.ts:34:5)\",
            \"attributes\": {\"amount\": \"-50\", \"type\": \"string\"}
        }" | jq -r '.message // .error' 2>/dev/null || echo "(response received)"

    # Error 5: JSON parse error
    log "Ingesting: JSON parse error"
    curl -s -X POST "$CRISPR_URL/ingest" \
        -H "Content-Type: application/json" \
        -d "{
            \"project\": \"$GITHUB_OWNER/$GITHUB_REPO\",
            \"level\": \"error\",
            \"message\": \"SyntaxError: Unexpected token '<' in JSON at position 0\",
            \"stack_trace\": \"SyntaxError: Unexpected token '<' in JSON at position 0\n    at JSON.parse (<anonymous>)\n    at fetchJson (src/api/client.ts:5:31)\"
        }" | jq -r '.message // .error' 2>/dev/null || echo "(response received)"

    echo ""
    log "5 errors ingested!"
    echo ""
    log "Watch the pipeline at: $CRISPR_URL"
    echo "  - Incidents will appear in Triage queue"
    echo "  - If LLM is configured, they'll be analyzed and fixed"
    echo "  - Check status with: ./scripts/test_pipeline.sh status"
}

# Check status of incidents
status() {
    check_crispr
    log "Fetching incident status..."
    echo ""

    curl -s "$CRISPR_URL/api/v1/incidents?limit=10" | jq -r '
        if type == "array" then
            if length == 0 then
                "No incidents found"
            else
                .[] | "[\(.status | ascii_upcase)] \(.message | .[0:60])..."
            end
        else
            "Error: \(.)"
        end
    ' 2>/dev/null || echo "Failed to fetch incidents (is CRISPR running?)"

    echo ""
    log "View details at: $CRISPR_URL"
}

# Clean up test repo and project
clean() {
    log "Cleaning up..."

    if [ -d "$TEST_REPO_DIR" ]; then
        rm -rf "$TEST_REPO_DIR"
        log "Removed test repo at $TEST_REPO_DIR"
    fi

    # Note: We don't delete the project from CRISPR as that would require knowing the ID
    warn "Project may still exist in CRISPR - delete manually from UI if needed"

    log "Cleanup complete"
}

# Main
case "${1:-}" in
    setup)
        setup
        ;;
    trigger)
        trigger
        ;;
    status)
        status
        ;;
    clean)
        clean
        ;;
    *)
        echo "CRISPR Pipeline Test Script"
        echo ""
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  setup   - Create test repo with bugs and register project"
        echo "  trigger - Ingest errors to CRISPR"
        echo "  status  - Check incident status"
        echo "  clean   - Remove test repo"
        echo ""
        echo "Quick start:"
        echo "  $0 setup    # Create test repo"
        echo "  $0 trigger  # Send errors to CRISPR"
        echo "  $0 status   # Check what happened"
        echo ""
        echo "Environment variables:"
        echo "  CRISPR_URL   - CRISPR server URL (default: http://localhost:8081)"
        echo "  TEST_REPO_DIR - Where to create test repo (default: /tmp/crispr-test-repo)"
        ;;
esac
