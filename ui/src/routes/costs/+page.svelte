<script lang="ts">
	import { onMount } from 'svelte';
	import { usage, projects } from '$lib/stores';
	import * as api from '$lib/api';
	import Breadcrumb from '$lib/components/Breadcrumb.svelte';

	const breadcrumbItems = [
		{ label: 'Surgery Board', href: '/' },
		{ label: 'Costs', href: '/costs' }
	];

	let usageDetails = $state<Awaited<ReturnType<typeof api.getUsageDetails>>>([]);
	let projectStats = $state<Map<string, api.ProjectStats>>(new Map());
	let timeRange = $state<'7d' | '30d' | '90d'>('30d');

	onMount(async () => {
		await loadData();
	});

	async function loadData() {
		const endDate = new Date();
		const startDate = new Date();

		if (timeRange === '7d') startDate.setDate(endDate.getDate() - 7);
		else if (timeRange === '30d') startDate.setDate(endDate.getDate() - 30);
		else startDate.setDate(endDate.getDate() - 90);

		await usage.load({
			start_date: startDate.toISOString().split('T')[0],
			end_date: endDate.toISOString().split('T')[0],
		});

		try {
			usageDetails = await api.getUsageDetails({
				start_date: startDate.toISOString().split('T')[0],
				end_date: endDate.toISOString().split('T')[0],
			});
		} catch (e) {
			console.error('Failed to load usage details:', e);
		}

		// Load project stats
		for (const project of $projects) {
			try {
				const stats = await api.getProjectStats(project.id);
				projectStats.set(project.id, stats);
			} catch (e) {
				console.error('Failed to load project stats:', e);
			}
		}
		projectStats = projectStats; // trigger reactivity
	}

	$effect(() => {
		loadData();
	});

	// Compute cost by service
	let costByService = $derived(() => {
		const costs: { repo: string; cost: number; incidents: number }[] = [];
		for (const project of $projects) {
			const stats = projectStats.get(project.id);
			costs.push({
				repo: project.repo,
				cost: stats?.total_cost ?? 0,
				incidents: stats?.incident_count ?? 0,
			});
		}
		return costs.sort((a, b) => b.cost - a.cost);
	});

	// Compute cost by purpose (reader vs writer vs internist)
	let costByPurpose = $derived(() => {
		const byPurpose = $usage?.by_purpose ?? {};
		return Object.entries(byPurpose).map(([purpose, data]) => ({
			purpose: purpose === 'triage' ? 'Log Analysis (Reader)' :
				purpose === 'fix' ? 'Fix Generation (Writer)' :
					purpose === 'context' ? 'Context Gathering' :
						purpose === 'internist' ? 'Pattern Analysis (Internist)' : purpose,
			cost: data.cost,
			requests: data.requests,
		})).sort((a, b) => b.cost - a.cost);
	});

	// Compute cost by provider
	let costByProvider = $derived(() => {
		const byProvider = $usage?.by_provider ?? {};
		return Object.entries(byProvider).map(([provider, data]) => ({
			provider: provider.charAt(0).toUpperCase() + provider.slice(1),
			cost: data.cost,
			requests: data.requests,
			inputTokens: data.input_tokens,
			outputTokens: data.output_tokens,
		})).sort((a, b) => b.cost - a.cost);
	});

	function formatCost(cost: number): string {
		return `$${cost.toFixed(2)}`;
	}

	function formatNumber(num: number): string {
		if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
		if (num >= 1000) return `${(num / 1000).toFixed(1)}K`;
		return num.toString();
	}
</script>

<svelte:head>
	<title>Administration - CRISPR</title>
</svelte:head>

