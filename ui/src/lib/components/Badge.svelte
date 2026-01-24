<script lang="ts">
	/**
	 * Badge Component
	 * A medical-themed status badge used for displaying statuses, labels, and tags
	 * throughout the CRISPR surgery center UI.
	 */

	import type { Snippet } from 'svelte';

	type BadgeVariant =
		| 'success'
		| 'warning'
		| 'error'
		| 'info'
		| 'neutral'
		| 'surgery'
		| 'observation'
		| 'healthy'
		| 'critical'
		| 'discharged';

	type BadgeSize = 'sm' | 'md';

	interface Props {
		/** Visual variant of the badge */
		variant?: BadgeVariant;
		/** Size of the badge */
		size?: BadgeSize;
		/** Show a pulsing dot indicator (for active states) */
		pulse?: boolean;
		/** Additional CSS classes */
		class?: string;
		/** Badge content */
		children: Snippet;
	}

	let {
		variant = 'neutral',
		size = 'md',
		pulse = false,
		class: className = '',
		children
	}: Props = $props();
</script>

<span class="badge badge-{variant} badge-{size} {className}" class:has-pulse={pulse}>
	{#if pulse}
		<span class="pulse-dot"></span>
	{/if}
	{@render children()}
</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		border-radius: var(--radius-full);
		font-weight: 500;
		letter-spacing: 0.02em;
		white-space: nowrap;
	}

	/* Sizes */
	.badge-sm {
		padding: 2px var(--space-sm);
		font-size: 0.6875rem;
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}

	.badge-md {
		padding: var(--space-xs) var(--space-sm);
		font-size: 0.75rem;
	}

	/* Variants using medical-themed CSS variables */

	/* Success - Surgical green (scrubs) */
	.badge-success,
	.badge-healthy {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	/* Warning - Alert amber */
	.badge-warning,
	.badge-surgery {
		background: var(--alert-soft);
		color: #b45309;
	}

	/* Error - Critical red */
	.badge-error,
	.badge-critical {
		background: var(--critical-soft);
		color: #b91c1c;
	}

	/* Info - Vitals blue */
	.badge-info,
	.badge-observation {
		background: var(--vitals-soft);
		color: #1d4ed8;
	}

	/* Neutral - Gray */
	.badge-neutral {
		background: var(--paper);
		color: var(--graphite);
	}

	/* Discharged - Purple (stable) */
	.badge-discharged {
		background: var(--stable-soft);
		color: #6d28d9;
	}

	/* Pulse dot animation for active states */
	.pulse-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: currentColor;
		animation: pulse 1.5s infinite ease-in-out;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
			transform: scale(1);
		}
		50% {
			opacity: 0.5;
			transform: scale(1.3);
		}
	}

	/* Ensure adequate space when pulse dot is present */
	.has-pulse {
		gap: 4px;
	}
</style>
