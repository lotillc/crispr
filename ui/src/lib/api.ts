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
	pr_state?: string;
	pr_merged_at?: string;
	triage_result?: TriageResult;
	project?: Project;
	// Queue tracking
	queued_at?: string;
	queue_position?: number;
	// Verification tracking
	verification_round?: number;
	verification_status?: 'verifying' | 'verified' | 'recurred';
	verified_at?: string;
	verification_duration_hours?: number;  // LLM-recommended monitoring duration
	verification_started_at?: string;
	verification_ends_at?: string;
	recurrence_detected_at?: string;
	recurrence_count?: number;  // New occurrences since fix was merged
}

export type IncidentStatus =
	| 'detected'
	| 'triaging'
	| 'unfixable'
	| 'pending_fix'
	| 'fixing'
	| 'pr_opening'
	| 'pr_open'
	| 'pr_merged'
	| 'pr_closed'
	| 'verifying'
	| 'verified'
	| 'pending_verification'
	| 'ignored'
	| 'cooldown'
	| 'fix_failed'
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
	// Verification recommendation from LLM
	recommended_verification_hours?: number;
	verification_reasoning?: string;  // Why this duration was chosen
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

export interface RetryOptions {
	feedback?: string;
	includePrComments?: boolean;
}

export async function retryIncident(id: string, options?: RetryOptions): Promise<void> {
	const body = options ? {
		feedback: options.feedback,
		include_pr_comments: options.includePrComments ?? true,
	} : null;
	await request(`/api/v1/incidents/${id}/retry`, {
		method: 'POST',
		body: body ? JSON.stringify(body) : undefined,
	});
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

// Verification

export interface VerificationRound {
	id: string;
	incident_id: string;
	round_number: number;
	started_at: string;
	completed_at?: string;
	status: 'monitoring' | 'passed' | 'failed';
	occurrence_count_start: number;
	occurrence_count_end?: number;
	duration_hours: number;
}

export interface VerificationSummary {
	incident_id: string;
	status: 'not_started' | 'verifying' | 'verified' | 'recurred';
	current_round?: number;
	total_rounds: number;
	rounds: VerificationRound[];
	started_at?: string;
	verified_at?: string;
}

export async function getVerificationStatus(incidentId: string): Promise<VerificationSummary> {
	return request(`/api/v1/incidents/${incidentId}/verification`);
}

export async function startVerificationNow(incidentId: string): Promise<VerificationSummary> {
	return request(`/api/v1/incidents/${incidentId}/verify-now`, { method: 'POST' });
}

// Surgical Queue

export interface CurrentOperation {
	lock_id: string;
	incident_id: string;
	incident_title?: string;
	fingerprint: string;
	worker_id: string;
	acquired_at: string;
	expires_at: string;
}

export interface QueuedIncident {
	incident_id: string;
	incident_title?: string;
	fingerprint: string;
	queue_position: number;
	queued_at?: string;
}

export interface SurgicalQueue {
	current_operation?: CurrentOperation;
	waiting: QueuedIncident[];
}

export async function getProjectQueue(projectId: string): Promise<SurgicalQueue> {
	return request(`/api/v1/projects/${projectId}/queue`);
}

// Webhook Management

export interface WebhookStatus {
	configured: boolean;
	webhook_id?: number;
	webhook_url?: string;
	events: string[];
	active: boolean;
	error?: string;
}

export interface SetupWebhookResponse {
	success: boolean;
	webhook_id?: number;
	message: string;
}

export async function getProjectWebhook(projectId: string): Promise<WebhookStatus> {
	return request(`/api/v1/projects/${projectId}/webhook`);
}

export async function setupProjectWebhook(projectId: string): Promise<SetupWebhookResponse> {
	return request(`/api/v1/projects/${projectId}/webhook`, { method: 'POST' });
}

export async function deleteProjectWebhook(projectId: string): Promise<{ deleted: boolean; webhook_id?: number; message?: string }> {
	return request(`/api/v1/projects/${projectId}/webhook`, { method: 'DELETE' });
}

// Issue Trackers

export type IssueTrackerProvider = 'github' | 'jira' | 'linear' | 'webhook';

export interface IssueTrackerConfig {
	id: string;
	project_id: string;
	provider: IssueTrackerProvider;
	enabled: boolean;
	// GitHub config
	github_labels?: string[];
	github_assignees?: string[];
	// Jira config
	jira_site_url?: string;
	jira_project_key?: string;
	jira_issue_type?: string;
	// Linear config
	linear_team_id?: string;
	// Webhook config
	webhook_url?: string;
	webhook_events?: string[];
	// Status mapping
	status_mapping?: Record<string, string>;
	created_at: string;
}

export interface UpsertIssueTrackerRequest {
	provider: IssueTrackerProvider;
	enabled?: boolean;
	github_labels?: string[];
	github_assignees?: string[];
	jira_site_url?: string;
	jira_project_key?: string;
	jira_issue_type?: string;
	jira_priority?: string;
	status_mapping?: Record<string, string>;
	webhook_url?: string;
	webhook_headers?: Record<string, string>;
	webhook_events?: string[];
	linear_team_id?: string;
	linear_project_id?: string;
}

export async function getProjectIssueTrackers(projectId: string): Promise<IssueTrackerConfig[]> {
	return request(`/api/v1/projects/${projectId}/issue-trackers`);
}

export async function upsertProjectIssueTracker(
	projectId: string,
	provider: IssueTrackerProvider,
	config: UpsertIssueTrackerRequest
): Promise<IssueTrackerConfig> {
	return request(`/api/v1/projects/${projectId}/issue-trackers/${provider}`, {
		method: 'PUT',
		body: JSON.stringify(config)
	});
}

export async function deleteProjectIssueTracker(
	projectId: string,
	provider: IssueTrackerProvider
): Promise<{ deleted: boolean }> {
	return request(`/api/v1/projects/${projectId}/issue-trackers/${provider}`, {
		method: 'DELETE'
	});
}

export async function testWebhook(
	projectId: string,
	url: string,
	headers?: Record<string, string>
): Promise<{ success: boolean; status_code?: number; response_body?: string; error?: string }> {
	return request(`/api/v1/projects/${projectId}/issue-trackers/webhook/test`, {
		method: 'POST',
		body: JSON.stringify({ url, headers })
	});
}

// External Issues

export interface ExternalIssue {
	id: string;
	incident_id: string;
	provider: IssueTrackerProvider;
	external_id?: string;
	external_url?: string;
	external_status?: string;
	last_synced_at?: string;
	sync_error?: string;
	created_at: string;
}

export async function getIncidentExternalIssues(incidentId: string): Promise<ExternalIssue[]> {
	return request(`/api/v1/incidents/${incidentId}/external-issues`);
}

// Jira Connection

export interface JiraConnectionStatus {
	connected: boolean;
	site_url?: string;
	connected_by?: string;
	user_email?: string;
}

export async function getJiraStatus(): Promise<JiraConnectionStatus> {
	return request('/api/v1/auth/jira/status');
}

export async function connectJira(
	siteUrl: string,
	email: string,
	apiToken: string
): Promise<{ connected: boolean; site_url: string; user_name: string; user_email?: string }> {
	return request('/api/v1/auth/jira/token', {
		method: 'POST',
		body: JSON.stringify({ site_url: siteUrl, email, api_token: apiToken })
	});
}

export async function disconnectJira(): Promise<{ deleted: boolean }> {
	return request('/api/v1/auth/jira', { method: 'DELETE' });
}

// Linear Connection

export interface LinearConnectionStatus {
	connected: boolean;
	connected_by?: string;
	organization_name?: string;
}

export async function getLinearStatus(): Promise<LinearConnectionStatus> {
	return request('/api/v1/auth/linear/status');
}

export async function connectLinear(
	apiKey: string
): Promise<{ connected: boolean; user_name: string; user_email: string }> {
	return request('/api/v1/auth/linear/token', {
		method: 'POST',
		body: JSON.stringify({ api_key: apiKey })
	});
}

export async function disconnectLinear(): Promise<{ deleted: boolean }> {
	return request('/api/v1/auth/linear', { method: 'DELETE' });
}
