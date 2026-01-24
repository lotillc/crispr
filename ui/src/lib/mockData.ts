// Mock data for demo mode
import type { Project, Incident, AuthStatus, UsageSummary, IncidentStats, GeneratedTest, VerificationRound, VerificationSummary } from './api';

export const mockProjects: Project[] = [
	{
		id: 'proj-1',
		repo: 'acme/payments-api',
		enabled: true,
		auto_fix: true,
		watch_logs: true,
		created_at: '2024-01-10T08:00:00Z',
		incident_count: 12,
		last_incident: '2024-01-22T14:30:00Z'
	},
	{
		id: 'proj-2',
		repo: 'acme/user-service',
		enabled: true,
		auto_fix: true,
		watch_logs: true,
		created_at: '2024-01-08T10:00:00Z',
		incident_count: 8,
		last_incident: '2024-01-22T12:15:00Z'
	},
	{
		id: 'proj-3',
		repo: 'acme/notification-worker',
		enabled: true,
		auto_fix: false,
		watch_logs: true,
		created_at: '2024-01-05T09:00:00Z',
		incident_count: 3,
		last_incident: '2024-01-21T18:45:00Z'
	},
	{
		id: 'proj-4',
		repo: 'acme/analytics-dashboard',
		enabled: false,
		auto_fix: false,
		watch_logs: false,
		created_at: '2024-01-02T11:00:00Z',
		incident_count: 0
	}
];

