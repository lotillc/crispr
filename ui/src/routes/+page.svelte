<script lang="ts">
	import { projects, incidents, stats, activeIncidents, pendingPRs } from '$lib/stores';
	import type { Project, Incident } from '$lib/api';

	let activeTab = $state<'patients' | 'services'>('patients');
	let showAddService = $state(false);
	let newServiceRepo = $state('');
	let addingService = $state(false);

	// Group incidents by stage for kanban board
	let triageQueue = $derived(
		$incidents.filter(i => i.status === 'detected' || i.status === 'triaging')
	);

	let inSurgery = $derived(
		$incidents.filter(i => i.status === 'fixing' || i.status === 'pending_fix')
	);

	let inRecovery = $derived(
		$incidents.filter(i => i.status === 'pr_open')
	);

	let discharged = $derived(
		$incidents.filter(i => i.status === 'pr_merged').slice(0, 10)
	);

	let needsAttention = $derived(
		$incidents.filter(i => i.status === 'unfixable' || i.status === 'max_attempts')
	);

	function getStatusLabel(status: string): string {
		switch (status) {
			case 'detected': return 'Waiting';
			case 'triaging': return 'Analyzing';
			case 'pending_fix': return 'Queued';
			case 'fixing': return 'Operating';
			case 'pr_open': return 'Awaiting Merge';
			case 'pr_merged': return 'Discharged';
			case 'unfixable': return 'Manual Review';
			case 'max_attempts': return 'Specialist Needed';
			case 'ignored': return 'Dismissed';
			default: return status;
		}
	}

	function getServiceStatus(project: Project): 'healthy' | 'observation' | 'surgery' | 'critical' {
		const projectIncidents = $incidents.filter(i => i.project_id === project.id);
		const activeCount = projectIncidents.filter(i =>
			!['pr_merged', 'ignored'].includes(i.status)
		).length;

		if (activeCount === 0) return 'healthy';
		if (activeCount > 5) return 'critical';
		if (projectIncidents.some(i => ['fixing', 'triaging'].includes(i.status))) return 'surgery';
		return 'observation';
	}

	function getServiceStatusLabel(status: string): string {
		switch (status) {
			case 'healthy': return 'Stable';
			case 'observation': return 'Under Observation';
			case 'surgery': return 'In Surgery';
			case 'critical': return 'Critical';
			default: return status;
		}
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

	function getRepoName(incident: Incident): string {
		return incident.project?.repo?.split('/')[1] ?? 'Unknown';
	}

	async function handleAddService() {
		if (!newServiceRepo.trim()) return;
		addingService = true;
		try {
			await projects.create(newServiceRepo.trim());
			newServiceRepo = '';
			showAddService = false;
		} catch (e) {
			console.error('Failed to add service:', e);
		} finally {
			addingService = false;
		}
	}

	async function toggleWatchLogs(project: Project, e: Event) {
		e.preventDefault();
		e.stopPropagation();
		await projects.update(project.id, { watch_logs: !project.watch_logs });
	}

	async function toggleAutoFix(project: Project, e: Event) {
		e.preventDefault();
		e.stopPropagation();
		await projects.update(project.id, { auto_fix: !project.auto_fix });
	}
</script>

<svelte:head>
	<title>Surgery Board - CRISPR</title>
</svelte:head>

<div class="dashboard">
	<!-- Header HUD -->
	<header class="hud">
		<div class="hud-stat">
			<span class="hud-value">{$stats?.fixed_today ?? 0}</span>
			<span class="hud-label">Discharged Today</span>
		</div>
		<div class="hud-stat">
			<span class="hud-value">{$stats?.fixed_week ?? 0}</span>
			<span class="hud-label">This Week</span>
		</div>
		<div class="hud-stat highlight">
			<span class="hud-value">{inRecovery.length}</span>
			<span class="hud-label">Awaiting Approval</span>
		</div>
		<div class="hud-stat">
			<span class="hud-value">{triageQueue.length + inSurgery.length}</span>
			<span class="hud-label">In Treatment</span>
		</div>
	</header>

	<!-- Tab Navigation -->
	<div class="tab-nav">
		<button
			class="tab-btn"
			class:active={activeTab === 'patients'}
			onclick={() => activeTab = 'patients'}
		>
			<svg viewBox="0 0 20 20" fill="currentColor">
				<path d="M9 6a3 3 0 11-6 0 3 3 0 016 0zM17 6a3 3 0 11-6 0 3 3 0 016 0zM12.93 17c.046-.327.07-.66.07-1a6.97 6.97 0 00-1.5-4.33A5 5 0 0119 16v1h-6.07zM6 11a5 5 0 015 5v1H1v-1a5 5 0 015-5z"/>
			</svg>
			Patients
			{#if $activeIncidents.length > 0}
				<span class="tab-badge">{$activeIncidents.length}</span>
			{/if}
		</button>
		<button
			class="tab-btn"
			class:active={activeTab === 'services'}
			onclick={() => activeTab = 'services'}
		>
			<svg viewBox="0 0 20 20" fill="currentColor">
				<path d="M7 3a1 1 0 000 2h6a1 1 0 100-2H7zM4 7a1 1 0 011-1h10a1 1 0 110 2H5a1 1 0 01-1-1zM2 11a2 2 0 012-2h12a2 2 0 012 2v4a2 2 0 01-2 2H4a2 2 0 01-2-2v-4z"/>
			</svg>
			Services
			<span class="tab-count">{$projects.length}</span>
		</button>
	</div>

	<!-- Patients Tab: Kanban Board -->
	{#if activeTab === 'patients'}
		<div class="kanban-board">
			<!-- Triage Column -->
			<div class="kanban-column">
				<div class="column-header triage">
					<div class="column-title">
						<span class="column-icon">🔬</span>
						<span>Triage</span>
					</div>
					<span class="column-count">{triageQueue.length}</span>
				</div>
				<div class="column-cards">
					{#each triageQueue as incident (incident.id)}
						<a href="/theatre/{incident.project_id}?incident={incident.id}" class="patient-card">
							<div class="patient-header">
								<span class="patient-repo">{getRepoName(incident)}</span>
								<span class="patient-status status-{incident.status}">{getStatusLabel(incident.status)}</span>
							</div>
							<p class="patient-message">{incident.message}</p>
							<div class="patient-footer">
								<span class="patient-samples">{incident.sample_count} occurrences</span>
								<span class="patient-time">{formatTimeAgo(incident.last_seen)}</span>
							</div>
						</a>
					{:else}
						<div class="column-empty">
							<span>Queue clear</span>
						</div>
					{/each}
				</div>
			</div>

			<!-- Surgery Column -->
			<div class="kanban-column">
				<div class="column-header surgery">
					<div class="column-title">
						<span class="column-icon">🏥</span>
						<span>In Surgery</span>
					</div>
					<span class="column-count">{inSurgery.length}</span>
				</div>
				<div class="column-cards">
					{#each inSurgery as incident (incident.id)}
						<a href="/theatre/{incident.project_id}?incident={incident.id}" class="patient-card active">
							<div class="patient-header">
								<span class="patient-repo">{getRepoName(incident)}</span>
								<span class="patient-status status-fixing">
									<span class="pulse-dot"></span>
									{getStatusLabel(incident.status)}
								</span>
							</div>
							<p class="patient-message">{incident.message}</p>
							<div class="patient-footer">
								{#if incident.triage_result}
									<span class="patient-confidence">{Math.round(incident.triage_result.confidence * 100)}% confidence</span>
								{/if}
								<span class="patient-time">{formatTimeAgo(incident.last_seen)}</span>
							</div>
						</a>
					{:else}
						<div class="column-empty">
							<span>No active surgeries</span>
						</div>
					{/each}
				</div>
			</div>

			<!-- Recovery Column -->
			<div class="kanban-column">
				<div class="column-header recovery">
					<div class="column-title">
						<span class="column-icon">🩹</span>
						<span>Recovery</span>
					</div>
					<span class="column-count">{inRecovery.length}</span>
				</div>
				<div class="column-cards">
					{#each inRecovery as incident (incident.id)}
						<a href="/recovery" class="patient-card recovery">
							<div class="patient-header">
								<span class="patient-repo">{getRepoName(incident)}</span>
								{#if incident.pr_number}
									<span class="patient-pr">PR #{incident.pr_number}</span>
								{/if}
							</div>
							<p class="patient-message">{incident.message}</p>
							<div class="patient-footer">
								{#if incident.triage_result}
									<span class="patient-confidence high">{Math.round(incident.triage_result.confidence * 100)}%</span>
								{/if}
								<span class="patient-time">{formatTimeAgo(incident.last_seen)}</span>
							</div>
						</a>
					{:else}
						<div class="column-empty">
							<span>No PRs waiting</span>
						</div>
					{/each}
				</div>
			</div>

			<!-- Needs Attention Column -->
			{#if needsAttention.length > 0}
				<div class="kanban-column attention">
					<div class="column-header attention">
						<div class="column-title">
							<span class="column-icon">⚠️</span>
							<span>Needs Attention</span>
						</div>
						<span class="column-count">{needsAttention.length}</span>
					</div>
					<div class="column-cards">
						{#each needsAttention as incident (incident.id)}
							<a href="/theatre/{incident.project_id}?incident={incident.id}" class="patient-card warning">
								<div class="patient-header">
									<span class="patient-repo">{getRepoName(incident)}</span>
									<span class="patient-status status-warning">{getStatusLabel(incident.status)}</span>
								</div>
								<p class="patient-message">{incident.message}</p>
								<div class="patient-footer">
									{#if incident.triage_result?.reason}
										<span class="patient-reason">{incident.triage_result.reason}</span>
									{/if}
								</div>
							</a>
						{/each}
					</div>
				</div>
			{/if}
		</div>

		<!-- Recently Discharged -->
		{#if discharged.length > 0}
			<section class="discharged-section">
				<h3>Recently Discharged</h3>
				<div class="discharged-list">
					{#each discharged as incident (incident.id)}
						<div class="discharged-item">
							<span class="discharged-repo">{getRepoName(incident)}</span>
							<span class="discharged-message">{incident.message}</span>
							<span class="discharged-time">{formatTimeAgo(incident.last_seen)}</span>
						</div>
					{/each}
				</div>
			</section>
		{/if}
	{/if}

	<!-- Services Tab -->
	{#if activeTab === 'services'}
		<section class="services-section">
			<div class="section-header">
				<h2>Monitored Services</h2>
				<button class="btn btn-primary" onclick={() => showAddService = true}>
					<svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
						<path fill-rule="evenodd" d="M10 3a1 1 0 011 1v5h5a1 1 0 110 2h-5v5a1 1 0 11-2 0v-5H4a1 1 0 110-2h5V4a1 1 0 011-1z" clip-rule="evenodd"/>
					</svg>
					Add Service
				</button>
			</div>

			{#if $projects.length === 0}
				<div class="empty-state">
					<div class="empty-icon">
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
							<path stroke-linecap="round" stroke-linejoin="round" d="M12 9v6m3-3H9m12 0a9 9 0 11-18 0 9 9 0 0118 0z" />
						</svg>
					</div>
					<h3>No Services Registered</h3>
					<p>Add a GitHub repository to begin monitoring for errors.</p>
					<button class="btn btn-primary" onclick={() => showAddService = true}>
						Add Your First Service
					</button>
				</div>
			{:else}
				<div class="service-grid">
					{#each $projects as project (project.id)}
						{@const status = getServiceStatus(project)}
						{@const projectIncidents = $incidents.filter(i => i.project_id === project.id)}
						{@const activeCount = projectIncidents.filter(i => !['pr_merged', 'ignored'].includes(i.status)).length}
						<a href="/theatre/{project.id}" class="service-card" class:critical={status === 'critical'}>
							<div class="service-header">
								<div class="service-name">
									<span class="repo-owner">{project.repo.split('/')[0]}/</span>
									<span class="repo-name">{project.repo.split('/')[1]}</span>
								</div>
								<span class="badge badge-{status}">{getServiceStatusLabel(status)}</span>
							</div>

							<div class="service-stats">
								<div class="stat">
									<span class="stat-value">{activeCount}</span>
									<span class="stat-label">Active</span>
								</div>
								<div class="stat">
									<span class="stat-value">{projectIncidents.filter(i => i.status === 'pr_open').length}</span>
									<span class="stat-label">PRs Open</span>
								</div>
								<div class="stat">
									<span class="stat-value">{projectIncidents.filter(i => i.status === 'pr_merged').length}</span>
									<span class="stat-label">Fixed</span>
								</div>
							</div>

							<div class="service-controls">
								<button
									class="control-btn"
									class:active={project.watch_logs}
									onclick={(e) => toggleWatchLogs(project, e)}
									title={project.watch_logs ? 'Monitoring active' : 'Monitoring paused'}
								>
									<svg viewBox="0 0 20 20" fill="currentColor">
										<path d="M10 12a2 2 0 100-4 2 2 0 000 4z"/>
										<path fill-rule="evenodd" d="M.458 10C1.732 5.943 5.522 3 10 3s8.268 2.943 9.542 7c-1.274 4.057-5.064 7-9.542 7S1.732 14.057.458 10zM14 10a4 4 0 11-8 0 4 4 0 018 0z" clip-rule="evenodd"/>
									</svg>
								</button>
								<button
									class="control-btn"
									class:active={project.auto_fix}
									onclick={(e) => toggleAutoFix(project, e)}
									title={project.auto_fix ? 'Auto-fix enabled' : 'Auto-fix disabled'}
								>
									<svg viewBox="0 0 20 20" fill="currentColor">
										<path fill-rule="evenodd" d="M11.3 1.046A1 1 0 0112 2v5h4a1 1 0 01.82 1.573l-7 10A1 1 0 018 18v-5H4a1 1 0 01-.82-1.573l7-10a1 1 0 011.12-.38z" clip-rule="evenodd"/>
									</svg>
								</button>
							</div>
						</a>
					{/each}
				</div>
			{/if}
		</section>
	{/if}
</div>

<!-- Add Service Modal -->
{#if showAddService}
	<div class="modal-overlay" onclick={() => showAddService = false}>
		<div class="modal" onclick={(e) => e.stopPropagation()}>
			<div class="modal-header">
				<h3>Add New Service</h3>
				<button class="modal-close" onclick={() => showAddService = false}>
					<svg viewBox="0 0 20 20" fill="currentColor">
						<path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd"/>
					</svg>
				</button>
			</div>
			<div class="modal-body">
				<label class="field">
					<span class="field-label">GitHub Repository</span>
					<input
						type="text"
						placeholder="owner/repository"
						bind:value={newServiceRepo}
						class="field-input"
					/>
					<span class="field-hint">Enter the full repository path (e.g., acme/api-server)</span>
				</label>
			</div>
			<div class="modal-footer">
				<button class="btn btn-secondary" onclick={() => showAddService = false}>Cancel</button>
				<button class="btn btn-primary" onclick={handleAddService} disabled={addingService || !newServiceRepo.trim()}>
					{addingService ? 'Adding...' : 'Add Service'}
				</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.dashboard {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	/* HUD Stats Bar */
	.hud {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: var(--space-md);
	}

	.hud-stat {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-lg);
		text-align: center;
	}

	.hud-stat.highlight {
		background: var(--scrubs-soft);
		border-color: var(--scrubs);
	}

	.hud-value {
		display: block;
		font-family: var(--font-display);
		font-size: 2.5rem;
		color: var(--obsidian);
		line-height: 1;
	}

	.hud-stat.highlight .hud-value {
		color: var(--scrubs-dark);
	}

	.hud-label {
		display: block;
		font-size: 0.75rem;
		color: var(--graphite);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		margin-top: var(--space-sm);
	}

	/* Tab Navigation */
	.tab-nav {
		display: flex;
		gap: var(--space-sm);
		border-bottom: 1px solid var(--silver);
		padding-bottom: var(--space-sm);
	}

	.tab-btn {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-sm) var(--space-lg);
		background: transparent;
		border: none;
		border-radius: var(--radius-md);
		font-size: 0.9375rem;
		font-weight: 500;
		color: var(--graphite);
		cursor: pointer;
		transition: all var(--transition-fast);
	}

	.tab-btn:hover {
		background: var(--paper);
		color: var(--charcoal);
	}

	.tab-btn.active {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.tab-btn svg {
		width: 18px;
		height: 18px;
	}

	.tab-badge {
		background: var(--alert);
		color: white;
		font-size: 0.6875rem;
		font-weight: 700;
		padding: 2px 6px;
		border-radius: var(--radius-full);
	}

	.tab-count {
		font-size: 0.75rem;
		color: var(--slate);
	}

	/* Kanban Board */
	.kanban-board {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
		gap: var(--space-lg);
		align-items: start;
	}

	.kanban-column {
		background: var(--paper);
		border-radius: var(--radius-lg);
		min-height: 400px;
		display: flex;
		flex-direction: column;
	}

	.kanban-column.attention {
		background: var(--caution-soft);
	}

	.column-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-md) var(--space-lg);
		border-bottom: 2px solid;
	}

	.column-header.triage {
		border-color: var(--vitals);
	}

	.column-header.surgery {
		border-color: var(--scrubs);
	}

	.column-header.recovery {
		border-color: var(--stable);
	}

	.column-header.attention {
		border-color: var(--caution);
	}

	.column-title {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		font-weight: 600;
		font-size: 0.9375rem;
	}

	.column-icon {
		font-size: 1rem;
	}

	.column-count {
		background: var(--white);
		padding: 2px 8px;
		border-radius: var(--radius-full);
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--graphite);
	}

	.column-cards {
		flex: 1;
		padding: var(--space-md);
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		overflow-y: auto;
		max-height: 600px;
	}

	.column-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100px;
		color: var(--slate);
		font-size: 0.875rem;
	}

	/* Patient Cards */
	.patient-card {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-md);
		padding: var(--space-md);
		text-decoration: none;
		color: inherit;
		transition: all var(--transition-fast);
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.patient-card:hover {
		border-color: var(--scrubs);
		box-shadow: var(--shadow-md);
		transform: translateY(-1px);
	}

	.patient-card.active {
		border-left: 3px solid var(--scrubs);
		background: linear-gradient(to right, var(--scrubs-soft), var(--white));
	}

	.patient-card.recovery {
		border-left: 3px solid var(--stable);
	}

	.patient-card.warning {
		border-left: 3px solid var(--caution);
		background: var(--caution-soft);
	}

	.patient-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-sm);
	}

	.patient-repo {
		font-weight: 600;
		font-size: 0.8125rem;
		color: var(--charcoal);
	}

	.patient-status {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		padding: 2px 6px;
		border-radius: var(--radius-sm);
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.status-detected, .status-triaging {
		background: var(--vitals-soft);
		color: #1D4ED8;
	}

	.status-fixing, .status-pending_fix {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.status-warning {
		background: var(--caution);
		color: white;
	}

	.pulse-dot {
		width: 6px;
		height: 6px;
		background: var(--scrubs);
		border-radius: 50%;
		animation: pulse 1.5s infinite;
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.5; transform: scale(1.3); }
	}

	.patient-pr {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--stable-dark);
		background: var(--stable-soft);
		padding: 2px 6px;
		border-radius: var(--radius-sm);
	}

	.patient-message {
		font-size: 0.8125rem;
		color: var(--charcoal);
		line-height: 1.4;
		overflow: hidden;
		text-overflow: ellipsis;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
	}

	.patient-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-sm);
		font-size: 0.75rem;
		color: var(--slate);
	}

	.patient-samples {
		color: var(--graphite);
	}

	.patient-confidence {
		color: var(--graphite);
	}

	.patient-confidence.high {
		color: var(--scrubs-dark);
		font-weight: 600;
	}

	.patient-reason {
		font-size: 0.6875rem;
		color: var(--graphite);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* Discharged Section */
	.discharged-section {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-lg);
	}

	.discharged-section h3 {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--graphite);
		margin-bottom: var(--space-md);
	}

	.discharged-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.discharged-item {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		padding: var(--space-sm) 0;
		border-bottom: 1px solid var(--paper);
		font-size: 0.8125rem;
	}

	.discharged-item:last-child {
		border-bottom: none;
	}

	.discharged-repo {
		font-weight: 500;
		color: var(--charcoal);
		min-width: 100px;
	}

	.discharged-message {
		flex: 1;
		color: var(--graphite);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.discharged-time {
		color: var(--slate);
		font-size: 0.75rem;
	}

	/* Services Section */
	.services-section {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-lg);
	}

	.section-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: var(--space-lg);
	}

	.section-header h2 {
		font-size: 1.125rem;
	}

	/* Empty State */
	.empty-state {
		background: var(--paper);
		border: 2px dashed var(--silver);
		border-radius: var(--radius-xl);
		padding: var(--space-3xl);
		text-align: center;
	}

	.empty-icon {
		width: 64px;
		height: 64px;
		margin: 0 auto var(--space-lg);
		color: var(--slate);
	}

	.empty-icon svg {
		width: 100%;
		height: 100%;
	}

	.empty-state h3 {
		font-family: var(--font-body);
		font-size: 1.125rem;
		font-weight: 600;
		margin-bottom: var(--space-sm);
	}

	.empty-state p {
		color: var(--graphite);
		margin-bottom: var(--space-lg);
	}

	/* Service Grid */
	.service-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
		gap: var(--space-md);
	}

	.service-card {
		background: var(--paper);
		border: 1px solid var(--silver);
		border-radius: var(--radius-md);
		padding: var(--space-md);
		text-decoration: none;
		color: inherit;
		transition: all var(--transition-fast);
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.service-card:hover {
		border-color: var(--scrubs);
		background: var(--white);
	}

	.service-card.critical {
		border-color: var(--critical);
		background: linear-gradient(to bottom, var(--critical-soft), var(--paper));
	}

	.service-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-sm);
	}

	.service-name {
		font-size: 0.9375rem;
	}

	.repo-owner {
		color: var(--graphite);
	}

	.repo-name {
		font-weight: 600;
		color: var(--obsidian);
	}

	.service-stats {
		display: flex;
		gap: var(--space-lg);
	}

	.stat {
		display: flex;
		flex-direction: column;
	}

	.stat-value {
		font-family: var(--font-display);
		font-size: 1.25rem;
		color: var(--obsidian);
	}

	.stat-label {
		font-size: 0.6875rem;
		color: var(--graphite);
		text-transform: uppercase;
	}

	.service-controls {
		display: flex;
		gap: var(--space-sm);
		margin-top: var(--space-sm);
		padding-top: var(--space-sm);
		border-top: 1px solid var(--silver);
	}

	.control-btn {
		width: 28px;
		height: 28px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-sm);
		background: var(--white);
		color: var(--slate);
		border: 1px solid var(--silver);
		transition: all var(--transition-fast);
	}

	.control-btn:hover {
		border-color: var(--graphite);
		color: var(--charcoal);
	}

	.control-btn.active {
		background: var(--scrubs-soft);
		border-color: var(--scrubs);
		color: var(--scrubs-dark);
	}

	.control-btn svg {
		width: 14px;
		height: 14px;
	}

	/* Badge variants */
	.badge-healthy {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.badge-observation {
		background: var(--vitals-soft);
		color: #1D4ED8;
	}

	.badge-surgery {
		background: var(--alert-soft);
		color: #B45309;
	}

	.badge-critical {
		background: var(--critical-soft);
		color: #B91C1C;
	}

	/* Modal */
	.modal-overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.4);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}

	.modal {
		background: var(--white);
		border-radius: var(--radius-xl);
		width: 100%;
		max-width: 480px;
		box-shadow: var(--shadow-xl);
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-lg);
		border-bottom: 1px solid var(--silver);
	}

	.modal-header h3 {
		font-family: var(--font-body);
		font-size: 1.125rem;
		font-weight: 600;
	}

	.modal-close {
		width: 32px;
		height: 32px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-md);
		background: transparent;
		color: var(--graphite);
		border: none;
		cursor: pointer;
	}

	.modal-close:hover {
		background: var(--paper);
	}

	.modal-close svg {
		width: 20px;
		height: 20px;
	}

	.modal-body {
		padding: var(--space-lg);
	}

	.modal-footer {
		display: flex;
		justify-content: flex-end;
		gap: var(--space-sm);
		padding: var(--space-lg);
		border-top: 1px solid var(--silver);
	}

	/* Form Fields */
	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.field-label {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--charcoal);
	}

	.field-input {
		width: 100%;
	}

	.field-hint {
		font-size: 0.75rem;
		color: var(--slate);
	}

	@media (max-width: 768px) {
		.hud {
			grid-template-columns: repeat(2, 1fr);
		}

		.kanban-board {
			grid-template-columns: 1fr;
		}

		.service-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
