<script lang="ts">
	import { pendingPRs, projects, incidents } from '$lib/stores';
	import type { Incident } from '$lib/api';

	let selectedPR = $state<Incident | null>(null);
	let loadingDetails = $state(false);

	// Fetch full incident details (works in both demo and live mode via store)
	async function selectPR(incident: Incident) {
		loadingDetails = true;
		try {
			const fullIncident = await incidents.get(incident.id);
			selectedPR = { ...incident, ...fullIncident };
		} catch (e) {
			console.error('Failed to load incident:', e);
			selectedPR = incident; // fallback to basic incident data
		} finally {
			loadingDetails = false;
		}
	}

	function getProjectRepo(projectId: string): string {
		const project = $projects.find(p => p.id === projectId);
		return project?.repo ?? 'Unknown';
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

	function getConfidenceLevel(confidence: number): 'high' | 'medium' | 'low' {
		if (confidence >= 0.9) return 'high';
		if (confidence >= 0.7) return 'medium';
		return 'low';
	}
</script>

<svelte:head>
	<title>Recovery Ward - CRISPR</title>
</svelte:head>

<div class="recovery">
	<header class="page-header">
		<h1>Recovery Ward</h1>
		<p class="page-subtitle">Review fixes before discharge (merge)</p>
	</header>

	{#if $pendingPRs.length === 0}
		<div class="empty-state">
			<div class="empty-icon">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
				</svg>
			</div>
			<h2>All Clear</h2>
			<p>No pull requests awaiting review. All patients have been discharged.</p>
		</div>
	{:else}
		<div class="recovery-layout">
			<!-- PR List -->
			<aside class="pr-list">
				<div class="list-header">
					<span class="list-title">Awaiting Review</span>
					<span class="list-count">{$pendingPRs.length}</span>
				</div>

				<div class="pr-items">
					{#each $pendingPRs as incident (incident.id)}
						<button
							class="pr-item"
							class:selected={selectedPR?.id === incident.id}
							onclick={() => selectPR(incident)}
						>
							<div class="pr-item-header">
								<span class="pr-repo">{getProjectRepo(incident.project_id).split('/')[1] ?? getProjectRepo(incident.project_id)}</span>
								{#if incident.pr_number}
									<span class="pr-number">#{incident.pr_number}</span>
								{/if}
							</div>
							<p class="pr-message">{incident.message}</p>
							<div class="pr-item-footer">
								<span class="confidence confidence-{getConfidenceLevel(incident.triage_result?.confidence ?? 0)}">
									{Math.round((incident.triage_result?.confidence ?? 0) * 100)}%
								</span>
								<span class="pr-time">{formatTimeAgo(incident.last_seen)}</span>
							</div>
						</button>
					{/each}
				</div>
			</aside>

			<!-- PR Detail -->
			<main class="pr-detail">
				{#if loadingDetails}
					<div class="loading-state">
						<div class="spinner"></div>
						<p>Loading fix details...</p>
					</div>
				{:else if selectedPR}
					<div class="detail-content">
						<!-- Header with GitHub Link -->
						<div class="detail-header">
							<div class="header-top">
								<div class="header-badges">
									<span class="pr-badge">PR #{selectedPR.pr_number}</span>
									<span class="confidence-badge confidence-{getConfidenceLevel(selectedPR.triage_result?.confidence ?? 0)}">
										{Math.round((selectedPR.triage_result?.confidence ?? 0) * 100)}% confidence
									</span>
								</div>
								{#if selectedPR.pr_url}
									<a href={selectedPR.pr_url} target="_blank" rel="noopener" class="github-link">
										<svg viewBox="0 0 24 24" fill="currentColor" width="20" height="20">
											<path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
										</svg>
										Review & Merge on GitHub
										<svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
											<path fill-rule="evenodd" d="M4.25 5.5a.75.75 0 00-.75.75v8.5c0 .414.336.75.75.75h8.5a.75.75 0 00.75-.75v-4a.75.75 0 011.5 0v4A2.25 2.25 0 0112.75 17h-8.5A2.25 2.25 0 012 14.75v-8.5A2.25 2.25 0 014.25 4h5a.75.75 0 010 1.5h-5z" clip-rule="evenodd"/>
											<path fill-rule="evenodd" d="M6.194 12.753a.75.75 0 001.06.053L16.5 4.44v2.81a.75.75 0 001.5 0v-4.5a.75.75 0 00-.75-.75h-4.5a.75.75 0 000 1.5h2.553l-9.056 8.194a.75.75 0 00-.053 1.06z" clip-rule="evenodd"/>
										</svg>
									</a>
								{/if}
							</div>
							<div class="header-info">
								<span class="header-repo">{getProjectRepo(selectedPR.project_id)}</span>
								<h2>{selectedPR.message}</h2>
							</div>
						</div>

						<!-- Fix Explanation Cards -->
						<div class="fix-explanation">
							<!-- What Went Wrong -->
							<section class="explanation-card problem">
								<div class="card-header">
									<span class="card-icon">🔴</span>
									<h3>What Went Wrong</h3>
								</div>
								<p class="card-content">
									{selectedPR.triage_result?.description ?? 'No description available'}
								</p>
								<div class="card-meta">
									<span class="meta-label">Category</span>
									<span class="meta-value">{selectedPR.triage_result?.category ?? 'unknown'}</span>
								</div>
							</section>

							<!-- What We Fixed -->
							<section class="explanation-card solution">
								<div class="card-header">
									<span class="card-icon">🔧</span>
									<h3>What We Fixed</h3>
								</div>
								<p class="card-content">
									{selectedPR.triage_result?.what_changed ?? selectedPR.triage_result?.fix_approach ?? 'Applied automated fix based on error pattern analysis'}
								</p>
								{#if selectedPR.triage_result?.suggested_files && selectedPR.triage_result.suggested_files.length > 0}
									<div class="files-changed">
										<span class="files-label">Files Modified</span>
										<div class="files-list">
											{#each selectedPR.triage_result.suggested_files as file}
												<code class="file-chip">{file}</code>
											{/each}
										</div>
									</div>
								{/if}
							</section>

							<!-- Expected Behavior -->
							<section class="explanation-card outcome">
								<div class="card-header">
									<span class="card-icon">✅</span>
									<h3>Expected Behavior</h3>
								</div>
								<p class="card-content">
									{selectedPR.triage_result?.expected_behavior ?? 'The error should no longer occur after this fix is merged.'}
								</p>
							</section>
						</div>

						<!-- Stats Row -->
						<div class="stats-row">
							<div class="stat-item">
								<span class="stat-value">{selectedPR.sample_count}</span>
								<span class="stat-label">Occurrences</span>
							</div>
							<div class="stat-item">
								<span class="stat-value">{formatTimeAgo(selectedPR.first_seen)}</span>
								<span class="stat-label">First Seen</span>
							</div>
							<div class="stat-item">
								<span class="stat-value">{selectedPR.fix_attempts}</span>
								<span class="stat-label">Fix Attempts</span>
							</div>
						</div>

						<!-- Action Footer -->
						<div class="action-footer">
							{#if selectedPR.pr_url}
								<a href={selectedPR.pr_url} target="_blank" rel="noopener" class="btn btn-primary btn-large">
									<svg viewBox="0 0 20 20" fill="currentColor" width="20" height="20">
										<path fill-rule="evenodd" d="M16.704 4.153a.75.75 0 01.143 1.052l-8 10.5a.75.75 0 01-1.127.075l-4.5-4.5a.75.75 0 011.06-1.06l3.894 3.893 7.48-9.817a.75.75 0 011.05-.143z" clip-rule="evenodd"/>
									</svg>
									Review & Approve on GitHub
								</a>
							{/if}
							<p class="action-hint">
								Once merged, the patient will be automatically discharged from the system.
							</p>
						</div>
					</div>
				{:else}
					<div class="empty-detail">
						<div class="empty-icon">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
								<path stroke-linecap="round" stroke-linejoin="round" d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 01-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 011.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 00-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 01-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 00-3.375-3.375h-1.5a1.125 1.125 0 01-1.125-1.125v-1.5a3.375 3.375 0 00-3.375-3.375H9.75" />
							</svg>
						</div>
						<h3>Select a Fix to Review</h3>
						<p>Choose a pending PR from the list to see what was fixed</p>
					</div>
				{/if}
			</main>
		</div>
	{/if}
</div>

<style>
	.recovery {
		height: calc(100vh - 120px);
		display: flex;
		flex-direction: column;
	}

	.page-header {
		margin-bottom: var(--space-xl);
	}

	.page-header h1 {
		font-size: 1.75rem;
		margin-bottom: var(--space-xs);
	}

	.page-subtitle {
		color: var(--graphite);
	}

	/* Empty State */
	.empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-xl);
	}

	.empty-state .empty-icon,
	.empty-detail .empty-icon {
		width: 80px;
		height: 80px;
		color: var(--scrubs);
		margin-bottom: var(--space-lg);
	}

	.empty-icon svg {
		width: 100%;
		height: 100%;
	}

	.empty-state h2,
	.empty-detail h3 {
		font-size: 1.25rem;
		margin-bottom: var(--space-sm);
	}

	.empty-state p,
	.empty-detail p {
		color: var(--graphite);
	}

	/* Layout */
	.recovery-layout {
		display: flex;
		gap: var(--space-lg);
		flex: 1;
		min-height: 0;
	}

	/* PR List */
	.pr-list {
		width: 320px;
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

	.list-title {
		font-weight: 600;
		color: var(--charcoal);
	}

	.list-count {
		background: var(--stable-soft);
		color: var(--stable-dark);
		padding: 2px 8px;
		border-radius: var(--radius-full);
		font-size: 0.75rem;
		font-weight: 600;
	}

	.pr-items {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-sm);
	}

	.pr-item {
		width: 100%;
		text-align: left;
		background: transparent;
		padding: var(--space-md);
		border-radius: var(--radius-md);
		margin-bottom: var(--space-xs);
		transition: all var(--transition-fast);
		border: 1px solid transparent;
		cursor: pointer;
	}

	.pr-item:hover {
		background: var(--paper);
	}

	.pr-item.selected {
		background: var(--stable-soft);
		border-color: var(--stable);
	}

	.pr-item-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: var(--space-xs);
	}

	.pr-repo {
		font-weight: 600;
		font-size: 0.875rem;
		color: var(--charcoal);
	}

	.pr-number {
		font-size: 0.75rem;
		color: var(--stable-dark);
		font-weight: 600;
	}

	.pr-message {
		font-size: 0.8125rem;
		color: var(--graphite);
		margin-bottom: var(--space-sm);
		display: -webkit-box;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		line-height: 1.4;
	}

	.pr-item-footer {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.confidence {
		font-size: 0.75rem;
		font-weight: 600;
	}

	.confidence-high { color: var(--scrubs-dark); }
	.confidence-medium { color: #B45309; }
	.confidence-low { color: #B91C1C; }

	.pr-time {
		font-size: 0.75rem;
		color: var(--slate);
	}

	/* PR Detail */
	.pr-detail {
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

	.detail-content {
		padding: var(--space-xl);
	}

	/* Detail Header */
	.detail-header {
		margin-bottom: var(--space-xl);
		padding-bottom: var(--space-lg);
		border-bottom: 1px solid var(--silver);
	}

	.header-top {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: var(--space-md);
	}

	.header-badges {
		display: flex;
		gap: var(--space-sm);
	}

	.pr-badge {
		background: var(--stable-soft);
		color: var(--stable-dark);
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-sm);
		font-size: 0.75rem;
		font-weight: 600;
	}

	.confidence-badge {
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-sm);
		font-size: 0.75rem;
		font-weight: 600;
	}

	.confidence-badge.confidence-high {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.confidence-badge.confidence-medium {
		background: var(--alert-soft);
		color: #B45309;
	}

	.confidence-badge.confidence-low {
		background: var(--critical-soft);
		color: #B91C1C;
	}

	.github-link {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-sm) var(--space-md);
		background: var(--obsidian);
		color: var(--white);
		border-radius: var(--radius-md);
		font-size: 0.875rem;
		font-weight: 500;
		text-decoration: none;
		transition: all var(--transition-fast);
	}

	.github-link:hover {
		background: var(--charcoal);
	}

	.header-info {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.header-repo {
		font-size: 0.8125rem;
		color: var(--graphite);
		font-family: var(--font-mono);
	}

	.detail-header h2 {
		font-size: 1.125rem;
		color: var(--charcoal);
		line-height: 1.4;
	}

	/* Fix Explanation Cards */
	.fix-explanation {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		margin-bottom: var(--space-xl);
	}

	.explanation-card {
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		border-left: 4px solid;
	}

	.explanation-card.problem {
		background: linear-gradient(to right, var(--critical-soft), var(--paper));
		border-color: var(--critical);
	}

	.explanation-card.solution {
		background: linear-gradient(to right, var(--vitals-soft), var(--paper));
		border-color: var(--vitals);
	}

	.explanation-card.outcome {
		background: linear-gradient(to right, var(--scrubs-soft), var(--paper));
		border-color: var(--scrubs);
	}

	.card-header {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin-bottom: var(--space-sm);
	}

	.card-icon {
		font-size: 1.25rem;
	}

	.card-header h3 {
		font-size: 0.9375rem;
		font-weight: 600;
		color: var(--charcoal);
	}

	.card-content {
		font-size: 0.9375rem;
		color: var(--charcoal);
		line-height: 1.6;
	}

	.card-meta {
		display: flex;
		gap: var(--space-sm);
		margin-top: var(--space-md);
		padding-top: var(--space-md);
		border-top: 1px solid rgba(0, 0, 0, 0.1);
	}

	.meta-label {
		font-size: 0.75rem;
		color: var(--graphite);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.meta-value {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--charcoal);
		text-transform: capitalize;
	}

	.files-changed {
		margin-top: var(--space-md);
		padding-top: var(--space-md);
		border-top: 1px solid rgba(0, 0, 0, 0.1);
	}

	.files-label {
		display: block;
		font-size: 0.75rem;
		color: var(--graphite);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		margin-bottom: var(--space-sm);
	}

	.files-list {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-xs);
	}

	.file-chip {
		font-size: 0.75rem;
		background: var(--white);
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-sm);
		border: 1px solid var(--silver);
	}

	/* Stats Row */
	.stats-row {
		display: flex;
		gap: var(--space-xl);
		padding: var(--space-lg);
		background: var(--paper);
		border-radius: var(--radius-lg);
		margin-bottom: var(--space-xl);
	}

	.stat-item {
		display: flex;
		flex-direction: column;
	}

	.stat-value {
		font-family: var(--font-display);
		font-size: 1.25rem;
		color: var(--obsidian);
	}

	.stat-label {
		font-size: 0.75rem;
		color: var(--graphite);
		text-transform: uppercase;
	}

	/* Action Footer */
	.action-footer {
		text-align: center;
		padding-top: var(--space-lg);
		border-top: 1px solid var(--silver);
	}

	.btn-large {
		padding: var(--space-md) var(--space-xl);
		font-size: 1rem;
	}

	.btn-large svg {
		margin-right: var(--space-sm);
	}

	.action-hint {
		margin-top: var(--space-md);
		font-size: 0.8125rem;
		color: var(--slate);
	}

	@media (max-width: 1024px) {
		.recovery-layout {
			flex-direction: column;
		}

		.pr-list {
			width: 100%;
			max-height: 250px;
		}

		.header-top {
			flex-direction: column;
			align-items: flex-start;
			gap: var(--space-md);
		}

		.stats-row {
			flex-wrap: wrap;
		}
	}
</style>