export const mockIncidents: Incident[] = [
	{
		id: 'inc-1',
		project_id: 'proj-1',
		fingerprint: 'fp-abc123',
		level: 'error',
		message: "TypeError: Cannot read property 'userId' of undefined",
		status: 'pr_open',
		sample_count: 847,
		first_seen: '2024-01-20T10:00:00Z',
		last_seen: '2024-01-22T14:30:00Z',
		fix_attempts: 1,
		pr_url: 'https://github.com/acme/payments-api/pull/142',
		pr_number: 142,
		triage_result: {
			fixable: true,
			confidence: 0.92,
			reason: 'Null reference error with clear fix pattern',
			category: 'null_reference',
			suggested_files: ['src/handlers/payment.ts'],
			description: 'The processRefund handler receives a request where the user session has expired, resulting in req.user being undefined. When the code attempts to access req.user.userId, it throws a TypeError.',
			fix_approach: 'Add null check before accessing userId property',
			what_changed: 'Added optional chaining and early return with 401 status when user session is missing. The handler now validates the session before processing the refund.',
			expected_behavior: 'Requests with expired sessions will receive a clear 401 Unauthorized response instead of crashing. Valid requests continue to process normally.'
		}
	},
	{
		id: 'inc-2',
		project_id: 'proj-1',
		fingerprint: 'fp-def456',
		level: 'error',
		message: 'ValidationError: Amount must be a positive number',
		status: 'fixing',
		sample_count: 234,
		first_seen: '2024-01-21T08:00:00Z',
		last_seen: '2024-01-22T13:00:00Z',
		fix_attempts: 0,
		triage_result: {
			fixable: true,
			confidence: 0.88,
			reason: 'Input validation issue',
			category: 'type_error',
			suggested_files: ['src/validators/amount.ts'],
			description: 'String being passed instead of number',
			fix_approach: 'Add type coercion in validator'
		}
	},
	{
		id: 'inc-3',
		project_id: 'proj-2',
		fingerprint: 'fp-ghi789',
		level: 'error',
		message: "ReferenceError: getProfile is not defined",
		status: 'verifying',
		sample_count: 56,
		first_seen: '2026-01-22T15:00:00Z',
		last_seen: '2026-01-22T15:30:00Z',
		fix_attempts: 1,
		pr_url: 'https://github.com/acme/user-service/pull/89',
		pr_number: 89,
		pr_merged_at: '2026-01-23T10:00:00Z',
		verification_round: 2,
		verification_status: 'verifying',
		verification_duration_hours: 24,
		verification_started_at: '2026-01-23T10:00:00Z',
		verification_ends_at: '2026-01-24T10:00:00Z',
		recurrence_count: 0,
		triage_result: {
			fixable: true,
			confidence: 0.95,
			reason: 'Missing import statement',
			category: 'missing_import',
			suggested_files: ['src/controllers/user.ts'],
			description: 'Function not imported from utils module',
			fix_approach: 'Add missing import statement',
			what_changed: 'Added the missing import for getProfile from utils/profile',
			expected_behavior: 'UserController.show() successfully calls getProfile and returns user data',
			recommended_verification_hours: 24,
			verification_reasoning: 'Missing import is a deterministic error - it either works or it does not. 24-hour monitoring is sufficient to confirm the fix works across all code paths that call this function.'
		}
	},
	{
		id: 'inc-4',
		project_id: 'proj-2',
		fingerprint: 'fp-jkl012',
		level: 'error',
		message: 'IndexError: list index out of range',
		status: 'triaging',
		sample_count: 12,
		first_seen: '2024-01-22T11:00:00Z',
		last_seen: '2024-01-22T12:15:00Z',
		fix_attempts: 0
	},
	{
		id: 'inc-5',
		project_id: 'proj-1',
		fingerprint: 'fp-mno345',
		level: 'error',
		message: 'TimeoutError: Database connection timed out',
		status: 'unfixable',
		sample_count: 45,
		first_seen: '2024-01-18T09:00:00Z',
		last_seen: '2024-01-22T10:00:00Z',
		fix_attempts: 0,
		triage_result: {
			fixable: false,
			confidence: 0.85,
			reason: 'Infrastructure issue, not a code bug',
			category: 'external_service',
			suggested_files: [],
			description: 'Database connection pool exhaustion'
		}
	},
	{
		id: 'inc-6',
		project_id: 'proj-3',
		fingerprint: 'fp-pqr678',
		level: 'error',
		message: 'SMTPError: Connection refused to mail server',
		status: 'detected',
		sample_count: 89,
		first_seen: '2024-01-21T14:00:00Z',
		last_seen: '2024-01-21T18:45:00Z',
		fix_attempts: 0
	},
	{
		id: 'inc-7',
		project_id: 'proj-1',
		fingerprint: 'fp-stu901',
		level: 'warning',
		message: 'DeprecationWarning: Buffer() is deprecated',
		status: 'ignored',
		sample_count: 1203,
		first_seen: '2024-01-15T08:00:00Z',
		last_seen: '2024-01-22T14:00:00Z',
		fix_attempts: 0
	},
	{
		id: 'inc-8',
		project_id: 'proj-2',
		fingerprint: 'fp-vwx234',
		level: 'error',
		message: "SyntaxError: Unexpected token '<' in JSON",
		status: 'pr_open',
		sample_count: 23,
		first_seen: '2024-01-22T09:00:00Z',
		last_seen: '2024-01-22T11:00:00Z',
		fix_attempts: 1,
		pr_url: 'https://github.com/acme/user-service/pull/91',
		pr_number: 91,
		triage_result: {
			fixable: true,
			confidence: 0.78,
			reason: 'HTML response being parsed as JSON',
			category: 'logic_error',
			suggested_files: ['src/api/client.ts'],
			description: 'When the external API returns an error (5xx status), the server responds with an HTML error page. The client attempts to parse this as JSON, causing a SyntaxError when it encounters the opening < of the HTML doctype.',
			fix_approach: 'Add content-type check before parsing',
			what_changed: 'Added response content-type validation before JSON parsing. When the response is not application/json, the client now throws a descriptive ApiError with the status code and a helpful message instead of attempting to parse HTML.',
			expected_behavior: 'API errors will be caught and reported with meaningful error messages. The application will gracefully handle upstream service failures instead of crashing on parse errors.'
		}
	},
	// Verification lifecycle examples
	{
		id: 'inc-9',
		project_id: 'proj-1',
		fingerprint: 'fp-ver001',
		level: 'error',
		message: "NullPointerException: session.getAttribute() returned null",
		status: 'verifying',
		sample_count: 312,
		first_seen: '2026-01-20T08:00:00Z',
		last_seen: '2026-01-22T14:00:00Z',
		fix_attempts: 1,
		pr_url: 'https://github.com/acme/payments-api/pull/138',
		pr_number: 138,
		pr_merged_at: '2026-01-22T16:00:00Z',
		verification_round: 3,
		verification_status: 'verifying',
		verification_duration_hours: 48,
		verification_started_at: '2026-01-22T16:00:00Z',
		verification_ends_at: '2026-01-24T16:00:00Z',
		recurrence_count: 0,
		triage_result: {
			fixable: true,
			confidence: 0.91,
			reason: 'Session attribute access without null check',
			category: 'null_reference',
			suggested_files: ['src/middleware/session.ts'],
			description: 'Session attribute accessed before confirming session validity',
			fix_approach: 'Add session validation before accessing attributes',
			what_changed: 'Added early session validation with proper error response',
			expected_behavior: 'Invalid sessions return 401 instead of throwing NPE',
			recommended_verification_hours: 48,
			verification_reasoning: 'Session issues typically surface within 24-48 hours as users cycle through login/logout flows. Monitoring for 2 days covers weekday and weekend traffic patterns.'
		}
	},
	{
		id: 'inc-10',
		project_id: 'proj-2',
		fingerprint: 'fp-ver002',
		level: 'error',
		message: "TypeError: Cannot read properties of undefined (reading 'map')",
		status: 'verified',
		sample_count: 89,
		first_seen: '2024-01-10T10:00:00Z',
		last_seen: '2024-01-12T09:00:00Z',
		fix_attempts: 1,
		pr_url: 'https://github.com/acme/user-service/pull/85',
		pr_number: 85,
		pr_merged_at: '2024-01-12T14:00:00Z',
		verification_round: 3,
		verification_status: 'verified',
		verified_at: '2024-01-13T14:00:00Z',
		verification_duration_hours: 24,
		verification_started_at: '2024-01-12T14:00:00Z',
		verification_ends_at: '2024-01-13T14:00:00Z',
		recurrence_count: 0,
		triage_result: {
			fixable: true,
			confidence: 0.94,
			reason: 'Array method called on undefined',
			category: 'undefined_access',
			suggested_files: ['src/components/UserList.tsx'],
			description: 'users.map() called when API returns undefined instead of empty array',
			fix_approach: 'Add default empty array fallback',
			what_changed: 'Added nullish coalescing: (users ?? []).map()',
			expected_behavior: 'Component renders empty state when no users returned',
			recommended_verification_hours: 24,
			verification_reasoning: 'Simple null check fix with high confidence. Standard 24-hour monitoring sufficient to catch any edge cases in normal user flows.'
		}
	},
	{
		id: 'inc-11',
		project_id: 'proj-1',
		fingerprint: 'fp-ver003',
		level: 'error',
		message: "ConnectionResetError: Connection reset by peer during transaction",
		status: 'verifying',
		sample_count: 156,
		first_seen: '2026-01-15T11:00:00Z',
		last_seen: '2026-01-18T15:30:00Z',
		fix_attempts: 1,
		pr_url: 'https://github.com/acme/payments-api/pull/140',
		pr_number: 140,
		pr_merged_at: '2026-01-20T09:00:00Z',
		verification_round: 1,
		verification_status: 'verifying',
		verification_duration_hours: 168,
		verification_started_at: '2026-01-20T09:00:00Z',
		verification_ends_at: '2026-01-27T09:00:00Z',
		recurrence_count: 0,
		triage_result: {
			fixable: true,
			confidence: 0.72,
			reason: 'Connection not properly handled during network issues',
			category: 'network_error',
			suggested_files: ['src/db/transaction.ts'],
			description: 'Database transactions fail silently when connection drops',
			fix_approach: 'Add retry logic and proper connection pooling',
			what_changed: 'Implemented connection health checks and automatic retry',
			expected_behavior: 'Transactions retry on transient network failures',
			recommended_verification_hours: 168,
			verification_reasoning: 'Network issues are often transient and load-dependent. This error occurred sporadically over 3 days. Recommend 7-day monitoring to capture various traffic patterns and potential infrastructure events.'
		}
	},
	{
		id: 'inc-12',
		project_id: 'proj-3',
		fingerprint: 'fp-ver004',
		level: 'error',
		message: "RateLimitError: API rate limit exceeded",
		status: 'pending_verification',
		sample_count: 234,
		first_seen: '2026-01-10T09:00:00Z',
		last_seen: '2026-01-23T08:00:00Z',
		fix_attempts: 1,
		pr_url: 'https://github.com/acme/notification-worker/pull/45',
		pr_number: 45,
		pr_merged_at: '2026-01-24T10:00:00Z',
		verification_duration_hours: 720,
		triage_result: {
			fixable: true,
			confidence: 0.88,
			reason: 'No rate limiting on outbound API calls',
			category: 'rate_limit',
			suggested_files: ['src/services/email.ts', 'src/utils/rateLimit.ts'],
			description: 'Burst of notifications triggers external API rate limits',
			fix_approach: 'Add token bucket rate limiter for outbound calls',
			what_changed: 'Implemented rate limiter with exponential backoff',
			expected_behavior: 'Notifications queue and send within rate limits',
			recommended_verification_hours: 720,
			verification_reasoning: 'Rate limit errors depend on usage volume. This service handles marketing campaigns which spike at month-end. Recommend 30-day monitoring to cover a full billing cycle and catch any edge cases in high-volume periods.'
		}
	}
];

