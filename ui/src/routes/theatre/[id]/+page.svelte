<script lang="ts">
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { projects, incidents } from '$lib/stores';
	import * as api from '$lib/api';
	import type { Project, Incident, Sample, ActivityLog } from '$lib/api';

	let projectId = $derived($page.params.id);
	let incidentIdFromUrl = $derived($page.url.searchParams.get('incident'));
	let project = $derived($projects.find(p => p.id === projectId));
	let projectIncidents = $derived($incidents.filter(i => i.project_id === projectId));
	let projectStats = $state<api.ProjectStats | null>(null);

	let selectedIncident = $state<(Incident & { samples: Sample[]; activity: ActivityLog[] }) | null>(null);
	let loadingIncident = $state(false);
	let actionLoading = $state(false);
	let hasAutoSelected = $state(false);

	onMount(async () => {
		if (projectId) {
			try {
				projectStats = await api.getProjectStats(projectId);
			} catch (e) {
				console.error('Failed to load project stats:', e);
			}
		}
	});

	// Auto-select incident if provided in URL (reactive to handle async loading)
	$effect(() => {
		if (incidentIdFromUrl && projectIncidents.length > 0 && !hasAutoSelected && !selectedIncident) {
			const incidentToSelect = projectIncidents.find(i => i.id === incidentIdFromUrl);
			if (incidentToSelect) {
				hasAutoSelected = true;
				selectIncident(incidentToSelect);
			}
		}
	});

	async function selectIncident(incident: Incident) {
		loadingIncident = true;
		try {
			selectedIncident = await incidents.get(incident.id);
		} catch (e) {
			console.error('Failed to load incident:', e);
		} finally {
			loadingIncident = false;
		}
	}

	async function retryIncident() {
		if (!selectedIncident) return;
		actionLoading = true;
		try {
			await incidents.retry(selectedIncident.id);
			selectedIncident = await incidents.get(selectedIncident.id);
		} finally {
			actionLoading = false;
		}
	}

	async function ignoreIncident() {
		if (!selectedIncident) return;
		actionLoading = true;
		try {
			await incidents.ignore(selectedIncident.id);
			selectedIncident = await incidents.get(selectedIncident.id);
		} finally {
			actionLoading = false;
		}
	}

	function getStatusLabel(status: string): string {
		switch (status) {
			case 'detected': return 'Detected';
			case 'triaging': return 'Analyzing';
			case 'unfixable': return 'Manual Review';
			case 'pending_fix': return 'Queued';
			case 'fixing': return 'Operating';
			case 'pr_open': return 'Awaiting Approval';
			case 'pr_merged': return 'Discharged';
			case 'ignored': return 'Dismissed';
			case 'max_attempts': return 'Requires Specialist';
			default: return status;
		}
	}

	function getStatusBadgeClass(status: string): string {
		if (['fixing', 'triaging'].includes(status)) return 'badge-surgery';
		if (['detected', 'pending_fix'].includes(status)) return 'badge-observation';
		if (['pr_open'].includes(status)) return 'badge-healthy';
		if (['unfixable', 'max_attempts'].includes(status)) return 'badge-critical';
		return 'badge-discharged';
	}

	function formatTime(dateStr: string): string {
		return new Date(dateStr).toLocaleString();
	}

	function formatTimeAgo(dateStr: string): string {
		const date = new Date(dateStr);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffMins = Math.floor(diffMs / 60000);

		if (diffMins < 1) return 'just now';
		if (diffMins < 60) return `${diffMins}m ago`;
		const diffHours = Math.floor(diffMins / 60);
		if (diffHours < 24) return `${diffHours}h ago`;
		const diffDays = Math.floor(diffHours / 24);
		return `${diffDays}d ago`;
	}
</script>

<svelte:head>
	<title>{project?.repo ?? 'Service'} - Operating Theatre - CRISPR</title>
</svelte:head>

