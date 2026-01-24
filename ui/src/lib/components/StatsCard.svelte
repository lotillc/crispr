<script lang="ts">
	/**
	 * StatsCard Component
	 * A reusable card for displaying dashboard statistics with optional
	 * icon, trend indicator, and loading state.
	 */

	import type { Snippet } from 'svelte';
	import Skeleton from './Skeleton.svelte';

	type TrendDirection = 'up' | 'down' | 'neutral';

	interface Props {
		/** Label describing the statistic */
		label: string;
		/** The value to display */
		value: string | number;
		/** Whether the card is in a loading state */
		loading?: boolean;
		/** Optional trend data */
		trend?: {
			direction: TrendDirection;
			value: string;
		};
		/** Whether this card should be highlighted */
		highlight?: boolean;
		/** Additional CSS classes */
		class?: string;
		/** Optional icon slot */
		icon?: Snippet;
	}

	let {
		label,
		value,
		loading = false,
		trend,
		highlight = false,
		class: className = '',
		icon
	}: Props = $props();
</script>

<div class="stats-card {className}" class:highlight class:loading>
	{#if icon}
		<div class="stats-icon">
			{@render icon()}
		</div>
	{/if}

	<div class="stats-content">
		{#if loading}
			<Skeleton variant="text" width="60%" height="2rem" />
			<Skeleton variant="text" width="80%" height="0.75rem" class="stats-label-skeleton" />
		{:else}
			<span class="stats-value">{value}</span>
			<span class="stats-label">{label}</span>
		{/if}
	</div>

	{#if trend && !loading}
		<div class="stats-trend trend-{trend.direction}">
			{#if trend.direction === 'up'}
				<svg viewBox="0 0 20 20" fill="currentColor" class="trend-icon">
					<path
						fill-rule="evenodd"
						d="M5.293 9.707a1 1 0 010-1.414l4-4a1 1 0 011.414 0l4 4a1 1 0 01-1.414 1.414L11 7.414V15a1 1 0 11-2 0V7.414L6.707 9.707a1 1 0 01-1.414 0z"
						clip-rule="evenodd"
					/>
				</svg>
			{:else if trend.direction === 'down'}
				<svg viewBox="0 0 20 20" fill="currentColor" class="trend-icon">
					<path
						fill-rule="evenodd"
						d="M14.707 10.293a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 111.414-1.414L9 12.586V5a1 1 0 012 0v7.586l2.293-2.293a1 1 0 011.414 0z"
						clip-rule="evenodd"
					/>
				</svg>
			{:else}
				<svg viewBox="0 0 20 20" fill="currentColor" class="trend-icon">
					<path
						fill-rule="evenodd"
						d="M5 10a1 1 0 011-1h8a1 1 0 110 2H6a1 1 0 01-1-1z"
						clip-rule="evenodd"
					/>
				</svg>
			{/if}
			<span class="trend-value">{trend.value}</span>
		</div>
	{/if}
</div>

<style>
	.stats-card {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-lg);
		transition: all var(--transition-fast);
	}

	.stats-card:hover:not(.loading) {
		box-shadow: var(--shadow-md);
	}

	.stats-card.highlight {
		background: var(--scrubs-soft);
		border-color: var(--scrubs);
	}

	.stats-card.loading {
		cursor: default;
	}

	.stats-icon {
		width: 40px;
		height: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--paper);
		border-radius: var(--radius-md);
		color: var(--graphite);
		margin-bottom: var(--space-xs);
	}

	.stats-card.highlight .stats-icon {
		background: var(--white);
		color: var(--scrubs-dark);
	}

	.stats-icon :global(svg) {
		width: 20px;
		height: 20px;
	}

	.stats-content {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.stats-value {
		display: block;
		font-family: var(--font-display);
		font-size: 2rem;
		color: var(--obsidian);
		line-height: 1;
	}

	.stats-card.highlight .stats-value {
		color: var(--scrubs-dark);
	}

	.stats-label {
		display: block;
		font-size: 0.75rem;
		color: var(--graphite);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.stats-card.highlight .stats-label {
		color: var(--scrubs-dark);
		opacity: 0.8;
	}

	:global(.stats-label-skeleton) {
		margin-top: var(--space-xs);
	}

	.stats-trend {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		margin-top: var(--space-xs);
		padding-top: var(--space-sm);
		border-top: 1px solid var(--paper);
	}

	.stats-card.highlight .stats-trend {
		border-color: rgba(255, 255, 255, 0.3);
	}

	.trend-icon {
		width: 14px;
		height: 14px;
	}

	.trend-value {
		font-size: 0.75rem;
		font-weight: 500;
	}

	/* Trend colors */
	.trend-up {
		color: var(--scrubs-dark);
	}

	.trend-down {
		color: var(--critical);
	}

	.trend-neutral {
		color: var(--graphite);
	}

	/* Responsive adjustments */
	@media (max-width: 768px) {
		.stats-card {
			padding: var(--space-md);
		}

		.stats-value {
			font-size: 1.75rem;
		}

		.stats-label {
			font-size: 0.6875rem;
		}
	}

	@media (max-width: 480px) {
		.stats-value {
			font-size: 1.5rem;
		}

		.stats-icon {
			width: 32px;
			height: 32px;
		}

		.stats-icon :global(svg) {
			width: 16px;
			height: 16px;
		}
	}
</style>