export const mockAuthStatus: AuthStatus = {
	github: {
		connected: true,
		mode: 'pat',
		user: {
			login: 'demo-user',
			name: 'Demo User',
			avatar_url: 'https://avatars.githubusercontent.com/u/1234567'
		}
	},
	slack: {
		enabled: true,
		connected: true,
		team_name: 'Acme Corp',
		channel_name: '#crispr-alerts'
	},
	llm: {
		configured: true,
		reader_model: 'claude-3-haiku-20240307',
		writer_model: 'claude-sonnet-4-20250514'
	}
};

export const mockUsageSummary: UsageSummary = {
	total_requests: 1247,
	total_input_tokens: 2450000,
	total_output_tokens: 890000,
	total_cost: 47.82,
	by_provider: {
		anthropic: {
			requests: 1105,
			input_tokens: 2200000,
			output_tokens: 780000,
			cost: 42.15
		},
		openai: {
			requests: 142,
			input_tokens: 250000,
			output_tokens: 110000,
			cost: 5.67
		}
	},
	by_purpose: {
		triage: {
			requests: 856,
			cost: 8.42
		},
		context: {
			requests: 203,
			cost: 4.12
		},
		fix: {
			requests: 188,
			cost: 35.28
		}
	}
};

export const mockIncidentStats: IncidentStats = {
	total: 52,
	by_status: {
		detected: 2,
		triaging: 1,
		fixing: 1,
		pr_open: 2,
		pr_merged: 22,
		verifying: 4,
		verified: 8,
		pending_verification: 1,
		unfixable: 5,
		ignored: 6
	},
	fixed_today: 3,
	fixed_week: 15,
	avg_fix_time_hours: 2.4
};

// Helper to get mock incident with project info
export function getMockIncidentsWithProjects(): Incident[] {
	return mockIncidents.map(incident => ({
		...incident,
		project: mockProjects.find(p => p.id === incident.project_id)
	}));
}