<div class="theatre">
	<!-- Header -->
	<header class="theatre-header">
		<a href="/" class="back-link">
			<svg viewBox="0 0 20 20" fill="currentColor">
				<path fill-rule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clip-rule="evenodd"/>
			</svg>
			Surgery Board
		</a>

		{#if project}
			<div class="theatre-title">
				<h1>
					<span class="repo-owner">{project.repo.split('/')[0]}/</span>
					<span class="repo-name">{project.repo.split('/')[1]}</span>
				</h1>
				<span class="theatre-subtitle">Operating Theatre</span>
			</div>

			<div class="theatre-stats">
				<div class="stat">
					<span class="stat-value">{projectStats?.open_incidents ?? 0}</span>
					<span class="stat-label">Active</span>
				</div>
				<div class="stat">
					<span class="stat-value">{projectStats?.fixed_incidents ?? 0}</span>
					<span class="stat-label">Fixed</span>
				</div>
				<div class="stat">
					<span class="stat-value">${(projectStats?.total_cost ?? 0).toFixed(2)}</span>
					<span class="stat-label">Cost</span>
				</div>
			</div>
		{/if}
	</header>

	<div class="theatre-body">
		<!-- Incident List -->
		<aside class="incident-list">
			<div class="list-header">
				<h2>Cases</h2>
				<span class="case-count">{projectIncidents.length}</span>
			</div>

			{#if projectIncidents.length === 0}
				<div class="empty-list">
					<p>No incidents recorded for this service.</p>
				</div>
			{:else}
				<div class="cases">
					{#each projectIncidents as incident}
						<button
							class="case-item"
							class:selected={selectedIncident?.id === incident.id}
							onclick={() => selectIncident(incident)}
						>
							<div class="case-header">
								<span class="badge {getStatusBadgeClass(incident.status)}">
									{getStatusLabel(incident.status)}
								</span>
								<span class="case-time">{formatTimeAgo(incident.last_seen)}</span>
							</div>
							<p class="case-message">{incident.message}</p>
							<div class="case-meta">
								<span class="case-level badge badge-{incident.level.toLowerCase()}">{incident.level}</span>
								<span class="case-samples">{incident.sample_count} samples</span>
							</div>
						</button>
					{/each}
				</div>
			{/if}
		</aside>

		<!-- Incident Detail -->
		<main class="incident-detail">
			{#if loadingIncident}
				<div class="loading-state">
					<div class="spinner"></div>
					<p>Loading case details...</p>
				</div>
			{:else if selectedIncident}
				<div class="detail-content">
					<div class="detail-header">
						<div>
							<span class="badge {getStatusBadgeClass(selectedIncident.status)} large">
								{getStatusLabel(selectedIncident.status)}
							</span>
							<h2>{selectedIncident.message}</h2>
							<p class="detail-meta">
								First seen: {formatTime(selectedIncident.first_seen)} &bull;
								Last seen: {formatTime(selectedIncident.last_seen)} &bull;
								{selectedIncident.sample_count} occurrences
							</p>
						</div>

						<div class="detail-actions">
							{#if selectedIncident.pr_url}
								<a href={selectedIncident.pr_url} target="_blank" class="btn btn-primary">
									<svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
										<path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM4.332 8.027a6.012 6.012 0 011.912-2.706C6.512 5.73 6.974 6 7.5 6A1.5 1.5 0 019 7.5V8a2 2 0 004 0 2 2 0 011.523-1.943A5.977 5.977 0 0116 10c0 .34-.028.675-.083 1H15a2 2 0 00-2 2v2.197A5.973 5.973 0 0110 16v-2a2 2 0 00-2-2 2 2 0 01-2-2 2 2 0 00-1.668-1.973z" clip-rule="evenodd"/>
									</svg>
									View PR
								</a>
							{/if}
							{#if !['pr_merged', 'ignored'].includes(selectedIncident.status)}
								<button class="btn btn-secondary" onclick={retryIncident} disabled={actionLoading}>
									Retry Fix
								</button>
								<button class="btn btn-ghost" onclick={ignoreIncident} disabled={actionLoading}>
									Dismiss
								</button>
							{/if}
						</div>
					</div>

					<!-- Pipeline Status -->
					<section class="pipeline-section">
						<h3>Pipeline Status</h3>
						<div class="pipeline">
							<div class="pipeline-step" class:complete={['triaging', 'unfixable', 'pending_fix', 'fixing', 'pr_open', 'pr_merged'].includes(selectedIncident.status)} class:active={selectedIncident.status === 'detected'}>
								<span class="step-icon">1</span>
								<span class="step-label">Detected</span>
							</div>
							<div class="pipeline-connector" class:complete={['triaging', 'unfixable', 'pending_fix', 'fixing', 'pr_open', 'pr_merged'].includes(selectedIncident.status)}></div>
							<div class="pipeline-step" class:complete={['unfixable', 'pending_fix', 'fixing', 'pr_open', 'pr_merged'].includes(selectedIncident.status)} class:active={selectedIncident.status === 'triaging'}>
								<span class="step-icon">2</span>
								<span class="step-label">Analyzing</span>
							</div>
							<div class="pipeline-connector" class:complete={['fixing', 'pr_open', 'pr_merged'].includes(selectedIncident.status)}></div>
							<div class="pipeline-step" class:complete={['pr_open', 'pr_merged'].includes(selectedIncident.status)} class:active={selectedIncident.status === 'fixing'}>
								<span class="step-icon">3</span>
								<span class="step-label">Operating</span>
							</div>
							<div class="pipeline-connector" class:complete={['pr_merged'].includes(selectedIncident.status)}></div>
							<div class="pipeline-step" class:complete={selectedIncident.status === 'pr_merged'} class:active={selectedIncident.status === 'pr_open'}>
								<span class="step-icon">4</span>
								<span class="step-label">Discharged</span>
							</div>
						</div>
					</section>

					<!-- Triage Result -->
					{#if selectedIncident.triage_result}
						<section class="triage-section">
							<h3>Diagnosis</h3>
							<div class="triage-card">
								<div class="triage-header">
									<span class="triage-verdict" class:fixable={selectedIncident.triage_result.fixable}>
										{selectedIncident.triage_result.fixable ? 'Treatable' : 'Requires Specialist'}
									</span>
									<span class="triage-confidence">
										{Math.round(selectedIncident.triage_result.confidence * 100)}% confidence
									</span>
								</div>
								<p class="triage-description">{selectedIncident.triage_result.description}</p>
								<div class="triage-details">
									<div class="detail-row">
										<span class="detail-label">Category:</span>
										<span class="detail-value">{selectedIncident.triage_result.category}</span>
									</div>
									{#if selectedIncident.triage_result.fix_approach}
										<div class="detail-row">
											<span class="detail-label">Treatment Plan:</span>
											<span class="detail-value">{selectedIncident.triage_result.fix_approach}</span>
										</div>
									{/if}
									{#if selectedIncident.triage_result.suggested_files.length > 0}
										<div class="detail-row">
											<span class="detail-label">Affected Files:</span>
											<div class="file-list">
												{#each selectedIncident.triage_result.suggested_files as file}
													<code class="file-path">{file}</code>
												{/each}
											</div>
										</div>
									{/if}
								</div>
							</div>
						</section>
					{/if}

					<!-- Sample -->
					{#if selectedIncident.samples.length > 0}
						<section class="sample-section">
							<h3>Sample Error</h3>
							<div class="sample-card">
								{#if selectedIncident.samples[0].stack_trace}
									<pre class="stack-trace">{selectedIncident.samples[0].stack_trace}</pre>
								{:else}
									<pre class="raw-log">{selectedIncident.samples[0].raw_log}</pre>
								{/if}
							</div>
						</section>
					{/if}

					<!-- Activity Log -->
					{#if selectedIncident.activity.length > 0}
						<section class="activity-section">
							<h3>Activity Log</h3>
							<div class="activity-list">
								{#each selectedIncident.activity as entry}
									<div class="activity-item">
										<span class="activity-time">{formatTime(entry.created_at)}</span>
										<span class="activity-action">{entry.action}</span>
										{#if entry.details}
											<span class="activity-details">{entry.details}</span>
										{/if}
									</div>
								{/each}
							</div>
						</section>
					{/if}
				</div>
			{:else}
				<div class="empty-detail">
					<div class="empty-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
							<path stroke-linecap="round" stroke-linejoin="round" d="M9 12h3.75M9 15h3.75M9 18h3.75m3 .75H18a2.25 2.25 0 002.25-2.25V6.108c0-1.135-.845-2.098-1.976-2.192a48.424 48.424 0 00-1.123-.08m-5.801 0c-.065.21-.1.433-.1.664 0 .414.336.75.75.75h4.5a.75.75 0 00.75-.75 2.25 2.25 0 00-.1-.664m-5.8 0A2.251 2.251 0 0113.5 2.25H15c1.012 0 1.867.668 2.15 1.586m-5.8 0c-.376.023-.75.05-1.124.08C9.095 4.01 8.25 4.973 8.25 6.108V8.25m0 0H4.875c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V9.375c0-.621-.504-1.125-1.125-1.125H8.25zM6.75 12h.008v.008H6.75V12zm0 3h.008v.008H6.75V15zm0 3h.008v.008H6.75V18z" />
						</svg>
					</div>
					<h3>Select a Case</h3>
					<p>Choose an incident from the list to view details</p>
				</div>
			{/if}
		</main>
	</div>
</div>

<style>
	.theatre {
		display: flex;
		flex-direction: column;
		height: calc(100vh - 120px);
	}

	/* Header */
	.theatre-header {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		padding-bottom: var(--space-lg);
		border-bottom: 1px solid var(--silver);
		margin-bottom: var(--space-lg);
	}

	.back-link {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		font-size: 0.875rem;
		color: var(--graphite);
	}

	.back-link svg {
		width: 16px;
		height: 16px;
	}

	.back-link:hover {
		color: var(--scrubs-dark);
	}

	.theatre-title {
		flex: 1;
	}

	.theatre-title h1 {
		font-size: 1.5rem;
		margin-bottom: var(--space-xs);
	}

	.repo-owner {
		color: var(--graphite);
		font-weight: 400;
	}

	.repo-name {
		color: var(--obsidian);
	}

	.theatre-subtitle {
		font-size: 0.875rem;
		color: var(--graphite);
	}

	.theatre-stats {
		display: flex;
		gap: var(--space-xl);
	}

	.stat {
		text-align: right;
	}

	.stat-value {
		display: block;
		font-family: var(--font-display);
		font-size: 1.5rem;
		color: var(--obsidian);
	}

	.stat-label {
		font-size: 0.75rem;
		color: var(--graphite);
	}

	/* Body */
	.theatre-body {
		display: flex;
		gap: var(--space-lg);
		flex: 1;
		min-height: 0;
	}

	/* Incident List */
	.incident-list {
		width: 360px;
		flex-shrink: 0;
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		display: flex;
		flex-direction: column;
	}

	.list-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-md) var(--space-lg);
		border-bottom: 1px solid var(--silver);
	}

	.list-header h2 {
		font-family: var(--font-body);
		font-size: 1rem;
		font-weight: 600;
	}

	.case-count {
		background: var(--paper);
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-full);
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--graphite);
	}

	.cases {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-sm);
	}

	.case-item {
		width: 100%;
		text-align: left;
		background: transparent;
		padding: var(--space-md);
		border-radius: var(--radius-md);
		margin-bottom: var(--space-xs);
		transition: all var(--transition-fast);
	}

	.case-item:hover {
		background: var(--paper);
	}

	.case-item.selected {
		background: var(--scrubs-soft);
	}

	.case-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: var(--space-sm);
	}

	.case-time {
		font-size: 0.75rem;
		color: var(--slate);
	}

	.case-message {
		font-size: 0.875rem;
		color: var(--charcoal);
		margin-bottom: var(--space-sm);
		display: -webkit-box;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.case-meta {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.case-samples {
		font-size: 0.75rem;
		color: var(--slate);
	}

	.badge-error { background: var(--critical-soft); color: #B91C1C; }
	.badge-warn { background: var(--alert-soft); color: #B45309; }
	.badge-info { background: var(--vitals-soft); color: #1D4ED8; }

	.empty-list {
		padding: var(--space-xl);
		text-align: center;
		color: var(--graphite);
	}

	/* Incident Detail */
	.incident-detail {
		flex: 1;
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		overflow-y: auto;
	}

	.loading-state,
	.empty-detail {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 100%;
		color: var(--graphite);
	}

	.spinner {
		width: 32px;
		height: 32px;
		border: 3px solid var(--paper);
		border-top-color: var(--scrubs);
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	.empty-icon {
		width: 64px;
		height: 64px;
		color: var(--slate);
		margin-bottom: var(--space-md);
	}

	.empty-icon svg {
		width: 100%;
		height: 100%;
	}

	.empty-detail h3 {
		font-family: var(--font-body);
		font-size: 1rem;
		font-weight: 600;
		margin-bottom: var(--space-xs);
	}

	.detail-content {
		padding: var(--space-xl);
	}

	.detail-header {
		display: flex;
		justify-content: space-between;
		gap: var(--space-lg);
		margin-bottom: var(--space-xl);
	}

	.detail-header h2 {
		font-size: 1.25rem;
		margin: var(--space-sm) 0;
	}

	.detail-meta {
		font-size: 0.8125rem;
		color: var(--graphite);
	}

	.detail-actions {
		display: flex;
		gap: var(--space-sm);
		align-items: flex-start;
	}

	.badge.large {
		font-size: 0.8125rem;
		padding: var(--space-xs) var(--space-md);
	}

	/* Pipeline */
	.pipeline-section,
	.triage-section,
	.sample-section,
	.activity-section {
		margin-bottom: var(--space-xl);
	}

	.pipeline-section h3,
	.triage-section h3,
	.sample-section h3,
	.activity-section h3 {
		font-family: var(--font-body);
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--charcoal);
		margin-bottom: var(--space-md);
	}

	.pipeline {
		display: flex;
		align-items: center;
		gap: 0;
	}

	.pipeline-step {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-xs);
	}

	.step-icon {
		width: 32px;
		height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
		background: var(--paper);
		color: var(--slate);
		font-size: 0.875rem;
		font-weight: 600;
	}

	.pipeline-step.complete .step-icon {
		background: var(--scrubs);
		color: white;
	}

	.pipeline-step.active .step-icon {
		background: var(--alert);
		color: white;
		animation: pulse 2s ease-in-out infinite;
	}

	.step-label {
		font-size: 0.75rem;
		color: var(--graphite);
	}

	.pipeline-connector {
		flex: 1;
		height: 2px;
		background: var(--silver);
		margin: 0 var(--space-sm);
		margin-bottom: 20px;
	}

	.pipeline-connector.complete {
		background: var(--scrubs);
	}

	/* Triage */
	.triage-card {
		background: var(--paper);
		border-radius: var(--radius-md);
		padding: var(--space-lg);
	}

	.triage-header {
		display: flex;
		justify-content: space-between;
		margin-bottom: var(--space-md);
	}

	.triage-verdict {
		font-weight: 600;
		color: var(--critical);
	}

	.triage-verdict.fixable {
		color: var(--scrubs-dark);
	}

	.triage-confidence {
		font-size: 0.875rem;
		color: var(--graphite);
	}

	.triage-description {
		color: var(--charcoal);
		margin-bottom: var(--space-md);
	}

	.triage-details {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.detail-row {
		display: flex;
		gap: var(--space-md);
	}

	.detail-label {
		font-size: 0.8125rem;
		color: var(--graphite);
		min-width: 120px;
	}

	.detail-value {
		font-size: 0.8125rem;
		color: var(--charcoal);
	}

	.file-list {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-xs);
	}

	.file-path {
		font-size: 0.75rem;
		background: var(--white);
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-sm);
	}

	/* Sample */
	.sample-card {
		background: var(--obsidian);
		border-radius: var(--radius-md);
		padding: var(--space-lg);
		overflow-x: auto;
	}

	.stack-trace,
	.raw-log {
		margin: 0;
		font-family: 'SF Mono', 'Fira Code', monospace;
		font-size: 0.8125rem;
		line-height: 1.6;
		color: var(--silver);
		white-space: pre-wrap;
		word-break: break-word;
	}

	/* Activity */
	.activity-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.activity-item {
		display: flex;
		align-items: baseline;
		gap: var(--space-md);
		padding: var(--space-sm) 0;
		border-bottom: 1px solid var(--paper);
	}

	.activity-time {
		font-size: 0.75rem;
		color: var(--slate);
		min-width: 160px;
	}

	.activity-action {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--charcoal);
	}

	.activity-details {
		font-size: 0.8125rem;
		color: var(--graphite);
	}

	@media (max-width: 1024px) {
		.theatre-body {
			flex-direction: column;
		}

		.incident-list {
			width: 100%;
			max-height: 300px;
		}
	}
</style>
