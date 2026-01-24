<script lang="ts">
	import { onMount } from 'svelte';
	import {
		getProjectIssueTrackers,
		upsertProjectIssueTracker,
		deleteProjectIssueTracker,
		testWebhook,
		getJiraStatus,
		connectJira,
		disconnectJira,
		getLinearStatus,
		connectLinear,
		disconnectLinear,
		type IssueTrackerConfig,
		type IssueTrackerProvider,
		type JiraConnectionStatus,
		type LinearConnectionStatus
	} from '$lib/api';

	export let projectId: string;

	let trackers: IssueTrackerConfig[] = [];
	let jiraStatus: JiraConnectionStatus | null = null;
	let linearStatus: LinearConnectionStatus | null = null;
	let loading = true;
	let error = '';
	let success = '';

	// Form state
	let activeTab: IssueTrackerProvider = 'github';

	// GitHub form
	let githubEnabled = false;
	let githubLabels = '';
	let githubAssignees = '';

	// Jira form
	let jiraEnabled = false;
	let jiraSiteUrl = '';
	let jiraEmail = '';
	let jiraApiToken = '';
	let jiraProjectKey = '';
	let jiraIssueType = 'Bug';

	// Linear form
	let linearEnabled = false;
	let linearApiKey = '';
	let linearTeamId = '';

	// Webhook form
	let webhookEnabled = false;
	let webhookUrl = '';
	let webhookAuthHeader = '';
	let webhookEvents = 'all';
	let webhookTestResult: { success: boolean; message: string } | null = null;

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
			const [trackersData, jiraData, linearData] = await Promise.all([
				getProjectIssueTrackers(projectId),
				getJiraStatus(),
				getLinearStatus()
			]);

			trackers = trackersData;
			jiraStatus = jiraData;
			linearStatus = linearData;

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
				jiraSiteUrl = jiraConfig.jira_site_url || '';
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
			// If not connected, connect first
			if (!jiraStatus?.connected && jiraEmail && jiraApiToken) {
				await connectJira(jiraSiteUrl, jiraEmail, jiraApiToken);
			}

			await upsertProjectIssueTracker(projectId, 'jira', {
				provider: 'jira',
				enabled: jiraEnabled,
				jira_site_url: jiraSiteUrl,
				jira_project_key: jiraProjectKey,
				jira_issue_type: jiraIssueType
			});
			success = 'Jira settings saved';
			await loadData();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to save';
		}
	}

	async function handleDisconnectJira() {
		error = '';
		try {
			await disconnectJira();
			jiraStatus = { connected: false };
			success = 'Jira disconnected';
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to disconnect';
		}
	}

	async function saveLinear() {
		error = '';
		success = '';
		try {
			// If not connected, connect first
			if (!linearStatus?.connected && linearApiKey) {
				await connectLinear(linearApiKey);
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

	async function handleDisconnectLinear() {
		error = '';
		try {
			await disconnectLinear();
			linearStatus = { connected: false };
			success = 'Linear disconnected';
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to disconnect';
		}
	}

	async function saveWebhook() {
		error = '';
		success = '';
		try {
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
			await deleteProjectIssueTracker(projectId, provider);
			success = `${provider} integration removed`;
			await loadData();
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to delete';
		}
	}
</script>

<div class="issue-tracker-settings">
	<h3>Issue Tracker Integration</h3>
	<p class="description">
		Automatically create and update issues in external trackers as incidents progress through the
		pipeline.
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
			<button class:active={activeTab === 'github'} on:click={() => (activeTab = 'github')}>
				GitHub Issues
			</button>
			<button class:active={activeTab === 'jira'} on:click={() => (activeTab = 'jira')}>
				Jira
			</button>
			<button class:active={activeTab === 'linear'} on:click={() => (activeTab = 'linear')}>
				Linear
			</button>
			<button class:active={activeTab === 'webhook'} on:click={() => (activeTab = 'webhook')}>
				Webhook
			</button>
		</div>

		<div class="tab-content">
			{#if activeTab === 'github'}
				<div class="form-section">
					<p class="provider-description">
						Create GitHub issues in the same repository to track incidents.
					</p>

					<label class="checkbox-label">
						<input type="checkbox" bind:checked={githubEnabled} />
						Enable GitHub Issues
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
						<button class="primary" on:click={saveGitHub}>Save</button>
						{#if trackers.find((t) => t.provider === 'github')}
							<button class="danger" on:click={() => handleDelete('github')}>Remove</button>
						{/if}
					</div>
				</div>
			{:else if activeTab === 'jira'}
				<div class="form-section">
					<p class="provider-description">
						Create Jira issues and transition them through your workflow.
					</p>

					{#if jiraStatus?.connected}
						<div class="connection-status connected">
							Connected to {jiraStatus.site_url}
							<button class="text-button" on:click={handleDisconnectJira}>Disconnect</button>
						</div>
					{:else}
						<div class="form-group">
							<label for="jira-site">Jira Site URL</label>
							<input
								type="text"
								id="jira-site"
								bind:value={jiraSiteUrl}
								placeholder="https://company.atlassian.net"
							/>
						</div>

						<div class="form-group">
							<label for="jira-email">Email</label>
							<input type="email" id="jira-email" bind:value={jiraEmail} placeholder="you@company.com" />
						</div>

						<div class="form-group">
							<label for="jira-token">API Token</label>
							<input
								type="password"
								id="jira-token"
								bind:value={jiraApiToken}
								placeholder="Your Jira API token"
							/>
							<small
								>Get your token from <a
									href="https://id.atlassian.com/manage-profile/security/api-tokens"
									target="_blank">Atlassian Account Settings</a
								></small
							>
						</div>
					{/if}

					<label class="checkbox-label">
						<input type="checkbox" bind:checked={jiraEnabled} />
						Enable Jira Integration
					</label>

					<div class="form-group">
						<label for="jira-project">Project Key</label>
						<input type="text" id="jira-project" bind:value={jiraProjectKey} placeholder="PROJ" />
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
						<button class="primary" on:click={saveJira}>Save</button>
						{#if trackers.find((t) => t.provider === 'jira')}
							<button class="danger" on:click={() => handleDelete('jira')}>Remove</button>
						{/if}
					</div>
				</div>
			{:else if activeTab === 'linear'}
				<div class="form-section">
					<p class="provider-description">Create Linear issues to track incidents.</p>

					{#if linearStatus?.connected}
						<div class="connection-status connected">
							Connected as {linearStatus.connected_by}
							<button class="text-button" on:click={handleDisconnectLinear}>Disconnect</button>
						</div>
					{:else}
						<div class="form-group">
							<label for="linear-key">API Key</label>
							<input
								type="password"
								id="linear-key"
								bind:value={linearApiKey}
								placeholder="lin_api_..."
							/>
							<small
								>Get your API key from <a href="https://linear.app/settings/api" target="_blank"
									>Linear Settings</a
								></small
							>
						</div>
					{/if}

					<label class="checkbox-label">
						<input type="checkbox" bind:checked={linearEnabled} />
						Enable Linear Integration
					</label>

					<div class="form-group">
						<label for="linear-team">Team ID</label>
						<input type="text" id="linear-team" bind:value={linearTeamId} placeholder="team-id" />
					</div>

					<div class="button-row">
						<button class="primary" on:click={saveLinear}>Save</button>
						{#if trackers.find((t) => t.provider === 'linear')}
							<button class="danger" on:click={() => handleDelete('linear')}>Remove</button>
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
						Enable Webhook
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
						<button class="secondary" on:click={handleTestWebhook} disabled={!webhookUrl}>
							Test Webhook
						</button>
						<button class="primary" on:click={saveWebhook}>Save</button>
						{#if trackers.find((t) => t.provider === 'webhook')}
							<button class="danger" on:click={() => handleDelete('webhook')}>Remove</button>
						{/if}
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.issue-tracker-settings {
		padding: 1.5rem;
		background: var(--surface-secondary, #1a1a1a);
		border-radius: 8px;
	}

	h3 {
		margin: 0 0 0.5rem 0;
		color: var(--text-primary, #fff);
	}

	.description {
		color: var(--text-secondary, #888);
		margin-bottom: 1.5rem;
		font-size: 0.9rem;
	}

	.provider-description {
		color: var(--text-secondary, #888);
		font-size: 0.85rem;
		margin-bottom: 1rem;
	}

	.tabs {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 1.5rem;
		border-bottom: 1px solid var(--border-color, #333);
		padding-bottom: 0.5rem;
	}

	.tabs button {
		padding: 0.5rem 1rem;
		background: none;
		border: none;
		color: var(--text-secondary, #888);
		cursor: pointer;
		border-radius: 4px;
		transition: all 0.2s;
	}

	.tabs button:hover {
		background: var(--surface-hover, #2a2a2a);
		color: var(--text-primary, #fff);
	}

	.tabs button.active {
		background: var(--accent-color, #4a9eff);
		color: #fff;
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
		color: var(--text-secondary, #888);
		font-size: 0.85rem;
	}

	.form-group input,
	.form-group select {
		padding: 0.75rem;
		background: var(--surface-primary, #0a0a0a);
		border: 1px solid var(--border-color, #333);
		border-radius: 4px;
		color: var(--text-primary, #fff);
		font-size: 0.9rem;
	}

	.form-group input:focus,
	.form-group select:focus {
		outline: none;
		border-color: var(--accent-color, #4a9eff);
	}

	.form-group small {
		color: var(--text-secondary, #666);
		font-size: 0.8rem;
	}

	.form-group small a {
		color: var(--accent-color, #4a9eff);
	}

	.checkbox-label {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		color: var(--text-primary, #fff);
		cursor: pointer;
	}

	.checkbox-label input[type='checkbox'] {
		width: 18px;
		height: 18px;
	}

	.button-row {
		display: flex;
		gap: 0.75rem;
		margin-top: 0.5rem;
	}

	button.primary {
		padding: 0.75rem 1.5rem;
		background: var(--accent-color, #4a9eff);
		border: none;
		border-radius: 4px;
		color: #fff;
		cursor: pointer;
		font-weight: 500;
	}

	button.primary:hover {
		opacity: 0.9;
	}

	button.secondary {
		padding: 0.75rem 1.5rem;
		background: var(--surface-hover, #2a2a2a);
		border: 1px solid var(--border-color, #333);
		border-radius: 4px;
		color: var(--text-primary, #fff);
		cursor: pointer;
	}

	button.danger {
		padding: 0.75rem 1.5rem;
		background: transparent;
		border: 1px solid var(--error-color, #ff4444);
		border-radius: 4px;
		color: var(--error-color, #ff4444);
		cursor: pointer;
	}

	button.danger:hover {
		background: var(--error-color, #ff4444);
		color: #fff;
	}

	button.text-button {
		background: none;
		border: none;
		color: var(--accent-color, #4a9eff);
		cursor: pointer;
		padding: 0;
		margin-left: 0.5rem;
	}

	.connection-status {
		padding: 0.75rem;
		border-radius: 4px;
		font-size: 0.9rem;
	}

	.connection-status.connected {
		background: rgba(74, 222, 128, 0.1);
		color: #4ade80;
		border: 1px solid rgba(74, 222, 128, 0.3);
	}

	.test-result {
		padding: 0.75rem;
		border-radius: 4px;
		font-size: 0.9rem;
	}

	.test-result.success {
		background: rgba(74, 222, 128, 0.1);
		color: #4ade80;
	}

	.test-result:not(.success) {
		background: rgba(255, 68, 68, 0.1);
		color: #ff4444;
	}

	.error-message {
		padding: 0.75rem;
		background: rgba(255, 68, 68, 0.1);
		border: 1px solid rgba(255, 68, 68, 0.3);
		border-radius: 4px;
		color: #ff4444;
		margin-bottom: 1rem;
	}

	.success-message {
		padding: 0.75rem;
		background: rgba(74, 222, 128, 0.1);
		border: 1px solid rgba(74, 222, 128, 0.3);
		border-radius: 4px;
		color: #4ade80;
		margin-bottom: 1rem;
	}

	.loading {
		text-align: center;
		color: var(--text-secondary, #888);
		padding: 2rem;
	}
</style>