// Mock samples for incidents
export const mockSamples: Record<string, Array<{ id: string; incident_id: string; timestamp: string; raw_log: string; stack_trace?: string; metadata?: Record<string, any> }>> = {
	'inc-1': [
		{
			id: 'sample-1a',
			incident_id: 'inc-1',
			timestamp: '2024-01-22T14:30:00Z',
			raw_log: "TypeError: Cannot read property 'userId' of undefined",
			stack_trace: `TypeError: Cannot read property 'userId' of undefined
    at processRefund (/app/src/handlers/payment.ts:42:15)
    at async Router.handle (/app/node_modules/express/lib/router/index.js:174:5)
    at async Layer.handle (/app/node_modules/express/lib/router/layer.js:95:5)
    at async /app/src/middleware/auth.ts:28:9`,
			metadata: { request_id: 'req-abc123', user_agent: 'Mozilla/5.0' }
		}
	],
	'inc-2': [
		{
			id: 'sample-2a',
			incident_id: 'inc-2',
			timestamp: '2024-01-22T13:00:00Z',
			raw_log: 'ValidationError: Amount must be a positive number',
			stack_trace: `ValidationError: Amount must be a positive number
    at validateAmount (/app/src/validators/amount.ts:15:11)
    at processPayment (/app/src/handlers/payment.ts:28:3)
    at async Router.handle (/app/node_modules/express/lib/router/index.js:174:5)`,
			metadata: { amount: '-50.00', currency: 'USD' }
		}
	],
	'inc-3': [
		{
			id: 'sample-3a',
			incident_id: 'inc-3',
			timestamp: '2024-01-19T15:30:00Z',
			raw_log: 'ReferenceError: getProfile is not defined',
			stack_trace: `ReferenceError: getProfile is not defined
    at UserController.show (/app/src/controllers/user.ts:55:12)
    at async Router.handle (/app/node_modules/express/lib/router/index.js:174:5)`
		}
	],
	'inc-4': [
		{
			id: 'sample-4a',
			incident_id: 'inc-4',
			timestamp: '2024-01-22T12:15:00Z',
			raw_log: 'IndexError: list index out of range',
			stack_trace: `IndexError: list index out of range
    at getFirstItem (/app/src/utils/array.ts:12:9)
    at processResults (/app/src/handlers/search.ts:45:18)`
		}
	],
	'inc-5': [
		{
			id: 'sample-5a',
			incident_id: 'inc-5',
			timestamp: '2024-01-22T10:00:00Z',
			raw_log: 'TimeoutError: Database connection timed out after 30000ms',
			stack_trace: `TimeoutError: Database connection timed out after 30000ms
    at Pool.connect (/app/node_modules/pg/lib/pool.js:85:11)
    at async DatabaseClient.query (/app/src/db/client.ts:42:5)`
		}
	],
	'inc-6': [
		{
			id: 'sample-6a',
			incident_id: 'inc-6',
			timestamp: '2024-01-21T18:45:00Z',
			raw_log: 'SMTPError: Connection refused to mail server at smtp.example.com:587',
			stack_trace: `SMTPError: Connection refused to mail server at smtp.example.com:587
    at SMTPClient.connect (/app/src/email/smtp.ts:34:9)
    at async sendNotification (/app/src/services/notification.ts:67:5)`
		}
	],
	'inc-8': [
		{
			id: 'sample-8a',
			incident_id: 'inc-8',
			timestamp: '2024-01-22T11:00:00Z',
			raw_log: "SyntaxError: Unexpected token '<' in JSON at position 0",
			stack_trace: `SyntaxError: Unexpected token '<' in JSON at position 0
    at JSON.parse (<anonymous>)
    at ApiClient.parseResponse (/app/src/api/client.ts:89:22)
    at async ApiClient.fetch (/app/src/api/client.ts:45:12)`,
			metadata: { endpoint: '/api/users', status: 502 }
		}
	]
};

