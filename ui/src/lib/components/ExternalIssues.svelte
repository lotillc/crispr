<script lang="ts">
	import { onMount } from 'svelte';
	import { getIncidentExternalIssues, type ExternalIssue } from '$lib/api';

	interface Props {
		incidentId: string;
	}

	let { incidentId }: Props = $props();

	let issues: ExternalIssue[] = [];
	let loading = true;
	let error = '';

	onMount(async () => {
		try {
			issues = await getIncidentExternalIssues(incidentId);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Failed to load external issues';
		} finally {
			loading = false;
		}
	});

	function getProviderIcon(provider: string): string {
		switch (provider) {
			case 'github':
				return 'GH';
			case 'jira':
				return 'JR';
			case 'linear':
				return 'LN';
			case 'webhook':
				return 'WH';
			default:
				return '?';
		}
	}

	function getProviderName(provider: string): string {
		switch (provider) {
			case 'github':
				return 'GitHub';
			case 'jira':
				return 'Jira';
			case 'linear':
				return 'Linear';
			case 'webhook':
				return 'Webhook';
			default:
				return provider;
		}
	}

	function formatDate(dateStr: string): string {
		return new Date(dateStr).toLocaleString();
	}
</script>

{#if loading}
	<div class="loading">Loading external issues...</div>
{:else if error}
	<div class="error">{error}</div>
{:else if issues.length > 0}
	<div class="external-issues">
		<h4>External Issues</h4>
		<div class="issues-list">
			{#each issues as issue}
				<div class="issue-item">
					<div class="provider-badge" title={getProviderName(issue.provider)}>
						{getProviderIcon(issue.provider)}
					</div>
					<div class="issue-info">
						{#if issue.external_url}
							<a href={issue.external_url} target="_blank" rel="noopener noreferrer">
								{issue.external_id || 'View Issue'}
							</a>
						{:else}
							<span class="no-link">{issue.external_id || 'Pending'}</span>
						{/if}
						{#if issue.external_status}
							<span class="status">{issue.external_status}</span>
						{/if}
					</div>
					{#if issue.sync_error}
						<div class="sync-error" title={issue.sync_error}>Sync Error</div>
					{:else if issue.last_synced_at}
						<div class="synced" title="Last synced: {formatDate(issue.last_synced_at)}">
							Synced
						</div>
					{/if}
				</div>
			{/each}
		</div>
	</div>
{/if}

<style>
	.external-issues {
		padding: 1rem;
		background: var(--surface-secondary, #1a1a1a);
		border-radius: 8px;
		margin-top: 1rem;
	}

	h4 {
		margin: 0 0 0.75rem 0;
		color: var(--text-primary, #fff);
		font-size: 0.9rem;
	}

	.issues-list {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.issue-item {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem;
		background: var(--surface-primary, #0a0a0a);
		border-radius: 4px;
	}

	.provider-badge {
		width: 28px;
		height: 28px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--surface-hover, #2a2a2a);
		border-radius: 4px;
		font-size: 0.7rem;
		font-weight: 600;
		color: var(--text-secondary, #888);
	}

	.issue-info {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.issue-info a {
		color: var(--accent-color, #4a9eff);
		text-decoration: none;
		font-size: 0.9rem;
	}

	.issue-info a:hover {
		text-decoration: underline;
	}

	.no-link {
		color: var(--text-secondary, #888);
		font-size: 0.9rem;
	}

	.status {
		padding: 0.2rem 0.5rem;
		background: var(--surface-hover, #2a2a2a);
		border-radius: 4px;
		font-size: 0.75rem;
		color: var(--text-secondary, #888);
	}

	.synced {
		font-size: 0.75rem;
		color: #4ade80;
	}

	.sync-error {
		font-size: 0.75rem;
		color: #ff4444;
		cursor: help;
	}

	.loading,
	.error {
		padding: 1rem;
		text-align: center;
		color: var(--text-secondary, #888);
		font-size: 0.9rem;
	}

	.error {
		color: #ff4444;
	}
</style>
