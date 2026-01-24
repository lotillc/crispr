<script lang="ts">
	/**
	 * Skeleton Component
	 * An animated loading placeholder with medical pulse animation.
	 * Used to indicate content loading states throughout the UI.
	 */

	type SkeletonVariant = 'text' | 'circle' | 'rect';

	interface Props {
		/** Width of the skeleton (CSS value) */
		width?: string;
		/** Height of the skeleton (CSS value) */
		height?: string;
		/** Shape variant of the skeleton */
		variant?: SkeletonVariant;
		/** Additional CSS classes */
		class?: string;
	}

	let {
		width = '100%',
		height = '1rem',
		variant = 'text',
		class: className = ''
	}: Props = $props();

	// Compute dimensions based on variant
	let computedWidth = $derived(variant === 'circle' ? height : width);
	let computedHeight = $derived(height);
	let borderRadius = $derived(
		variant === 'circle' ? '50%' : variant === 'text' ? 'var(--radius-sm)' : 'var(--radius-md)'
	);
</script>

<div
	class="skeleton skeleton-{variant} {className}"
	style:width={computedWidth}
	style:height={computedHeight}
	style:border-radius={borderRadius}
	role="status"
	aria-label="Loading..."
>
	<span class="sr-only">Loading...</span>
</div>

<style>
	.skeleton {
		position: relative;
		background: var(--paper);
		overflow: hidden;
	}

	/* Medical pulse animation - like a heartbeat monitor */
	.skeleton::after {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: linear-gradient(
			90deg,
			transparent 0%,
			var(--silver) 20%,
			var(--ash) 50%,
			var(--silver) 80%,
			transparent 100%
		);
		animation: pulse-sweep 1.5s ease-in-out infinite;
		transform: translateX(-100%);
	}

	@keyframes pulse-sweep {
		0% {
			transform: translateX(-100%);
		}
		50% {
			transform: translateX(0%);
		}
		100% {
			transform: translateX(100%);
		}
	}

	/* Text variant - slightly shorter height and rounded */
	.skeleton-text {
		min-height: 0.75rem;
	}

	/* Circle variant - perfect circle */
	.skeleton-circle {
		flex-shrink: 0;
	}

	/* Rect variant - standard rectangle with medium radius */
	.skeleton-rect {
		min-height: 2rem;
	}

	/* Screen reader only text */
	.sr-only {
		position: absolute;
		width: 1px;
		height: 1px;
		padding: 0;
		margin: -1px;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}

	/* Dark mode adjustments */
	:global(:root.dark) .skeleton {
		background: var(--silver);
	}

	:global(:root.dark) .skeleton::after {
		background: linear-gradient(
			90deg,
			transparent 0%,
			var(--ash) 20%,
			var(--slate) 50%,
			var(--ash) 80%,
			transparent 100%
		);
	}
</style>
