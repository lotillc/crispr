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
