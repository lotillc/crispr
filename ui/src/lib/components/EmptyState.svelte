<script lang="ts">
	/**
	 * EmptyState Component
	 * A medical-themed empty state component with illustration placeholder
	 * and optional action slot. Used when there's no data to display.
	 */

	import type { Snippet } from 'svelte';

	interface Props {
		/** Title for the empty state */
		title: string;
		/** Description text explaining the empty state */
		description: string;
		/** Additional CSS classes */
		class?: string;
		/** Optional custom icon slot */
		icon?: Snippet;
		/** Optional action slot (buttons, links, etc.) */
		action?: Snippet;
	}

	let { title, description, class: className = '', icon, action }: Props = $props();
</script>

<div class="empty-state {className}">
	<div class="empty-icon">
		{#if icon}
			{@render icon()}
		{:else}
			<!-- Default medical cross/plus icon -->
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					d="M12 9v6m3-3H9m12 0a9 9 0 11-18 0 9 9 0 0118 0z"
				/>
			</svg>
		{/if}
	</div>

	<h3 class="empty-title">{title}</h3>
	<p class="empty-description">{description}</p>

	{#if action}
		<div class="empty-action">
			{@render action()}
		</div>
	{/if}

	<!-- Decorative medical pattern -->
	<div class="empty-decoration" aria-hidden="true">
		<svg viewBox="0 0 100 100" class="decoration-pattern">
			<!-- EKG/heartbeat line pattern -->
			<polyline
				points="0,50 20,50 25,30 30,70 35,50 45,50 50,20 55,80 60,50 100,50"
				fill="none"
				stroke="currentColor"
				stroke-width="1"
			/>
		</svg>
	</div>
</div>

<style>
	.empty-state {
		position: relative;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--space-3xl);
		text-align: center;
		background: var(--paper);
		border: 2px dashed var(--silver);
		border-radius: var(--radius-xl);
		overflow: hidden;
	}

	.empty-icon {
		width: 64px;
		height: 64px;
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: var(--space-lg);
		color: var(--scrubs);
		opacity: 0.8;
	}

	.empty-icon :global(svg) {
		width: 100%;
		height: 100%;
	}

	.empty-title {
		font-family: var(--font-body);
		font-size: 1.125rem;
		font-weight: 600;
		color: var(--charcoal);
		margin-bottom: var(--space-sm);
	}

	.empty-description {
		font-size: 0.9375rem;
		color: var(--graphite);
		max-width: 320px;
		line-height: 1.5;
	}

	.empty-action {
		margin-top: var(--space-lg);
	}

	.empty-action :global(.btn) {
		min-width: 160px;
	}

	/* Decorative medical pattern */
	.empty-decoration {
		position: absolute;
		bottom: 0;
		left: 0;
		right: 0;
		height: 40px;
		opacity: 0.1;
		overflow: hidden;
	}

	.decoration-pattern {
		width: 100%;
		height: 100%;
		color: var(--scrubs);
	}

	/* Animation for the EKG line */
	.decoration-pattern polyline {
		stroke-dasharray: 200;
		stroke-dashoffset: 200;
		animation: draw-ekg 3s ease-in-out infinite;
	}

	@keyframes draw-ekg {
		0% {
			stroke-dashoffset: 200;
		}
		50% {
			stroke-dashoffset: 0;
		}
		100% {
			stroke-dashoffset: -200;
		}
	}

	/* Responsive adjustments */
	@media (max-width: 768px) {
		.empty-state {
			padding: var(--space-xl);
		}

		.empty-icon {
			width: 48px;
			height: 48px;
			margin-bottom: var(--space-md);
		}

		.empty-title {
			font-size: 1rem;
		}

		.empty-description {
			font-size: 0.875rem;
		}

		.empty-action :global(.btn) {
			width: 100%;
			min-height: 44px;
		}
	}

	@media (max-width: 480px) {
		.empty-state {
			padding: var(--space-lg);
		}

		.empty-icon {
			width: 40px;
			height: 40px;
		}

		.empty-title {
			font-size: 0.9375rem;
		}

		.empty-description {
			font-size: 0.8125rem;
		}
	}

	/* Dark mode adjustments */
	:global(:root.dark) .empty-state {
		background: var(--paper);
		border-color: var(--ash);
	}

	:global(:root.dark) .empty-decoration {
		opacity: 0.15;
	}
</style>
