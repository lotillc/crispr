<script lang="ts">
	/**
	 * GeneratedTests Component
	 * Displays generated tests for an incident with toggleable PR inclusion
	 * and code preview functionality.
	 */

	import type { GeneratedTest } from '$lib/api';
	import Badge from './Badge.svelte';
	import EmptyState from './EmptyState.svelte';

	interface Props {
		/** Array of generated tests to display */
		tests: GeneratedTest[];
		/** Whether the component is in a loading state */
		loading?: boolean;
		/** Callback when a test's included_in_pr status is toggled */
		onToggleInclude?: (testId: string, included: boolean) => void;
		/** Whether toggling is currently in progress */
		toggling?: boolean;
		/** Additional CSS classes */
		class?: string;
	}

	let {
		tests,
		loading = false,
		onToggleInclude,
		toggling = false,
		class: className = ''
	}: Props = $props();

	// Track which test code blocks are expanded
	let expandedTests = $state<Set<string>>(new Set());

	function toggleExpand(testId: string) {
		if (expandedTests.has(testId)) {
			expandedTests.delete(testId);
			expandedTests = new Set(expandedTests);
		} else {
			expandedTests.add(testId);
			expandedTests = new Set(expandedTests);
		}
	}

	function handleToggleInclude(testId: string, event: Event) {
		const target = event.target as HTMLInputElement;
		onToggleInclude?.(testId, target.checked);
	}

	function getFrameworkBadgeVariant(framework: string): 'info' | 'success' | 'warning' | 'neutral' {
		switch (framework.toLowerCase()) {
			case 'jest':
			case 'vitest':
				return 'info';
			case 'pytest':
				return 'success';
			case 'go test':
			case 'go':
				return 'info';
			case 'rspec':
				return 'warning';
			default:
				return 'neutral';
		}
	}

	function getFileExtension(filePath: string): string {
		const ext = filePath.split('.').pop() || '';
		return ext.toLowerCase();
	}

	function getLanguageClass(filePath: string): string {
		const ext = getFileExtension(filePath);
		switch (ext) {
			case 'ts':
			case 'tsx':
				return 'language-typescript';
			case 'js':
			case 'jsx':
				return 'language-javascript';
			case 'py':
				return 'language-python';
			case 'go':
				return 'language-go';
			case 'rb':
				return 'language-ruby';
			case 'rs':
				return 'language-rust';
			default:
				return 'language-plaintext';
		}
	}
</script>

