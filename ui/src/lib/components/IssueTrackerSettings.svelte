<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getProjectIssueTrackers,
		upsertProjectIssueTracker,
		deleteProjectIssueTracker,
		testWebhook,
		getJiraStatus,
		getLinearStatus,
		type IssueTrackerConfig,
		type IssueTrackerProvider,
		type JiraConnectionStatus,
		type LinearConnectionStatus
	} from '$lib/api';
	import { auth, demoMode } from '$lib/stores';

	interface Props {
		projectId: string;
	}

	let { projectId }: Props = $props();

	let trackers = $state<IssueTrackerConfig[]>([]);
	let jiraStatus = $state<JiraConnectionStatus | null>(null);
	let linearStatus = $state<LinearConnectionStatus | null>(null);
	let loading = $state(true);
	let error = $state('');
	let success = $state('');

	// Mock data for demo mode
	const mockTrackers: IssueTrackerConfig[] = [
		{
			id: 'mock-github-tracker',
			project_id: projectId,
			provider: 'github',
			enabled: true,
			github_labels: ['bug', 'crispr'],
			github_assignees: [],
			created_at: new Date().toISOString()
		}
	];
	const mockJiraStatus: JiraConnectionStatus = { connected: false };
	const mockLinearStatus: LinearConnectionStatus = { connected: false };

	// Determine available tabs based on global connections
	let availableTabs = $derived.by(() => {
		const tabs: { id: IssueTrackerProvider; label: string; configured: boolean }[] = [];

		// Add Jira if globally connected
		if (jiraStatus?.connected) {
			tabs.push({ id: 'jira', label: 'Jira', configured: true });
		}

		// Add Linear if globally connected
		if (linearStatus?.connected) {
			tabs.push({ id: 'linear', label: 'Linear', configured: true });
		}

		// GitHub is always available (uses same GitHub connection)
		tabs.push({ id: 'github', label: 'GitHub Issues', configured: $auth?.github?.connected ?? false });

		// Webhook is always available
		tabs.push({ id: 'webhook', label: 'Webhook', configured: true });

		return tabs;
	});

	// Set default tab to first configured integration
	let activeTab: IssueTrackerProvider = $state('github');

	// GitHub form
	let githubEnabled = $state(false);
	let githubLabels = $state('');
	let githubAssignees = $state('');

	// Jira form (project-specific only - auth is global)
	let jiraEnabled = $state(false);
	let jiraProjectKey = $state('');
	let jiraIssueType = $state('Bug');

	// Linear form (project-specific only - auth is global)
	let linearEnabled = $state(false);
	let linearTeamId = $state('');

	// Webhook form
	let webhookEnabled = $state(false);
	let webhookUrl = $state('');
	let webhookAuthHeader = $state('');
	let webhookEvents = $state('all');
	let webhookTestResult = $state<{ success: boolean; message: string } | null>(null);

	const CRISPR_STATES = [
		'detected',
		'triaging',
		'fixing',
		'pr_open',
		'pr_merged',
		'verifying',
		'verified',
		'unfixable',
		'fix_failed'
	];

	onMount(async () => {
		await loadData();
	});

	async function loadData() {
		loading = true;
		error = '';
		try {
			if ($demoMode) {
				// Use mock data in demo mode
				trackers = mockTrackers;
				jiraStatus = mockJiraStatus;
				linearStatus = mockLinearStatus;
			} else {
				const [trackersData, jiraData, linearData] = await Promise.all([
					getProjectIssueTrackers(projectId),
					getJiraStatus(),
					getLinearStatus()
				]);

				trackers = trackersData;
				jiraStatus = jiraData;
				linearStatus = linearData;
			}

			// Set default tab to first configured external tracker, or github
			if (jiraStatus?.connected) {
				activeTab = 'jira';
			} else if (linearStatus?.connected) {
				activeTab = 'linear';
			} else {
				activeTab = 'github';
			}

			// Populate forms from existing config
			const githubConfig = trackers.find((t) => t.provider === 'github');
			if (githubConfig) {
				githubEnabled = githubConfig.enabled;
				githubLabels = githubConfig.github_labels?.join(', ') || '';
				githubAssignees = githubConfig.github_assignees?.join(', ') || '';
			}

			const jiraConfig = trackers.find((t) => t.provider === 'jira');
			if (jiraConfig) {
				jiraEnabled = jiraConfig.enabled;
				jiraProjectKey = jiraConfig.jira_project_key || '';
				jiraIssueType = jiraConfig.jira_issue_type || 'Bug';
			}

			const linearConfig = trackers.find((t) => t.provider === 'linear');
			if (linearConfig) {
				linearEnabled = linearConfig.enabled;
				linearTeamId = linearConfig.linear_team_id || '';
			}

			const webhookConfig = trackers.find((t) => t.provider === 'webhook');
			if (webhookConfig) {
				webhookEnabled = webhookConfig.enabled;
				webhookUrl = webhookConfig.webhook_url || '';
				webhookEvents = webhookConfig.webhook_events?.join(', ') || 'all';
			}
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load issue tracker settings';
		} finally {
			loading = false;
		}
	}

	async function saveGitHub() {
		error = '';
		success = '';
		try {
			if ($demoMode) {
				success = 'GitHub Issues settings saved (demo mode)';
				return;
			}
			await upsertProjectIssueTracker(projectId, 'github', {
				provider: 'github',
				enabled: githubEnabled,
				github_labels: githubLabels
					.split(',')
					.map((s) => s.trim())
					.filter(Boolean),
				github_assignees: githubAssignees
					.split(',')
					.map((s) => s.trim())
					.filter(Boolean)
			});
			success = 'GitHub Issues settings saved';
			await loadData();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save';
		}
	}

	async function saveJira() {
		error = '';
		success = '';
		try {
			if ($demoMode) {
				success = 'Jira settings saved (demo mode)';
				return;
			}
			await upsertProjectIssueTracker(projectId, 'jira', {
				provider: 'jira',
				enabled: jiraEnabled,
				jira_project_key: jiraProjectKey,
				jira_issue_type: jiraIssueType
			});
			success = 'Jira settings saved';
			await loadData();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save';
		}
	}

	async function saveLinear() {
		error = '';
		success = '';
		try {
			if ($demoMode) {
				success = 'Linear settings saved (demo mode)';
				return;
			}
			await upsertProjectIssueTracker(projectId, 'linear', {
				provider: 'linear',
				enabled: linearEnabled,
				linear_team_id: linearTeamId
			});
			success = 'Linear settings saved';
			await loadData();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save';
		}
	}

	async function saveWebhook() {
		error = '';
		success = '';
		try {
			if ($demoMode) {
				success = 'Webhook settings saved (demo mode)';
				return;
			}
			const headers: Record<string, string> = {};
			if (webhookAuthHeader) {
				headers['Authorization'] = webhookAuthHeader;
			}

			await upsertProjectIssueTracker(projectId, 'webhook', {
				provider: 'webhook',
				enabled: webhookEnabled,
				webhook_url: webhookUrl,
				webhook_headers: Object.keys(headers).length > 0 ? headers : undefined,
				webhook_events: webhookEvents
					.split(',')
					.map((s) => s.trim())
					.filter(Boolean)
			});
			success = 'Webhook settings saved';
			await loadData();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save';
		}
	}

	async function handleTestWebhook() {
		webhookTestResult = null;
		if ($demoMode) {
			webhookTestResult = { success: true, message: 'Success (HTTP 200) - demo mode' };
			return;
		}
		try {
			const headers: Record<string, string> = {};
			if (webhookAuthHeader) {
				headers['Authorization'] = webhookAuthHeader;
			}

			const result = await testWebhook(projectId, webhookUrl, headers);
			webhookTestResult = {
				success: result.success,
				message: result.success
					? `Success (HTTP ${result.status_code})`
					: result.error || `Failed (HTTP ${result.status_code})`
			};
		} catch (e) {
			webhookTestResult = {
				success: false,
				message: e instanceof Error ? e.message : 'Test failed'
			};
		}
	}

	async function handleDelete(provider: IssueTrackerProvider) {
		if (!confirm(`Remove ${provider} integration?`)) return;

		error = '';
		try {
			if ($demoMode) {
				success = `${provider} integration removed (demo mode)`;
				return;
			}
			await deleteProjectIssueTracker(projectId, provider);
			success = `${provider} integration removed`;
			await loadData();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete';
		}
	}