// Mock activity logs for incidents
export const mockActivityLogs: Record<string, Array<{ id: string; incident_id: string; action: string; details?: string; created_at: string }>> = {
	'inc-1': [
		{ id: 'act-1a', incident_id: 'inc-1', action: 'Incident detected', details: 'First occurrence captured', created_at: '2024-01-20T10:00:00Z' },
		{ id: 'act-1b', incident_id: 'inc-1', action: 'Triage started', created_at: '2024-01-20T10:01:00Z' },
		{ id: 'act-1c', incident_id: 'inc-1', action: 'Triage complete', details: 'Fixable with 92% confidence', created_at: '2024-01-20T10:02:30Z' },
		{ id: 'act-1d', incident_id: 'inc-1', action: 'Fix generation started', created_at: '2024-01-20T10:03:00Z' },
		{ id: 'act-1e', incident_id: 'inc-1', action: 'Fix generated', details: 'Added null check for req.user', created_at: '2024-01-20T10:05:45Z' },
		{ id: 'act-1f', incident_id: 'inc-1', action: 'PR created', details: 'PR #142 opened on acme/payments-api', created_at: '2024-01-20T10:06:00Z' }
	],
	'inc-2': [
		{ id: 'act-2a', incident_id: 'inc-2', action: 'Incident detected', created_at: '2024-01-21T08:00:00Z' },
		{ id: 'act-2b', incident_id: 'inc-2', action: 'Triage started', created_at: '2024-01-21T08:01:00Z' },
		{ id: 'act-2c', incident_id: 'inc-2', action: 'Triage complete', details: 'Fixable with 88% confidence', created_at: '2024-01-21T08:02:15Z' },
		{ id: 'act-2d', incident_id: 'inc-2', action: 'Fix generation started', created_at: '2024-01-22T13:00:00Z' }
	],
	'inc-3': [
		{ id: 'act-3a', incident_id: 'inc-3', action: 'Incident detected', created_at: '2024-01-19T15:00:00Z' },
		{ id: 'act-3b', incident_id: 'inc-3', action: 'Triage complete', details: 'Missing import - high confidence fix', created_at: '2024-01-19T15:01:30Z' },
		{ id: 'act-3c', incident_id: 'inc-3', action: 'PR created', details: 'PR #89 opened', created_at: '2024-01-19T15:03:00Z' },
		{ id: 'act-3d', incident_id: 'inc-3', action: 'PR merged', details: 'Fix deployed to production', created_at: '2024-01-19T16:30:00Z' },
		{ id: 'act-3e', incident_id: 'inc-3', action: 'Verification started', details: 'Round 1 of 3 (1 hour)', created_at: '2024-01-19T16:30:00Z' },
		{ id: 'act-3f', incident_id: 'inc-3', action: 'Verification round passed', details: 'Round 1: No ReferenceError occurrences in 1 hour', created_at: '2024-01-19T17:30:00Z' },
		{ id: 'act-3g', incident_id: 'inc-3', action: 'Verification in progress', details: 'Round 2: Monitoring for 6 hours', created_at: '2024-01-19T17:30:00Z' }
	],
	'inc-4': [
		{ id: 'act-4a', incident_id: 'inc-4', action: 'Incident detected', created_at: '2024-01-22T11:00:00Z' },
		{ id: 'act-4b', incident_id: 'inc-4', action: 'Triage started', created_at: '2024-01-22T12:00:00Z' }
	],
	'inc-5': [
		{ id: 'act-5a', incident_id: 'inc-5', action: 'Incident detected', created_at: '2024-01-18T09:00:00Z' },
		{ id: 'act-5b', incident_id: 'inc-5', action: 'Triage complete', details: 'Infrastructure issue - not fixable via code', created_at: '2024-01-18T09:02:00Z' },
		{ id: 'act-5c', incident_id: 'inc-5', action: 'Marked as unfixable', details: 'Requires infrastructure team intervention', created_at: '2024-01-18T09:02:30Z' }
	],
	'inc-6': [
		{ id: 'act-6a', incident_id: 'inc-6', action: 'Incident detected', details: 'Awaiting triage', created_at: '2024-01-21T14:00:00Z' }
	],
	'inc-8': [
		{ id: 'act-8a', incident_id: 'inc-8', action: 'Incident detected', created_at: '2024-01-22T09:00:00Z' },
		{ id: 'act-8b', incident_id: 'inc-8', action: 'Triage complete', details: 'JSON parse error - fixable', created_at: '2024-01-22T09:01:45Z' },
		{ id: 'act-8c', incident_id: 'inc-8', action: 'PR created', details: 'PR #91 opened', created_at: '2024-01-22T09:05:00Z' }
	],
	'inc-9': [
		{ id: 'act-9a', incident_id: 'inc-9', action: 'Incident detected', details: 'First occurrence captured', created_at: '2024-01-15T08:00:00Z' },
		{ id: 'act-9b', incident_id: 'inc-9', action: 'Triage complete', details: 'Session null check - 91% confidence', created_at: '2024-01-15T08:02:00Z' },
		{ id: 'act-9c', incident_id: 'inc-9', action: 'PR created', details: 'PR #138 opened on acme/payments-api', created_at: '2024-01-20T10:00:00Z' },
		{ id: 'act-9d', incident_id: 'inc-9', action: 'PR merged', details: 'Fix deployed to production', created_at: '2024-01-20T16:00:00Z' },
		{ id: 'act-9e', incident_id: 'inc-9', action: 'Verification started', details: 'Round 1 of 3 (1 hour)', created_at: '2024-01-20T16:00:00Z' },
		{ id: 'act-9f', incident_id: 'inc-9', action: 'Verification round passed', details: 'Round 1: No new occurrences in 1 hour', created_at: '2024-01-20T17:00:00Z' },
		{ id: 'act-9g', incident_id: 'inc-9', action: 'Verification round passed', details: 'Round 2: No new occurrences in 6 hours', created_at: '2024-01-20T23:00:00Z' },
		{ id: 'act-9h', incident_id: 'inc-9', action: 'Verification in progress', details: 'Round 3: Monitoring for 24 hours - Expected: Invalid sessions return 401 instead of NPE', created_at: '2024-01-20T23:00:00Z' }
	],
	'inc-10': [
		{ id: 'act-10a', incident_id: 'inc-10', action: 'Incident detected', created_at: '2024-01-10T10:00:00Z' },
		{ id: 'act-10b', incident_id: 'inc-10', action: 'Triage complete', details: 'Array undefined access - 94% confidence', created_at: '2024-01-10T10:01:30Z' },
		{ id: 'act-10c', incident_id: 'inc-10', action: 'PR created', details: 'PR #85 opened', created_at: '2024-01-12T09:00:00Z' },
		{ id: 'act-10d', incident_id: 'inc-10', action: 'PR merged', details: 'Fix deployed to production', created_at: '2024-01-12T14:00:00Z' },
		{ id: 'act-10e', incident_id: 'inc-10', action: 'Verification started', details: 'Round 1 of 3', created_at: '2024-01-12T14:00:00Z' },
		{ id: 'act-10f', incident_id: 'inc-10', action: 'Verification round passed', details: 'Round 1: Confirmed empty state renders correctly', created_at: '2024-01-12T15:00:00Z' },
		{ id: 'act-10g', incident_id: 'inc-10', action: 'Verification round passed', details: 'Round 2: No errors during 6-hour monitoring', created_at: '2024-01-12T21:00:00Z' },
		{ id: 'act-10h', incident_id: 'inc-10', action: 'Verification complete', details: 'All 3 rounds passed - Fix verified working as expected', created_at: '2024-01-13T14:00:00Z' }
	],
	'inc-11': [
		{ id: 'act-11a', incident_id: 'inc-11', action: 'Incident detected', created_at: '2024-01-18T11:00:00Z' },
		{ id: 'act-11b', incident_id: 'inc-11', action: 'Triage complete', details: 'Connection handling issue - 72% confidence', created_at: '2024-01-18T11:02:00Z' },
		{ id: 'act-11c', incident_id: 'inc-11', action: 'PR created', details: 'PR #140 opened', created_at: '2024-01-21T14:00:00Z' },
		{ id: 'act-11d', incident_id: 'inc-11', action: 'PR merged', details: 'Fix deployed to production', created_at: '2024-01-21T17:00:00Z' },
		{ id: 'act-11e', incident_id: 'inc-11', action: 'Verification started', details: 'Round 1 of 3 - Monitoring for transient retries', created_at: '2024-01-21T17:00:00Z' }
	],
	'inc-12': [
		{ id: 'act-12a', incident_id: 'inc-12', action: 'Incident detected', created_at: '2024-01-17T09:00:00Z' },
		{ id: 'act-12b', incident_id: 'inc-12', action: 'Triage complete', details: 'Rate limiting needed - 88% confidence', created_at: '2024-01-17T09:02:00Z' },
		{ id: 'act-12c', incident_id: 'inc-12', action: 'PR created', details: 'PR #45 opened', created_at: '2024-01-22T08:00:00Z' },
		{ id: 'act-12d', incident_id: 'inc-12', action: 'PR merged', details: 'Fix deployed to production', created_at: '2024-01-22T10:00:00Z' },
		{ id: 'act-12e', incident_id: 'inc-12', action: 'Pending verification', details: 'Waiting for verification slot - Expected: Notifications queue within rate limits', created_at: '2024-01-22T10:00:00Z' }
	]
};

