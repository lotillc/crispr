<script lang="ts">
	interface BreadcrumbItem {
		label: string;
		href: string;
	}

	let { items = [] }: { items: BreadcrumbItem[] } = $props();
</script>

<nav class="breadcrumb" aria-label="Breadcrumb">
	<ol class="breadcrumb-list">
		{#each items as item, index}
			<li class="breadcrumb-item">
				{#if index < items.length - 1}
					<a href={item.href} class="breadcrumb-link">
						{item.label}
					</a>
					<svg class="breadcrumb-separator" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
						<path fill-rule="evenodd" d="M7.21 14.77a.75.75 0 01.02-1.06L11.168 10 7.23 6.29a.75.75 0 111.04-1.08l4.5 4.25a.75.75 0 010 1.08l-4.5 4.25a.75.75 0 01-1.06-.02z" clip-rule="evenodd"/>
					</svg>
				{:else}
					<span class="breadcrumb-current" aria-current="page">
						{item.label}
					</span>
				{/if}
			</li>
		{/each}
	</ol>
</nav>

<style>
	.breadcrumb {
		padding: var(--space-sm) 0;
		margin-bottom: var(--space-md);
	}

	.breadcrumb-list {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: var(--space-xs);
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.breadcrumb-item {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
	}

	.breadcrumb-link {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-sm);
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--graphite);
		text-decoration: none;
		transition: all var(--transition-fast);
	}

	.breadcrumb-link:hover {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.breadcrumb-separator {
		width: 16px;
		height: 16px;
		color: var(--slate);
		flex-shrink: 0;
	}

	.breadcrumb-current {
		display: inline-flex;
		align-items: center;
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-sm);
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--charcoal);
		background: var(--paper);
	}

	@media (max-width: 640px) {
		.breadcrumb {
			padding: var(--space-xs) 0;
		}

		.breadcrumb-link,
		.breadcrumb-current {
			font-size: 0.75rem;
			padding: var(--space-xs);
		}

		.breadcrumb-separator {
			width: 14px;
			height: 14px;
		}
	}
</style>