</script>

<div class="issue-tracker-settings">
	<p class="description">
		Create and update issues in external trackers as incidents progress.
	</p>

	{#if error}
		<div class="error-message">{error}</div>
	{/if}

	{#if success}
		<div class="success-message">{success}</div>
	{/if}

	{#if loading}
		<div class="loading">Loading...</div>
	{:else}
		<div class="tabs">
			{#each availableTabs as tab}
				<button
					class:active={activeTab === tab.id}
					onclick={() => activeTab = tab.id}
				>
					{tab.label}
					{#if trackers.find(t => t.provider === tab.id && t.enabled)}
						<span class="enabled-dot"></span>
					{/if}
				</button>
			{/each}
		</div>

		<div class="tab-content">
			{#if activeTab === 'github'}
				<div class="form-section">
					{#if !$auth?.github?.connected}
						<div class="not-configured">
							<p>GitHub is not connected. Connect GitHub in the global settings to use GitHub Issues.</p>
						</div>
					{:else}
						<label class="checkbox-label">
							<input type="checkbox" bind:checked={githubEnabled} />
							Enable GitHub Issues for this project
						</label>

						<div class="form-group">
							<label for="github-labels">Labels (comma-separated)</label>
							<input
								type="text"
								id="github-labels"
								bind:value={githubLabels}
								placeholder="bug, crispr, auto-fix"
							/>
						</div>

						<div class="form-group">
							<label for="github-assignees">Assignees (comma-separated)</label>
							<input
								type="text"
								id="github-assignees"
								bind:value={githubAssignees}
								placeholder="username1, username2"
							/>
						</div>

						<div class="button-row">
							<button class="primary" onclick={saveGitHub}>Save</button>
							{#if trackers.find((t) => t.provider === 'github')}
								<button class="danger" onclick={() => handleDelete('github')}>Remove</button>
							{/if}
						</div>
					{/if}
				</div>
			{:else if activeTab === 'jira'}
				<div class="form-section">
					<div class="connection-info">
						Connected to <strong>{jiraStatus?.site_url}</strong>
						<span class="hint">Manage connection in global settings</span>
					</div>

					<label class="checkbox-label">
						<input type="checkbox" bind:checked={jiraEnabled} />
						Enable Jira for this project
					</label>

					<div class="form-group">
						<label for="jira-project">Project Key</label>
						<input type="text" id="jira-project" bind:value={jiraProjectKey} placeholder="PROJ" />
						<small>The Jira project key where issues will be created</small>
					</div>

					<div class="form-group">
						<label for="jira-type">Issue Type</label>
						<select id="jira-type" bind:value={jiraIssueType}>
							<option value="Bug">Bug</option>
							<option value="Task">Task</option>
							<option value="Story">Story</option>
						</select>
					</div>

					<div class="button-row">
						<button class="primary" onclick={saveJira}>Save</button>
						{#if trackers.find((t) => t.provider === 'jira')}
							<button class="danger" onclick={() => handleDelete('jira')}>Remove</button>
						{/if}
					</div>
				</div>
			{:else if activeTab === 'linear'}
				<div class="form-section">
					<div class="connection-info">
						Connected to <strong>{linearStatus?.organization_name || 'Linear'}</strong>
						<span class="hint">Manage connection in global settings</span>
					</div>

					<label class="checkbox-label">
						<input type="checkbox" bind:checked={linearEnabled} />
						Enable Linear for this project
					</label>

					<div class="form-group">
						<label for="linear-team">Team ID</label>
						<input type="text" id="linear-team" bind:value={linearTeamId} placeholder="team-id" />
						<small>The Linear team where issues will be created</small>
					</div>

					<div class="button-row">
						<button class="primary" onclick={saveLinear}>Save</button>
						{#if trackers.find((t) => t.provider === 'linear')}
							<button class="danger" onclick={() => handleDelete('linear')}>Remove</button>
						{/if}
					</div>
				</div>
			{:else if activeTab === 'webhook'}
				<div class="form-section">
					<p class="provider-description">
						Send webhooks to any external system on incident state changes.
					</p>

					<label class="checkbox-label">
						<input type="checkbox" bind:checked={webhookEnabled} />
						Enable Webhook for this project
					</label>

					<div class="form-group">
						<label for="webhook-url">Webhook URL</label>
						<input
							type="url"
							id="webhook-url"
							bind:value={webhookUrl}
							placeholder="https://your-service.com/webhook"
						/>
					</div>

					<div class="form-group">
						<label for="webhook-auth">Authorization Header (optional)</label>
						<input
							type="password"
							id="webhook-auth"
							bind:value={webhookAuthHeader}
							placeholder="Bearer your-token"
						/>
					</div>

					<div class="form-group">
						<label for="webhook-events">Events (comma-separated, or "all")</label>
						<input
							type="text"
							id="webhook-events"
							bind:value={webhookEvents}
							placeholder="all"
						/>
						<small>Available: {CRISPR_STATES.join(', ')}</small>
					</div>

					{#if webhookTestResult}
						<div class="test-result" class:success={webhookTestResult.success}>
							{webhookTestResult.message}
						</div>
					{/if}

					<div class="button-row">
						<button class="secondary" onclick={handleTestWebhook} disabled={!webhookUrl}>
							Test Webhook
						</button>
						<button class="primary" onclick={saveWebhook}>Save</button>
						{#if trackers.find((t) => t.provider === 'webhook')}
							<button class="danger" onclick={() => handleDelete('webhook')}>Remove</button>
						{/if}
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.issue-tracker-settings {
		padding: 0;
	}

	.description {
		color: var(--graphite);
		margin-bottom: 1.5rem;
		font-size: 0.875rem;
	}

	.provider-description {
		color: var(--graphite);
		font-size: 0.8125rem;
		margin-bottom: 1rem;
	}

	.tabs {
		display: flex;
		gap: 0.25rem;
		margin-bottom: 1.5rem;
		border-bottom: 1px solid var(--silver);
		padding-bottom: 0;
	}

	.tabs button {
		padding: 0.75rem 1rem;
		background: none;
		border: none;
		border-bottom: 2px solid transparent;
		color: var(--graphite);
		cursor: pointer;
		transition: all 0.2s;
		font-size: 0.875rem;
		position: relative;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.tabs button:hover {
		color: var(--charcoal);
	}

	.tabs button.active {
		color: var(--scrubs);
		border-bottom-color: var(--scrubs);
	}

	.enabled-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--scrubs);
	}

	.form-section {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.form-group {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.form-group label {
		color: var(--charcoal);
		font-size: 0.8125rem;
		font-weight: 500;
	}

	.form-group input,
	.form-group select {
		padding: 0.625rem 0.75rem;
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-md);
		color: var(--charcoal);
		font-size: 0.875rem;
	}

	.form-group input:focus,
	.form-group select:focus {
		outline: none;
		border-color: var(--scrubs);
		box-shadow: 0 0 0 3px var(--scrubs-soft);
	}

	.form-group small {
		color: var(--slate);
		font-size: 0.75rem;
	}

	.checkbox-label {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		color: var(--charcoal);
		cursor: pointer;
		font-size: 0.875rem;
	}

	.checkbox-label input[type='checkbox'] {
		width: 18px;
		height: 18px;
		accent-color: var(--scrubs);
	}

	.button-row {
		display: flex;
		gap: 0.75rem;
		margin-top: 0.5rem;
	}

	button.primary {
		padding: 0.625rem 1.25rem;
		background: var(--scrubs);
		border: none;
		border-radius: var(--radius-md);
		color: #fff;
		cursor: pointer;
		font-weight: 500;
		font-size: 0.875rem;
	}

	button.primary:hover {
		background: var(--scrubs-dark);
	}

	button.secondary {
		padding: 0.625rem 1.25rem;
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-md);
		color: var(--charcoal);
		cursor: pointer;
		font-size: 0.875rem;
	}

	button.secondary:hover {
		background: var(--paper);
	}

	button.danger {
		padding: 0.625rem 1.25rem;
		background: transparent;
		border: 1px solid var(--critical);
		border-radius: var(--radius-md);
		color: var(--critical);
		cursor: pointer;
		font-size: 0.875rem;
	}

	button.danger:hover {
		background: var(--critical-soft);
	}

	.connection-info {
		padding: 0.75rem 1rem;
		background: var(--scrubs-soft);
		border-radius: var(--radius-md);
		font-size: 0.875rem;
		color: var(--scrubs-dark);
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.connection-info .hint {
		color: var(--slate);
		font-size: 0.75rem;
		margin-left: auto;
	}

	.not-configured {
		padding: 1rem;
		background: var(--paper);
		border-radius: var(--radius-md);
		text-align: center;
	}

	.not-configured p {
		color: var(--graphite);
		font-size: 0.875rem;
		margin: 0;
	}

	.test-result {
		padding: 0.75rem;
		border-radius: var(--radius-md);
		font-size: 0.875rem;
	}

	.test-result.success {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.test-result:not(.success) {
		background: var(--critical-soft);
		color: var(--critical);
	}

	.error-message {
		padding: 0.75rem;
		background: var(--critical-soft);
		border-radius: var(--radius-md);
		color: var(--critical);
		margin-bottom: 1rem;
		font-size: 0.875rem;
	}

	.success-message {
		padding: 0.75rem;
		background: var(--scrubs-soft);
		border-radius: var(--radius-md);
		color: var(--scrubs-dark);
		margin-bottom: 1rem;
		font-size: 0.875rem;
	}

	.loading {
		text-align: center;
		color: var(--graphite);
		padding: 2rem;
	}
</style>