// Get mock incident with full details (samples + activity)
export function getMockIncidentDetails(id: string) {
	const incident = mockIncidents.find(i => i.id === id);
	if (!incident) return null;

	return {
		...incident,
		project: mockProjects.find(p => p.id === incident.project_id),
		samples: mockSamples[id] || [],
		activity: mockActivityLogs[id] || []
	};
}

// Mock generated tests for incidents
export const mockGeneratedTests: Record<string, GeneratedTest[]> = {
	'inc-1': [
		{
			id: 'test-1a',
			incident_id: 'inc-1',
			file_path: 'src/handlers/__tests__/payment.test.ts',
			content: `import { processRefund } from '../payment';
import { createMockRequest, createMockResponse } from '../../test-utils';

describe('processRefund', () => {
  it('should return 401 when user session is missing', async () => {
    const req = createMockRequest({ user: undefined });
    const res = createMockResponse();

    await processRefund(req, res);

    expect(res.status).toHaveBeenCalledWith(401);
    expect(res.json).toHaveBeenCalledWith({
      error: 'Unauthorized',
      message: 'User session required'
    });
  });

  it('should return 401 when user object exists but userId is undefined', async () => {
    const req = createMockRequest({ user: {} });
    const res = createMockResponse();

    await processRefund(req, res);

    expect(res.status).toHaveBeenCalledWith(401);
  });

  it('should process refund successfully with valid user session', async () => {
    const req = createMockRequest({
      user: { userId: 'user-123' },
      body: { orderId: 'order-456', amount: 50.00 }
    });
    const res = createMockResponse();

    await processRefund(req, res);

    expect(res.status).toHaveBeenCalledWith(200);
  });
});`,
			description: 'Tests for null/undefined user session handling in processRefund',
			framework: 'jest',
			included_in_pr: true,
			commit_sha: 'abc123def',
			created_at: '2024-01-20T10:05:00Z'
		}
	],
	'inc-2': [
		{
			id: 'test-2a',
			incident_id: 'inc-2',
			file_path: 'src/validators/__tests__/amount.test.ts',
			content: `import { validateAmount } from '../amount';

describe('validateAmount', () => {
  it('should accept positive numbers', () => {
    expect(() => validateAmount(100)).not.toThrow();
    expect(() => validateAmount(0.01)).not.toThrow();
  });

  it('should reject negative numbers', () => {
    expect(() => validateAmount(-50)).toThrow('Amount must be a positive number');
  });

  it('should reject zero', () => {
    expect(() => validateAmount(0)).toThrow('Amount must be a positive number');
  });

  it('should coerce string numbers to numbers', () => {
    expect(() => validateAmount('100' as any)).not.toThrow();
    expect(() => validateAmount('-50' as any)).toThrow();
  });

  it('should reject non-numeric strings', () => {
    expect(() => validateAmount('abc' as any)).toThrow();
  });
});`,
			description: 'Tests for amount validation including type coercion',
			framework: 'jest',
			included_in_pr: false,
			created_at: '2024-01-21T08:02:00Z'
		}
	],
	'inc-3': [
		{
			id: 'test-3a',
			incident_id: 'inc-3',
			file_path: 'src/controllers/__tests__/user.test.ts',
			content: `import { UserController } from '../user';
import { getProfile } from '../../utils/profile';

jest.mock('../../utils/profile');

describe('UserController', () => {
  const controller = new UserController();

  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe('show', () => {
    it('should call getProfile with user id', async () => {
      const mockProfile = { id: 'user-1', name: 'Test User' };
      (getProfile as jest.Mock).mockResolvedValue(mockProfile);

      const result = await controller.show('user-1');

      expect(getProfile).toHaveBeenCalledWith('user-1');
      expect(result).toEqual(mockProfile);
    });

    it('should throw when user not found', async () => {
      (getProfile as jest.Mock).mockResolvedValue(null);

      await expect(controller.show('nonexistent')).rejects.toThrow('User not found');
    });
  });
});`,
			description: 'Tests for UserController.show method with getProfile import',
			framework: 'jest',
			included_in_pr: true,
			commit_sha: 'def456ghi',
			created_at: '2024-01-19T15:02:00Z'
		}
	],
	'inc-8': [
		{
			id: 'test-8a',
			incident_id: 'inc-8',
			file_path: 'src/api/__tests__/client.test.ts',
			content: `import { ApiClient } from '../client';

describe('ApiClient', () => {
  let client: ApiClient;

  beforeEach(() => {
    client = new ApiClient('https://api.example.com');
    global.fetch = jest.fn();
  });

  describe('parseResponse', () => {
    it('should parse JSON responses correctly', async () => {
      const mockResponse = {
        ok: true,
        headers: new Headers({ 'content-type': 'application/json' }),
        json: jest.fn().mockResolvedValue({ data: 'test' })
      };

      const result = await client.parseResponse(mockResponse as any);

      expect(result).toEqual({ data: 'test' });
    });

    it('should throw ApiError for HTML responses', async () => {
      const mockResponse = {
        ok: false,
        status: 502,
        headers: new Headers({ 'content-type': 'text/html' }),
        text: jest.fn().mockResolvedValue('<!DOCTYPE html><html>Error</html>')
      };

      await expect(client.parseResponse(mockResponse as any))
        .rejects.toThrow('Received non-JSON response (status: 502)');
    });

    it('should handle missing content-type header', async () => {
      const mockResponse = {
        ok: true,
        headers: new Headers(),
        json: jest.fn().mockResolvedValue({ data: 'test' })
      };

      const result = await client.parseResponse(mockResponse as any);
      expect(result).toEqual({ data: 'test' });
    });
  });
});`,
			description: 'Tests for API client response parsing with content-type validation',
			framework: 'jest',
			included_in_pr: true,
			commit_sha: 'ghi789jkl',
			created_at: '2024-01-22T09:04:00Z'
		},
		{
			id: 'test-8b',
			incident_id: 'inc-8',
			file_path: 'src/api/__tests__/client.integration.test.ts',
			content: `import { ApiClient } from '../client';
import nock from 'nock';

describe('ApiClient Integration', () => {
  let client: ApiClient;
  const baseUrl = 'https://api.example.com';

  beforeEach(() => {
    client = new ApiClient(baseUrl);
    nock.cleanAll();
  });

  afterAll(() => {
    nock.restore();
  });

  it('should handle 502 Bad Gateway with HTML response gracefully', async () => {
    nock(baseUrl)
      .get('/users')
      .reply(502, '<!DOCTYPE html><html><body>Bad Gateway</body></html>', {
        'Content-Type': 'text/html'
      });

    await expect(client.fetch('/users')).rejects.toMatchObject({
      message: expect.stringContaining('non-JSON response'),
      statusCode: 502
    });
  });

  it('should succeed with valid JSON response', async () => {
    nock(baseUrl)
      .get('/users')
      .reply(200, { users: [] }, {
        'Content-Type': 'application/json'
      });

    const result = await client.fetch('/users');
    expect(result).toEqual({ users: [] });
  });
});`,
			description: 'Integration tests for API client error handling',
			framework: 'jest',
			included_in_pr: false,
			created_at: '2024-01-22T09:04:30Z'
		}
	]
};

