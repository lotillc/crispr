// Mock data for demo mode
import type { Project, Incident, AuthStatus, UsageSummary, IncidentStats } from './api';

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
		status: 'pr_merged',
		sample_count: 56,
		first_seen: '2024-01-19T15:00:00Z',
		last_seen: '2024-01-19T15:30:00Z',
		fix_attempts: 1,
		pr_url: 'https://github.com/acme/user-service/pull/89',
		pr_number: 89,
		triage_result: {
			fixable: true,
			confidence: 0.95,
			reason: 'Missing import statement',
			category: 'missing_import',
			suggested_files: ['src/controllers/user.ts'],
			description: 'Function not imported from utils module',
			fix_approach: 'Add missing import statement'
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
	total: 47,
	by_status: {
		detected: 3,
		triaging: 2,
		fixing: 1,
		pr_open: 4,
		pr_merged: 28,
		unfixable: 5,
		ignored: 4
	},
	fixed_today: 3,
	fixed_week: 12,
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
		{ id: 'act-3d', incident_id: 'inc-3', action: 'PR merged', details: 'Fix deployed to production', created_at: '2024-01-19T16:30:00Z' }
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