<div class="costs-page">
	<!-- Breadcrumb Navigation -->
	<Breadcrumb items={breadcrumbItems} />

	<header class="page-header">
		<div class="header-content">
			<h1>Administration</h1>
			<p class="page-subtitle">Surgery costs and resource utilization</p>
		</div>

		<div class="time-selector" role="group" aria-label="Time range selection">
			<button
				class="time-btn"
				class:active={timeRange === '7d'}
				onclick={() => timeRange = '7d'}
				aria-pressed={timeRange === '7d'}
			>7 days</button>
			<button
				class="time-btn"
				class:active={timeRange === '30d'}
				onclick={() => timeRange = '30d'}
				aria-pressed={timeRange === '30d'}
			>30 days</button>
			<button
				class="time-btn"
				class:active={timeRange === '90d'}
				onclick={() => timeRange = '90d'}
				aria-pressed={timeRange === '90d'}
			>90 days</button>
		</div>
	</header>

	<!-- Summary Cards -->
	<div class="summary-grid">
		<div class="summary-card">
			<span class="summary-label">Total Cost</span>
			<span class="summary-value">{formatCost($usage?.total_cost ?? 0)}</span>
		</div>
		<div class="summary-card">
			<span class="summary-label">API Requests</span>
			<span class="summary-value">{formatNumber($usage?.total_requests ?? 0)}</span>
		</div>
		<div class="summary-card">
			<span class="summary-label">Input Tokens</span>
			<span class="summary-value">{formatNumber($usage?.total_input_tokens ?? 0)}</span>
		</div>
		<div class="summary-card">
			<span class="summary-label">Output Tokens</span>
			<span class="summary-value">{formatNumber($usage?.total_output_tokens ?? 0)}</span>
		</div>
	</div>

	<div class="charts-grid">
		<!-- Cost by Service -->
		<section class="chart-section">
			<h2>Cost by Service</h2>
			<p class="section-desc">Which services are consuming the most resources</p>

			{#if costByService().length === 0}
				<div class="empty-chart">
					<p>No usage data available</p>
				</div>
			{:else}
				<div class="bar-chart">
					{#each costByService() as item, i}
						{@const maxCost = Math.max(...costByService().map(s => s.cost), 1)}
						<div class="bar-row" style="animation-delay: {i * 50}ms">
							<div class="bar-label">
								<span class="bar-name">{item.repo}</span>
								<span class="bar-meta">{item.incidents} incidents</span>
							</div>
							<div class="bar-container">
								<div
									class="bar-fill"
									style="width: {(item.cost / maxCost) * 100}%"
								></div>
							</div>
							<span class="bar-value">{formatCost(item.cost)}</span>
						</div>
					{/each}
				</div>
			{/if}
		</section>

		<!-- Cost by Purpose -->
		<section class="chart-section">
			<h2>Cost by Purpose</h2>
			<p class="section-desc">Breakdown between log analysis and fix generation</p>

			{#if costByPurpose().length === 0}
				<div class="empty-chart">
					<p>No usage data available</p>
				</div>
			{:else}
				<div class="purpose-breakdown">
					{#each costByPurpose() as item}
						{@const totalCost = costByPurpose().reduce((sum, p) => sum + p.cost, 0) || 1}
						<div class="purpose-item">
							<div class="purpose-header">
								<span class="purpose-name">{item.purpose}</span>
								<span class="purpose-cost">{formatCost(item.cost)}</span>
							</div>
							<div class="purpose-bar">
								<div
									class="purpose-fill"
									style="width: {(item.cost / totalCost) * 100}%"
								></div>
							</div>
							<span class="purpose-requests">{formatNumber(item.requests)} requests</span>
						</div>
					{/each}
				</div>
			{/if}
		</section>

		<!-- Cost by Provider -->
		<section class="chart-section full-width">
			<h2>Cost by Provider</h2>
			<p class="section-desc">Usage across different LLM providers</p>

			{#if costByProvider().length === 0}
				<div class="empty-chart">
					<p>No provider data available</p>
				</div>
			{:else}
				<div class="provider-grid">
					{#each costByProvider() as provider}
						<div class="provider-card">
							<div class="provider-header">
								<span class="provider-name">{provider.provider}</span>
								<span class="provider-cost">{formatCost(provider.cost)}</span>
							</div>
							<div class="provider-stats">
								<div class="provider-stat">
									<span class="stat-value">{formatNumber(provider.requests)}</span>
									<span class="stat-label">Requests</span>
								</div>
								<div class="provider-stat">
									<span class="stat-value">{formatNumber(provider.inputTokens)}</span>
									<span class="stat-label">Input Tokens</span>
								</div>
								<div class="provider-stat">
									<span class="stat-value">{formatNumber(provider.outputTokens)}</span>
									<span class="stat-label">Output Tokens</span>
								</div>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</section>
	</div>

	<!-- Usage Details Table -->
	{#if usageDetails.length > 0}
		<section class="details-section" aria-label="Usage details">
			<h2 id="usage-details-heading">Usage Details</h2>
			<div class="table-wrapper">
				<table class="details-table" aria-labelledby="usage-details-heading">
					<thead>
						<tr>
							<th scope="col">Date</th>
							<th scope="col">Provider</th>
							<th scope="col">Model</th>
							<th scope="col">Purpose</th>
							<th scope="col">Requests</th>
							<th scope="col">Input Tokens</th>
							<th scope="col">Output Tokens</th>
							<th scope="col">Cost</th>
						</tr>
					</thead>
					<tbody>
						{#each usageDetails.slice(0, 50) as row}
							<tr>
								<td>{row.date}</td>
								<td>{row.provider}</td>
								<td class="mono">{row.model}</td>
								<td>{row.purpose}</td>
								<td>{formatNumber(row.requests)}</td>
								<td>{formatNumber(row.input_tokens)}</td>
								<td>{formatNumber(row.output_tokens)}</td>
								<td class="cost-cell">{formatCost(row.cost)}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		</section>
	{/if}
</div>

<style>
	.costs-page {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	.page-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
	}

	.page-header h1 {
		font-size: 1.75rem;
		margin-bottom: var(--space-xs);
	}

	.page-subtitle {
		color: var(--graphite);
	}

	.time-selector {
		display: flex;
		background: var(--paper);
		border-radius: var(--radius-md);
		padding: 4px;
	}

	.time-btn {
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--graphite);
		font-size: 0.875rem;
	}

	.time-btn:hover {
		color: var(--charcoal);
	}

	.time-btn.active {
		background: var(--white);
		color: var(--charcoal);
		box-shadow: var(--shadow-sm);
	}

	/* Summary Cards */
	.summary-grid {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: var(--space-md);
	}

	.summary-card {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-lg);
	}

	.summary-label {
		display: block;
		font-size: 0.75rem;
		color: var(--graphite);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		margin-bottom: var(--space-sm);
	}

	.summary-value {
		font-family: var(--font-display);
		font-size: 2rem;
		color: var(--obsidian);
	}

	/* Charts Grid */
	.charts-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--space-lg);
	}

	.chart-section {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-lg);
	}

	.chart-section.full-width {
		grid-column: 1 / -1;
	}

	.chart-section h2 {
		font-family: var(--font-body);
		font-size: 1rem;
		font-weight: 600;
		margin-bottom: var(--space-xs);
	}

	.section-desc {
		font-size: 0.8125rem;
		color: var(--graphite);
		margin-bottom: var(--space-lg);
	}

	.empty-chart {
		padding: var(--space-xl);
		text-align: center;
		color: var(--slate);
	}

	/* Bar Chart */
	.bar-chart {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.bar-row {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		animation: slideUp var(--transition-base) ease-out both;
	}

	.bar-label {
		width: 180px;
		flex-shrink: 0;
	}

	.bar-name {
		display: block;
		font-size: 0.875rem;
		color: var(--charcoal);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.bar-meta {
		font-size: 0.75rem;
		color: var(--slate);
	}

	.bar-container {
		flex: 1;
		height: 24px;
		background: var(--paper);
		border-radius: var(--radius-sm);
		overflow: hidden;
	}

	.bar-fill {
		height: 100%;
		background: linear-gradient(90deg, var(--scrubs-soft), var(--scrubs));
		border-radius: var(--radius-sm);
		transition: width var(--transition-slow);
	}

	.bar-value {
		width: 70px;
		text-align: right;
		font-weight: 500;
		color: var(--charcoal);
	}

	/* Purpose Breakdown */
	.purpose-breakdown {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	.purpose-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.purpose-header {
		display: flex;
		justify-content: space-between;
	}

	.purpose-name {
		font-size: 0.875rem;
		color: var(--charcoal);
	}

	.purpose-cost {
		font-weight: 600;
		color: var(--obsidian);
	}

	.purpose-bar {
		height: 8px;
		background: var(--paper);
		border-radius: var(--radius-full);
		overflow: hidden;
	}

	.purpose-fill {
		height: 100%;
		background: var(--vitals);
		border-radius: var(--radius-full);
		transition: width var(--transition-slow);
	}

	.purpose-requests {
		font-size: 0.75rem;
		color: var(--slate);
	}

	/* Provider Grid */
	.provider-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
		gap: var(--space-md);
	}

	.provider-card {
		background: var(--paper);
		border-radius: var(--radius-md);
		padding: var(--space-lg);
	}

	.provider-header {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		margin-bottom: var(--space-md);
	}

	.provider-name {
		font-weight: 600;
		color: var(--charcoal);
	}

	.provider-cost {
		font-family: var(--font-display);
		font-size: 1.25rem;
		color: var(--obsidian);
	}

	.provider-stats {
		display: flex;
		gap: var(--space-lg);
	}

	.provider-stat {
		display: flex;
		flex-direction: column;
	}

	.provider-stat .stat-value {
		font-size: 1rem;
		font-weight: 500;
		color: var(--charcoal);
	}

	.provider-stat .stat-label {
		font-size: 0.75rem;
		color: var(--slate);
	}

	/* Details Table */
	.details-section {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-lg);
	}

	.details-section h2 {
		font-family: var(--font-body);
		font-size: 1rem;
		font-weight: 600;
		margin-bottom: var(--space-lg);
	}

	.table-wrapper {
		overflow-x: auto;
	}

	.details-table {
		min-width: 800px;
	}

	.details-table th {
		font-size: 0.6875rem;
	}

	.details-table td {
		font-size: 0.8125rem;
	}

	.cost-cell {
		font-weight: 500;
		color: var(--obsidian);
	}

	@media (max-width: 1024px) {
		.summary-grid {
			grid-template-columns: repeat(2, 1fr);
		}

		.charts-grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 640px) {
		.page-header {
			flex-direction: column;
			gap: var(--space-md);
		}

		.summary-grid {
			grid-template-columns: 1fr;
		}

		.bar-label {
			width: 100px;
		}
	}
</style>