// Get mock generated tests for an incident
export function getMockGeneratedTests(incidentId: string): GeneratedTest[] {
	return mockGeneratedTests[incidentId] || [];
}

// Mock verification rounds data
export const mockVerificationRounds: Record<string, VerificationSummary> = {
	'inc-3': {
		incident_id: 'inc-3',
		status: 'verifying',
		current_round: 2,
		total_rounds: 3,
		started_at: '2024-01-19T16:30:00Z',
		rounds: [
			{
				id: 'vr-3a',
				incident_id: 'inc-3',
				round_number: 1,
				started_at: '2024-01-19T16:30:00Z',
				completed_at: '2024-01-19T17:30:00Z',
				status: 'passed',
				occurrence_count_start: 56,
				occurrence_count_end: 56,
				duration_hours: 1
			},
			{
				id: 'vr-3b',
				incident_id: 'inc-3',
				round_number: 2,
				started_at: '2024-01-19T17:30:00Z',
				status: 'monitoring',
				occurrence_count_start: 56,
				duration_hours: 6
			}
		]
	},
	'inc-9': {
		incident_id: 'inc-9',
		status: 'verifying',
		current_round: 3,
		total_rounds: 3,
		started_at: '2024-01-20T16:00:00Z',
		rounds: [
			{
				id: 'vr-9a',
				incident_id: 'inc-9',
				round_number: 1,
				started_at: '2024-01-20T16:00:00Z',
				completed_at: '2024-01-20T17:00:00Z',
				status: 'passed',
				occurrence_count_start: 312,
				occurrence_count_end: 312,
				duration_hours: 1
			},
			{
				id: 'vr-9b',
				incident_id: 'inc-9',
				round_number: 2,
				started_at: '2024-01-20T17:00:00Z',
				completed_at: '2024-01-20T23:00:00Z',
				status: 'passed',
				occurrence_count_start: 312,
				occurrence_count_end: 312,
				duration_hours: 6
			},
			{
				id: 'vr-9c',
				incident_id: 'inc-9',
				round_number: 3,
				started_at: '2024-01-20T23:00:00Z',
				status: 'monitoring',
				occurrence_count_start: 312,
				duration_hours: 24
			}
		]
	},
	'inc-10': {
		incident_id: 'inc-10',
		status: 'verified',
		current_round: 3,
		total_rounds: 3,
		started_at: '2024-01-12T14:00:00Z',
		verified_at: '2024-01-13T14:00:00Z',
		rounds: [
			{
				id: 'vr-10a',
				incident_id: 'inc-10',
				round_number: 1,
				started_at: '2024-01-12T14:00:00Z',
				completed_at: '2024-01-12T15:00:00Z',
				status: 'passed',
				occurrence_count_start: 89,
				occurrence_count_end: 89,
				duration_hours: 1
			},
			{
				id: 'vr-10b',
				incident_id: 'inc-10',
				round_number: 2,
				started_at: '2024-01-12T15:00:00Z',
				completed_at: '2024-01-12T21:00:00Z',
				status: 'passed',
				occurrence_count_start: 89,
				occurrence_count_end: 89,
				duration_hours: 6
			},
			{
				id: 'vr-10c',
				incident_id: 'inc-10',
				round_number: 3,
				started_at: '2024-01-12T21:00:00Z',
				completed_at: '2024-01-13T14:00:00Z',
				status: 'passed',
				occurrence_count_start: 89,
				occurrence_count_end: 89,
				duration_hours: 24
			}
		]
	},
	'inc-11': {
		incident_id: 'inc-11',
		status: 'verifying',
		current_round: 1,
		total_rounds: 3,
		started_at: '2024-01-21T17:00:00Z',
		rounds: [
			{
				id: 'vr-11a',
				incident_id: 'inc-11',
				round_number: 1,
				started_at: '2024-01-21T17:00:00Z',
				status: 'monitoring',
				occurrence_count_start: 156,
				duration_hours: 1
			}
		]
	},
	'inc-12': {
		incident_id: 'inc-12',
		status: 'not_started',
		current_round: 0,
		total_rounds: 3,
		started_at: '2024-01-22T10:00:00Z',
		rounds: []
	}
};

