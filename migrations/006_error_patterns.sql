-- Error patterns table for automatic categorization and matching
-- These patterns allow auto-triaging known error types without LLM calls

CREATE TABLE error_patterns (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Pattern name for display
    name TEXT NOT NULL,
    -- Regex pattern to match against error messages/stack traces
    regex_pattern TEXT NOT NULL,
    -- Category this pattern belongs to (matches triage categories)
    category TEXT NOT NULL,
    -- Suggested action for this pattern type
    suggested_action TEXT,
    -- Severity level (critical, high, medium, low)
    severity TEXT DEFAULT 'medium',
    -- Whether this pattern is user-defined or built-in
    is_builtin BOOLEAN DEFAULT false,
    -- Whether this pattern is enabled
    enabled BOOLEAN DEFAULT true,
    -- Auto-triage: if true, matching errors skip LLM triage
    auto_triage BOOLEAN DEFAULT false,
    -- Auto-triage fixability (only used if auto_triage is true)
    auto_fixable BOOLEAN DEFAULT true,
    -- Description of the pattern for humans
    description TEXT,
    -- Example of what this pattern matches
    example TEXT,
    -- Tags for filtering/grouping
    tags TEXT[] DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_error_patterns_category ON error_patterns(category);
CREATE INDEX idx_error_patterns_enabled ON error_patterns(enabled);
CREATE INDEX idx_error_patterns_builtin ON error_patterns(is_builtin);

-- Pattern match statistics - track how often patterns are matched
CREATE TABLE pattern_match_stats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pattern_id UUID NOT NULL REFERENCES error_patterns(id) ON DELETE CASCADE,
    incident_id UUID NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
    -- Confidence of the match (for partial/fuzzy matches)
    confidence FLOAT NOT NULL DEFAULT 1.0,
    -- Portion of the error that matched
    matched_text TEXT,
    -- Whether this match led to successful fix
    led_to_fix BOOLEAN,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_pattern_match_stats_pattern ON pattern_match_stats(pattern_id);
CREATE INDEX idx_pattern_match_stats_incident ON pattern_match_stats(incident_id);
CREATE INDEX idx_pattern_match_stats_created ON pattern_match_stats(created_at);

-- Add pattern-related columns to incidents
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS matched_pattern_id UUID REFERENCES error_patterns(id);
ALTER TABLE incidents ADD COLUMN IF NOT EXISTS pattern_auto_triaged BOOLEAN DEFAULT false;

-- Insert built-in patterns for common error types
INSERT INTO error_patterns (name, regex_pattern, category, suggested_action, severity, is_builtin, auto_triage, auto_fixable, description, example, tags) VALUES
-- Null/undefined reference errors
(
    'NullPointerException (Java)',
    'java\.lang\.NullPointerException|NullPointerException',
    'null_reference',
    'Add null check before accessing the object',
    'high',
    true,
    true,
    true,
    'Java null pointer exception when accessing a null object reference',
    'java.lang.NullPointerException: Cannot invoke method on null object',
    ARRAY['java', 'null', 'npe']
),
(
    'Nil Pointer Dereference (Go)',
    'runtime error: invalid memory address or nil pointer dereference|nil pointer dereference',
    'null_reference',
    'Add nil check before accessing the pointer',
    'high',
    true,
    true,
    true,
    'Go nil pointer dereference when accessing an uninitialized pointer',
    'panic: runtime error: invalid memory address or nil pointer dereference',
    ARRAY['go', 'golang', 'nil', 'pointer']
),
(
    'TypeError: Cannot read property of null/undefined (JS)',
    'TypeError: Cannot read propert(y|ies) .* of (null|undefined)|TypeError: .* is (null|undefined)',
    'null_reference',
    'Add null/undefined check or use optional chaining',
    'high',
    true,
    true,
    true,
    'JavaScript TypeError when accessing property of null or undefined',
    'TypeError: Cannot read properties of undefined (reading ''name'')',
    ARRAY['javascript', 'typescript', 'null', 'undefined']
),
(
    'AttributeError: NoneType (Python)',
    'AttributeError: ''NoneType'' object has no attribute',
    'null_reference',
    'Add None check before accessing the attribute',
    'high',
    true,
    true,
    true,
    'Python AttributeError when accessing attribute on None',
    'AttributeError: ''NoneType'' object has no attribute ''name''',
    ARRAY['python', 'none', 'null']
),

-- Connection timeout errors
(
    'Connection Timeout',
    'connection timed out|connect timed out|timeout expired|ETIMEDOUT|ConnectionTimeoutError|TimeoutError.*connect',
    'connection_timeout',
    'Increase timeout, add retry logic, or check network configuration',
    'medium',
    true,
    false,
    false,
    'Network connection timeout - usually indicates network issues or slow services',
    'Error: connect ETIMEDOUT 10.0.0.1:5432',
    ARRAY['network', 'timeout', 'connection']
),
(
    'Read/Socket Timeout',
    'read timed out|socket timed out|SocketTimeoutException|ReadTimeoutError|ESOCKETTIMEDOUT',
    'connection_timeout',
    'Increase read timeout, check server response times',
    'medium',
    true,
    false,
    false,
    'Socket read timeout - server took too long to respond',
    'java.net.SocketTimeoutException: Read timed out',
    ARRAY['network', 'timeout', 'socket']
),

-- Authentication failures
(
    'HTTP 401 Unauthorized',
    'HTTP 401|401 Unauthorized|Unauthorized|authentication failed|invalid.*token|token.*expired|invalid.*credentials',
    'authentication',
    'Check authentication credentials, refresh tokens, or verify API keys',
    'high',
    true,
    false,
    false,
    'Authentication failure - invalid or expired credentials',
    'Error: 401 Unauthorized - Invalid API token',
    ARRAY['auth', 'security', '401']
),
(
    'HTTP 403 Forbidden',
    'HTTP 403|403 Forbidden|Forbidden|access denied|permission denied|insufficient.*permission',
    'authorization',
    'Check user permissions and access control rules',
    'high',
    true,
    false,
    false,
    'Authorization failure - user lacks required permissions',
    'Error: 403 Forbidden - Access denied',
    ARRAY['auth', 'security', '403', 'permissions']
),

-- Rate limiting errors
(
    'Rate Limit Exceeded',
    'rate limit|too many requests|429|throttl(ed|ing)|quota exceeded|API rate limit|RateLimitError',
    'rate_limit',
    'Implement exponential backoff, add request queuing, or increase rate limits',
    'medium',
    true,
    false,
    false,
    'API rate limit exceeded - too many requests in time window',
    'Error: 429 Too Many Requests - Rate limit exceeded',
    ARRAY['api', 'rate-limit', '429']
),

-- Database connection errors
(
    'Database Connection Failed',
    'could not connect to.*database|connection refused.*:5432|connection refused.*:3306|ECONNREFUSED.*(5432|3306|27017)|database.*connection.*failed|pg_connect|mysql_connect',
    'database_connection',
    'Check database server status, connection string, and network connectivity',
    'critical',
    true,
    false,
    false,
    'Database connection failure - cannot reach database server',
    'Error: could not connect to server: Connection refused (127.0.0.1:5432)',
    ARRAY['database', 'connection', 'postgres', 'mysql']
),
(
    'Database Connection Pool Exhausted',
    'connection pool|pool exhausted|no available connection|too many connections|max_connections|pool timeout',
    'database_connection',
    'Increase connection pool size, check for connection leaks, optimize query performance',
    'high',
    true,
    false,
    false,
    'Database connection pool exhausted - all connections in use',
    'Error: Connection pool exhausted - max connections reached',
    ARRAY['database', 'connection', 'pool']
),

-- Memory/resource exhaustion
(
    'Out of Memory (OOM)',
    'out of memory|OutOfMemoryError|OOM|heap space|GC overhead limit|JavaScript heap out of memory|MemoryError|memory allocation failed',
    'resource_exhaustion',
    'Increase memory limits, optimize memory usage, fix memory leaks',
    'critical',
    true,
    false,
    false,
    'Application ran out of memory',
    'FATAL ERROR: CALL_AND_RETRY_LAST Allocation failed - JavaScript heap out of memory',
    ARRAY['memory', 'oom', 'resource']
),
(
    'Stack Overflow',
    'stack overflow|StackOverflowError|maximum call stack size exceeded|too much recursion',
    'resource_exhaustion',
    'Check for infinite recursion, increase stack size, convert to iterative approach',
    'high',
    true,
    true,
    true,
    'Stack overflow from deep or infinite recursion',
    'RangeError: Maximum call stack size exceeded',
    ARRAY['stack', 'recursion', 'resource']
),
(
    'Disk Space Exhausted',
    'no space left on device|disk full|ENOSPC|out of disk space|insufficient storage',
    'resource_exhaustion',
    'Free disk space, implement log rotation, check for runaway file growth',
    'critical',
    true,
    false,
    false,
    'Disk storage exhausted',
    'Error: ENOSPC: no space left on device',
    ARRAY['disk', 'storage', 'resource']
),

-- Array/index errors
(
    'Array Index Out of Bounds',
    'ArrayIndexOutOfBoundsException|IndexOutOfBoundsException|index out of (range|bounds)|IndexError: list index out of range|panic: runtime error: index out of range',
    'bounds_error',
    'Add bounds checking before array access',
    'high',
    true,
    true,
    true,
    'Array index access beyond valid bounds',
    'java.lang.ArrayIndexOutOfBoundsException: Index 5 out of bounds for length 3',
    ARRAY['array', 'index', 'bounds']
),

-- Type errors
(
    'Type Mismatch/Cast Error',
    'TypeError|ClassCastException|cannot convert|type mismatch|invalid cast|cannot assign|incompatible types',
    'type_error',
    'Fix type annotations, add type conversion, or correct type usage',
    'medium',
    true,
    true,
    true,
    'Type mismatch or invalid type conversion',
    'TypeError: cannot read property of undefined',
    ARRAY['type', 'casting']
),

-- Import/module errors
(
    'Module Not Found',
    'ModuleNotFoundError|Cannot find module|ImportError|No module named|Module not found|require.*failed|ERR_MODULE_NOT_FOUND',
    'missing_import',
    'Install missing dependency or fix import path',
    'medium',
    true,
    true,
    true,
    'Module or package import failed',
    'ModuleNotFoundError: No module named ''requests''',
    ARRAY['import', 'module', 'dependency']
),

-- JSON/Parsing errors
(
    'JSON Parse Error',
    'JSON\.parse|SyntaxError.*JSON|invalid json|Unexpected token|json decode|JSONDecodeError',
    'parse_error',
    'Validate JSON input, add error handling for malformed data',
    'medium',
    true,
    true,
    true,
    'Failed to parse JSON data',
    'SyntaxError: Unexpected token < in JSON at position 0',
    ARRAY['json', 'parsing', 'syntax']
),

-- File errors
(
    'File Not Found',
    'ENOENT|FileNotFoundError|No such file or directory|file not found|path not found',
    'file_error',
    'Check file path, verify file exists, handle missing file gracefully',
    'medium',
    true,
    true,
    true,
    'File or directory not found',
    'Error: ENOENT: no such file or directory, open ''/path/to/file''',
    ARRAY['file', 'filesystem', 'path']
),

-- Network/HTTP errors
(
    'HTTP 500 Internal Server Error',
    'HTTP 500|500 Internal Server Error|Internal Server Error|server error.*500',
    'external_service',
    'Check server logs, implement retry logic with backoff',
    'high',
    true,
    false,
    false,
    'Remote server returned 500 error',
    'Error: Request failed with status code 500',
    ARRAY['http', 'server', '500']
),
(
    'HTTP 502/503/504 Gateway Error',
    'HTTP 50[234]|502 Bad Gateway|503 Service Unavailable|504 Gateway Timeout|bad gateway|service unavailable',
    'external_service',
    'Check upstream service health, implement retry with exponential backoff',
    'high',
    true,
    false,
    false,
    'Gateway or upstream service error',
    'Error: 503 Service Unavailable',
    ARRAY['http', 'gateway', '502', '503', '504']
),
(
    'DNS Resolution Failed',
    'ENOTFOUND|getaddrinfo|DNS.*failed|name.*resolution|unknown host|could not resolve host',
    'network_error',
    'Check DNS configuration, verify hostname, check network connectivity',
    'high',
    true,
    false,
    false,
    'DNS resolution failure - cannot resolve hostname',
    'Error: getaddrinfo ENOTFOUND api.example.com',
    ARRAY['dns', 'network', 'resolution']
),

-- Assertion/validation errors
(
    'Assertion Failed',
    'AssertionError|assertion failed|assert.*failed|invariant violation',
    'logic_error',
    'Check assertion condition, fix logical error causing assertion to fail',
    'medium',
    true,
    true,
    true,
    'Code assertion failure',
    'AssertionError: expected 5 to equal 3',
    ARRAY['assertion', 'test', 'validation']
);

COMMENT ON TABLE error_patterns IS 'Regex patterns for automatic error categorization and triage';
COMMENT ON COLUMN error_patterns.regex_pattern IS 'Regular expression to match against error messages and stack traces';
COMMENT ON COLUMN error_patterns.auto_triage IS 'If true, matching errors skip LLM triage and use pattern-defined fixability';
COMMENT ON TABLE pattern_match_stats IS 'Statistics on pattern matches for analytics and pattern refinement';
