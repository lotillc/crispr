// CRISPR API Client

const BASE_URL = '';

// Types
export interface Project {
	id: string;
	repo: string;
	enabled: boolean;
	auto_fix: boolean;
	watch_logs: boolean;
	created_at: string;
	incident_count?: number;
	last_incident?: string;
}

export interface Incident {
	id: string;
	project_id: string;
	fingerprint: string;
	level: string;
	message: string;
	status: IncidentStatus;
	sample_count: number;
	first_seen: string;
	last_seen: string;
	fix_attempts: number;
	pr_url?: string;
	pr_number?: number;
	triage_result?: TriageResult;
	project?: Project;
}

export type IncidentStatus =
	| 'detected'
	| 'triaging'
	| 'unfixable'
	| 'pending_fix'
	| 'fixing'
	| 'pr_open'
	| 'pr_merged'
	| 'ignored'
	| 'max_attempts';

export interface TriageResult {
	fixable: boolean;
	confidence: number;
	reason: string;
	category: string;
	suggested_files: string[];
	description: string;
	fix_approach?: string;
	what_changed?: string;
	expected_behavior?: string;
}

export interface Sample {
	id: string;
	incident_id: string;
	timestamp: string;
	raw_log: string;
	stack_trace?: string;
	metadata?: Record<string, any>;
}

export interface ActivityLog {
	id: string;
	incident_id: string;
	action: string;
	details?: string;
	created_at: string;
}

export interface LlmConfig {
	reader_provider?: string;
	reader_model?: string;
	reader_base_url?: string;
	writer_provider?: string;
	writer_model?: string;
	writer_base_url?: string;
}

export interface LlmModel {
	id: string;
	name: string;
}

export interface AuthStatus {
	github: {
		connected: boolean;
		mode: string;
		oauth_url?: string;
		user?: {
			login: string;
			name?: string;
			avatar_url: string;
		};
	};
	slack: {
		enabled: boolean;
		connected: boolean;
		team_name?: string;
		channel_name?: string;
	};
	llm: {
		configured: boolean;
		reader_model?: string;
		writer_model?: string;
	};
}

export interface SlackChannel {
	id: string;
	name: string;
	is_private: boolean;
	is_member: boolean;
}

export interface UsageSummary {
	total_requests: number;
	total_input_tokens: number;
	total_output_tokens: number;
	total_cost: number;
	by_provider: Record<string, ProviderUsage>;
	by_purpose: Record<string, PurposeUsage>;
}

export interface ProviderUsage {
	requests: number;
	input_tokens: number;
	output_tokens: number;
	cost: number;
}

export interface PurposeUsage {
	requests: number;
	cost: number;
}

export interface IncidentStats {
	total: number;
	by_status: Record<string, number>;
	fixed_today: number;
	fixed_week: number;
	avg_fix_time_hours?: number;
}

export interface GeneratedTest {
	id: string;
	incident_id: string;
	file_path: string;
	content: string;
	description?: string;
	framework: string;
	included_in_pr: boolean;
	commit_sha?: string;
	created_at: string;
}

export interface ProjectStats {
	incident_count: number;
	open_incidents: number;
	fixed_incidents: number;
	total_cost: number;
}

// API Functions

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
	const response = await fetch(`${BASE_URL}${path}`, {
		...options,
		headers: {
			'Content-Type': 'application/json',
			...options.headers,
		},
	});

	if (!response.ok) {
		const text = await response.text();
		// Try to parse as JSON error
		try {
			const json = JSON.parse(text);
			throw new Error(json.error || json.message || text);
		} catch {
			throw new Error(text || response.statusText);
		}
	}

	const text = await response.text();
	if (!text) {
		return {} as T;
	}
	try {
		return JSON.parse(text);
	} catch {
		throw new Error(`Invalid JSON response: ${text.substring(0, 100)}`);
	}
}

// Status
export async function getStatus(): Promise<{
	status: string;
	version: string;
	environment: string;
	display_name: string;
	llm_configured: boolean;
	github_connected: boolean;
}> {
	return request('/api/v1/status');
}

// Auth
export async function getAuthStatus(): Promise<AuthStatus> {
	return request('/api/v1/auth/status');
}

export function getGithubAuthUrl(): string {
	return `${BASE_URL}/api/v1/auth/github`;
}

export function getSlackAuthUrl(): string {
	return `${BASE_URL}/api/v1/auth/slack`;
}

export async function getSlackChannels(): Promise<SlackChannel[]> {
	return request('/api/v1/auth/slack/channels');
}

export async function setSlackChannel(channelId: string, channelName: string): Promise<{ success: boolean; message: string }> {
	return request('/api/v1/auth/slack/channel', {
		method: 'POST',
		body: JSON.stringify({ channel_id: channelId, channel_name: channelName }),
	});
}

export async function testSlackConnection(): Promise<{ success: boolean; message: string }> {
	return request('/api/v1/auth/slack/test', { method: 'POST' });
}

export async function disconnectSlack(): Promise<{ success: boolean; message: string }> {
	return request('/api/v1/auth/slack', { method: 'DELETE' });
}

export async function disconnectGithub(): Promise<{ success: boolean; message: string }> {
	return request('/api/v1/auth/github', { method: 'DELETE' });
}

// Projects
export async function getProjects(): Promise<Project[]> {
	return request('/api/v1/projects');
}