// Get mock verification status for an incident
export function getMockVerificationStatus(incidentId: string): VerificationSummary {
	return mockVerificationRounds[incidentId] || {
		incident_id: incidentId,
		status: 'not_started',
		current_round: 0,
		total_rounds: 3,
		rounds: []
	};
}

// Get all incidents currently in verification (for the verification panel)
export function getMockVerifyingIncidents(): Array<Incident & { verification: VerificationSummary }> {
	const verifyingStatuses = ['verifying', 'pending_verification'];
	return mockIncidents
		.filter(i => verifyingStatuses.includes(i.status) || i.verification_status === 'verifying')
		.map(incident => ({
			...incident,
			project: mockProjects.find(p => p.id === incident.project_id),
			verification: getMockVerificationStatus(incident.id)
		}));
}

// Usage details for charts
export const mockUsageDetails = [
	{ date: '2024-01-16', provider: 'anthropic', model: 'claude-3-haiku', purpose: 'triage', requests: 45, input_tokens: 89000, output_tokens: 12000, cost: 0.89 },
	{ date: '2024-01-16', provider: 'anthropic', model: 'claude-sonnet-4', purpose: 'fix', requests: 12, input_tokens: 156000, output_tokens: 45000, cost: 4.23 },
	{ date: '2024-01-17', provider: 'anthropic', model: 'claude-3-haiku', purpose: 'triage', requests: 52, input_tokens: 102000, output_tokens: 14000, cost: 1.02 },
	{ date: '2024-01-17', provider: 'anthropic', model: 'claude-sonnet-4', purpose: 'fix', requests: 15, input_tokens: 189000, output_tokens: 52000, cost: 5.12 },
	{ date: '2024-01-18', provider: 'anthropic', model: 'claude-3-haiku', purpose: 'triage', requests: 38, input_tokens: 76000, output_tokens: 10000, cost: 0.76 },
	{ date: '2024-01-18', provider: 'anthropic', model: 'claude-sonnet-4', purpose: 'fix', requests: 8, input_tokens: 104000, output_tokens: 28000, cost: 2.78 },
	{ date: '2024-01-19', provider: 'anthropic', model: 'claude-3-haiku', purpose: 'triage', requests: 61, input_tokens: 122000, output_tokens: 17000, cost: 1.22 },
	{ date: '2024-01-19', provider: 'anthropic', model: 'claude-sonnet-4', purpose: 'fix', requests: 18, input_tokens: 234000, output_tokens: 62000, cost: 6.24 },
	{ date: '2024-01-20', provider: 'anthropic', model: 'claude-3-haiku', purpose: 'triage', requests: 73, input_tokens: 146000, output_tokens: 20000, cost: 1.46 },
	{ date: '2024-01-20', provider: 'anthropic', model: 'claude-sonnet-4', purpose: 'fix', requests: 21, input_tokens: 273000, output_tokens: 71000, cost: 7.28 },
	{ date: '2024-01-21', provider: 'anthropic', model: 'claude-3-haiku', purpose: 'triage', requests: 89, input_tokens: 178000, output_tokens: 25000, cost: 1.78 },
	{ date: '2024-01-21', provider: 'anthropic', model: 'claude-sonnet-4', purpose: 'fix', requests: 24, input_tokens: 312000, output_tokens: 84000, cost: 8.32 },
	{ date: '2024-01-22', provider: 'anthropic', model: 'claude-3-haiku', purpose: 'triage', requests: 67, input_tokens: 134000, output_tokens: 18000, cost: 1.34 },
	{ date: '2024-01-22', provider: 'anthropic', model: 'claude-sonnet-4', purpose: 'fix', requests: 14, input_tokens: 182000, output_tokens: 48000, cost: 4.85 },
];

// Mock external issues for demo mode
export function getMockExternalIssues(incidentId: string): Array<{
	id: string;
	incident_id: string;
	provider: 'github' | 'jira' | 'linear' | 'webhook';
	external_id: string;
	external_url: string;
	external_status: string;
	last_synced_at: string;
	sync_error: string | null;
	created_at: string;
}> {
	// Find the incident to determine its state
	const incident = mockIncidents.find(i => i.id === incidentId);
	if (!incident) return [];

	const issues = [];
	const now = new Date();

	// GitHub issue - always present for incidents that have progressed past triaging
	if (['pr_open', 'pr_merged', 'verifying', 'verified', 'fixing', 'pending_fix'].includes(incident.status)) {
		const issueNumber = Math.floor(Math.random() * 500) + 100;
		issues.push({
			id: `ext-gh-${incidentId}`,
			incident_id: incidentId,
			provider: 'github' as const,
			external_id: `#${issueNumber}`,
			external_url: `https://github.com/acme/api/issues/${issueNumber}`,
			external_status: incident.status === 'verified' ? 'closed' : 'open',
			last_synced_at: new Date(now.getTime() - Math.random() * 3600000).toISOString(),
			sync_error: null,
			created_at: incident.first_seen
		});
	}

	// For some incidents, also show a Jira issue
	if (incidentId === 'inc-2' || incidentId === 'inc-5') {
		issues.push({
			id: `ext-jira-${incidentId}`,
			incident_id: incidentId,
			provider: 'jira' as const,
			external_id: 'CRISPR-' + (Math.floor(Math.random() * 100) + 200),
			external_url: 'https://acme.atlassian.net/browse/CRISPR-' + (Math.floor(Math.random() * 100) + 200),
			external_status: incident.status === 'verified' ? 'Done' : 'In Progress',
			last_synced_at: new Date(now.getTime() - Math.random() * 7200000).toISOString(),
			sync_error: null,
			created_at: incident.first_seen
		});
	}

	return issues;
}
