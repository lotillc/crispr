<script lang="ts">
	import { toasts, type Toast } from '$lib/stores/toast';
	import { fly } from 'svelte/transition';

	let toastList = $state<Toast[]>([]);

	$effect(() => {
		const unsubscribe = toasts.subscribe((value) => {
			toastList = value;
		});
		return unsubscribe;
	});

	function getIcon(type: Toast['type']): string {
		switch (type) {
			case 'success':
				return 'M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z';
			case 'error':
				return 'M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z';
			case 'warning':
				return 'M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z';
			case 'info':
			default:
				return 'M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z';
		}
	}
</script>

<div class="toast-container" aria-live="polite" aria-label="Notifications">
	{#each toastList as toast (toast.id)}
		<div
			class="toast toast-{toast.type}"
			role="alert"
			transition:fly={{ x: 300, duration: 300 }}
		>
			<div class="toast-icon">
				<svg viewBox="0 0 20 20" fill="currentColor">
					<path fill-rule="evenodd" d={getIcon(toast.type)} clip-rule="evenodd"/>
				</svg>
			</div>
			<p class="toast-message">{toast.message}</p>
			<button
				class="toast-dismiss"
				onclick={() => toasts.dismissToast(toast.id)}
				aria-label="Dismiss notification"
			>
				<svg viewBox="0 0 20 20" fill="currentColor">
					<path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd"/>
				</svg>
			</button>
		</div>
	{/each}
</div>

<style>
	.toast-container {
		position: fixed;
		top: var(--space-lg);
		right: var(--space-lg);
		z-index: 9999;
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		max-width: 400px;
		pointer-events: none;
	}

	.toast {
		display: flex;
		align-items: flex-start;
		gap: var(--space-sm);
		padding: var(--space-md);
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		box-shadow: var(--shadow-lg);
		pointer-events: auto;
		animation: slideIn 0.3s ease-out;
	}

	@keyframes slideIn {
		from {
			transform: translateX(100%);
			opacity: 0;
		}
		to {
			transform: translateX(0);
			opacity: 1;
		}
	}

	/* Toast type variants */
	.toast-success {
		border-left: 4px solid var(--scrubs);
		background: linear-gradient(to right, var(--scrubs-soft), var(--white) 20%);
	}

	.toast-success .toast-icon {
		color: var(--scrubs);
	}

	.toast-error {
		border-left: 4px solid var(--critical);
		background: linear-gradient(to right, var(--critical-soft), var(--white) 20%);
	}

	.toast-error .toast-icon {
		color: var(--critical);
	}

	.toast-warning {
		border-left: 4px solid var(--alert);
		background: linear-gradient(to right, var(--alert-soft), var(--white) 20%);
	}

	.toast-warning .toast-icon {
		color: var(--alert);
	}

	.toast-info {
		border-left: 4px solid var(--vitals);
		background: linear-gradient(to right, var(--vitals-soft), var(--white) 20%);
	}

	.toast-info .toast-icon {
		color: var(--vitals);
	}

	.toast-icon {
		flex-shrink: 0;
		width: 20px;
		height: 20px;
	}

	.toast-icon svg {
		width: 100%;
		height: 100%;
	}

	.toast-message {
		flex: 1;
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--charcoal);
		line-height: 1.4;
		margin: 0;
	}

	.toast-dismiss {
		flex-shrink: 0;
		width: 20px;
		height: 20px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: none;
		color: var(--slate);
		cursor: pointer;
		padding: 0;
		border-radius: var(--radius-sm);
		transition: all var(--transition-fast);
	}

	.toast-dismiss:hover {
		background: var(--paper);
		color: var(--charcoal);
	}

	.toast-dismiss svg {
		width: 14px;
		height: 14px;
	}

	/* Mobile adjustments */
	@media (max-width: 480px) {
		.toast-container {
			top: var(--space-md);
			right: var(--space-md);
			left: var(--space-md);
			max-width: none;
		}

		.toast {
			min-height: 44px;
		}
	}
</style>