export async function getProject(id: string): Promise<Project> {
	return request(`/api/v1/projects/${id}`);
}

export async function createProject(repo: string): Promise<Project> {
	return request('/api/v1/projects', {
		method: 'POST',
		body: JSON.stringify({ repo }),
	});
}

export async function updateProject(id: string, updates: Partial<Project>): Promise<Project> {
	return request(`/api/v1/projects/${id}`, {
		method: 'PATCH',
		body: JSON.stringify(updates),
	});
}

export async function deleteProject(id: string): Promise<void> {
	await request(`/api/v1/projects/${id}`, { method: 'DELETE' });
}

export async function getProjectStats(id: string): Promise<ProjectStats> {
	return request(`/api/v1/projects/${id}/stats`);
}

// Incidents
export async function getIncidents(params?: {
	project_id?: string;
	status?: string;
	limit?: number;
	offset?: number;
}): Promise<Incident[]> {
	const query = new URLSearchParams();
	if (params?.project_id) query.set('project_id', params.project_id);
	if (params?.status) query.set('status', params.status);
	if (params?.limit) query.set('limit', params.limit.toString());
	if (params?.offset) query.set('offset', params.offset.toString());

	const queryStr = query.toString();
	return request(`/api/v1/incidents${queryStr ? `?${queryStr}` : ''}`);
}

export async function getIncident(id: string): Promise<Incident & {
	samples: Sample[];
	activity: ActivityLog[];
}> {
	return request(`/api/v1/incidents/${id}`);
}

export async function retryIncident(id: string): Promise<void> {
	await request(`/api/v1/incidents/${id}/retry`, { method: 'POST' });
}

export async function ignoreIncident(id: string): Promise<void> {
	await request(`/api/v1/incidents/${id}/ignore`, { method: 'POST' });
}

export async function getIncidentStats(): Promise<IncidentStats> {
	return request('/api/v1/incidents/stats');
}

// Generated Tests
export async function getGeneratedTests(incidentId: string): Promise<GeneratedTest[]> {
	return request(`/api/v1/incidents/${incidentId}/tests`);
}

export async function updateGeneratedTest(
	incidentId: string,
	testId: string,
	updates: { included_in_pr?: boolean }
): Promise<GeneratedTest> {
	return request(`/api/v1/incidents/${incidentId}/tests/${testId}`, {
		method: 'PATCH',
		body: JSON.stringify(updates),
	});
}

// LLM Settings
export async function getLlmConfig(): Promise<LlmConfig> {
	return request('/api/v1/settings/llm');
}

export async function updateLlmConfig(config: {
	provider: string;
	api_key?: string;
	model?: string;
	base_url?: string;
	role: 'reader' | 'writer';
}): Promise<void> {
	await request('/api/v1/settings/llm', {
		method: 'PUT',
		body: JSON.stringify(config),
	});
}

export async function verifyLlm(config: {
	provider: string;
	api_key?: string;
	model?: string;
	base_url?: string;
}): Promise<{ success: boolean; error?: string }> {
	return request('/api/v1/settings/llm/verify', {
		method: 'POST',
		body: JSON.stringify(config),
	});
}

export async function listLlmModels(config: {
	provider: string;
	api_key?: string;
	base_url?: string;
}): Promise<LlmModel[]> {
	return request('/api/v1/settings/llm/models', {
		method: 'POST',
		body: JSON.stringify(config),
	});
}

// Usage
export async function getUsageSummary(params?: {
	start_date?: string;
	end_date?: string;
}): Promise<UsageSummary> {
	const query = new URLSearchParams();
	if (params?.start_date) query.set('start_date', params.start_date);
	if (params?.end_date) query.set('end_date', params.end_date);

	const queryStr = query.toString();
	return request(`/api/v1/usage/summary${queryStr ? `?${queryStr}` : ''}`);
}

export async function getUsageDetails(params?: {
	start_date?: string;
	end_date?: string;
	project_id?: string;
}): Promise<Array<{
	date: string;
	provider: string;
	model: string;
	purpose: string;
	requests: number;
	input_tokens: number;
	output_tokens: number;
	cost: number;
}>> {
	const query = new URLSearchParams();
	if (params?.start_date) query.set('start_date', params.start_date);
	if (params?.end_date) query.set('end_date', params.end_date);
	if (params?.project_id) query.set('project_id', params.project_id);

	const queryStr = query.toString();
	return request(`/api/v1/usage/details${queryStr ? `?${queryStr}` : ''}`);
}

// Webhook Settings
export interface WebhookSettings {
	configured: boolean;
	sentry: WebhookProviderSettings;
	datadog: WebhookProviderSettings;
	grafana: WebhookProviderSettings;
}

export interface WebhookProviderSettings {
	enabled: boolean;
	has_secret: boolean;
}

export interface UpdateWebhookSettingsRequest {
	sentry_enabled?: boolean;
	sentry_secret?: string;
	datadog_enabled?: boolean;
	datadog_token?: string;
	grafana_enabled?: boolean;
	grafana_token?: string;
}

export async function getWebhookSettings(): Promise<WebhookSettings> {
	return request('/api/v1/settings/webhooks');
}

export async function updateWebhookSettings(
	settings: UpdateWebhookSettingsRequest
): Promise<WebhookSettings> {
	return request('/api/v1/settings/webhooks', {
		method: 'PUT',
		body: JSON.stringify(settings)
	});
}