<section class="generated-tests {className}" aria-label="Generated tests">
	<div class="section-header">
		<h3 id="tests-heading">Generated Tests</h3>
		{#if tests.length > 0}
			<span class="test-count">{tests.length} test{tests.length !== 1 ? 's' : ''}</span>
		{/if}
	</div>

	{#if loading}
		<div class="loading-state">
			<div class="spinner"></div>
			<p>Loading tests...</p>
		</div>
	{:else if tests.length === 0}
		<EmptyState
			title="No Tests Generated"
			description="Tests will be generated when a fix is created for this incident."
		>
			{#snippet icon()}
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<path stroke-linecap="round" stroke-linejoin="round" d="M9.75 3.104v5.714a2.25 2.25 0 01-.659 1.591L5 14.5M9.75 3.104c-.251.023-.501.05-.75.082m.75-.082a24.301 24.301 0 014.5 0m0 0v5.714c0 .597.237 1.17.659 1.591L19.8 15.3M14.25 3.104c.251.023.501.05.75.082M19.8 15.3l-1.57.393A9.065 9.065 0 0112 15a9.065 9.065 0 00-6.23.693L5 14.5m14.8.8l1.402 1.402c1.232 1.232.65 3.318-1.067 3.611A48.309 48.309 0 0112 21c-2.773 0-5.491-.235-8.135-.687-1.718-.293-2.3-2.379-1.067-3.61L5 14.5" />
				</svg>
			{/snippet}
		</EmptyState>
	{:else}
		<div class="tests-list" role="list" aria-labelledby="tests-heading">
			{#each tests as test (test.id)}
				<article class="test-card" role="listitem">
					<div class="test-header">
						<div class="test-info">
							<code class="test-path">{test.file_path}</code>
							<div class="test-meta">
								<Badge variant={getFrameworkBadgeVariant(test.framework)} size="sm">
									{test.framework}
								</Badge>
								{#if test.commit_sha}
									<Badge variant="success" size="sm">
										Committed
									</Badge>
								{/if}
							</div>
						</div>
						<div class="test-actions">
							<label class="include-toggle" class:disabled={toggling || !!test.commit_sha}>
								<input
									type="checkbox"
									checked={test.included_in_pr}
									disabled={toggling || !!test.commit_sha}
									onchange={(e) => handleToggleInclude(test.id, e)}
								/>
								<span class="toggle-label">Include in PR</span>
							</label>
						</div>
					</div>

					{#if test.description}
						<p class="test-description">{test.description}</p>
					{/if}

					<div class="test-code-section">
						<button
							class="expand-toggle"
							onclick={() => toggleExpand(test.id)}
							aria-expanded={expandedTests.has(test.id)}
							aria-controls="code-{test.id}"
						>
							<svg
								class="expand-icon"
								class:expanded={expandedTests.has(test.id)}
								viewBox="0 0 20 20"
								fill="currentColor"
							>
								<path fill-rule="evenodd" d="M5.23 7.21a.75.75 0 011.06.02L10 11.168l3.71-3.938a.75.75 0 111.08 1.04l-4.25 4.5a.75.75 0 01-1.08 0l-4.25-4.5a.75.75 0 01.02-1.06z" clip-rule="evenodd" />
							</svg>
							<span>{expandedTests.has(test.id) ? 'Hide' : 'Show'} code</span>
						</button>

						{#if expandedTests.has(test.id)}
							<div class="code-container" id="code-{test.id}">
								<pre class="test-code {getLanguageClass(test.file_path)}"><code>{test.content}</code></pre>
							</div>
						{/if}
					</div>
				</article>
			{/each}
		</div>
	{/if}
</section>

<style>
	.generated-tests {
		margin-bottom: var(--space-xl);
	}

	.section-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: var(--space-md);
	}

	.section-header h3 {
		font-family: var(--font-body);
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--charcoal);
		margin: 0;
	}

	.test-count {
		background: var(--paper);
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-full);
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--graphite);
	}

	.loading-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--space-xl);
		color: var(--graphite);
	}

	.spinner {
		width: 24px;
		height: 24px;
		border: 2px solid var(--paper);
		border-top-color: var(--scrubs);
		border-radius: 50%;
		animation: spin 1s linear infinite;
		margin-bottom: var(--space-sm);
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	.tests-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.test-card {
		background: var(--paper);
		border-radius: var(--radius-md);
		padding: var(--space-lg);
		transition: box-shadow var(--transition-fast);
	}

	.test-card:hover {
		box-shadow: var(--shadow-sm);
	}

	.test-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: var(--space-md);
		margin-bottom: var(--space-sm);
	}

	.test-info {
		flex: 1;
		min-width: 0;
	}

	.test-path {
		display: block;
		font-family: 'SF Mono', 'Fira Code', monospace;
		font-size: 0.8125rem;
		color: var(--charcoal);
		background: var(--white);
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-sm);
		margin-bottom: var(--space-sm);
		word-break: break-all;
	}

	.test-meta {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		flex-wrap: wrap;
	}

	.test-actions {
		flex-shrink: 0;
	}

	.include-toggle {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		cursor: pointer;
		user-select: none;
	}

	.include-toggle.disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.include-toggle input[type="checkbox"] {
		width: 16px;
		height: 16px;
		accent-color: var(--scrubs);
		cursor: inherit;
	}

	.toggle-label {
		font-size: 0.8125rem;
		color: var(--graphite);
	}

	.test-description {
		font-size: 0.875rem;
		color: var(--graphite);
		margin-bottom: var(--space-md);
		line-height: 1.5;
	}

	.test-code-section {
		border-top: 1px solid var(--silver);
		padding-top: var(--space-md);
	}

	.expand-toggle {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		background: transparent;
		border: none;
		padding: var(--space-xs) 0;
		font-size: 0.8125rem;
		color: var(--scrubs-dark);
		cursor: pointer;
		transition: color var(--transition-fast);
	}

	.expand-toggle:hover {
		color: var(--scrubs);
	}

	.expand-icon {
		width: 16px;
		height: 16px;
		transition: transform var(--transition-fast);
	}

	.expand-icon.expanded {
		transform: rotate(180deg);
	}

	.code-container {
		margin-top: var(--space-md);
		border-radius: var(--radius-md);
		overflow: hidden;
	}

	.test-code {
		margin: 0;
		padding: var(--space-lg);
		background: #1a1f2e;
		border: 1px solid #2d3548;
		font-family: 'SF Mono', 'Fira Code', monospace;
		font-size: 0.8125rem;
		line-height: 1.6;
		color: #e2e8f0;
		overflow-x: auto;
		white-space: pre;
		tab-size: 2;
	}

	.test-code code {
		display: block;
	}

	/* Responsive adjustments */
	@media (max-width: 768px) {
		.test-header {
			flex-direction: column;
			align-items: stretch;
		}

		.test-actions {
			margin-top: var(--space-sm);
		}

		.test-code {
			font-size: 0.75rem;
			padding: var(--space-md);
		}
	}

	@media (max-width: 480px) {
		.test-card {
			padding: var(--space-md);
		}

		.test-path {
			font-size: 0.75rem;
		}
	}
</style>
