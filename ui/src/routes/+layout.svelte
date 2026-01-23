<script lang="ts">
	import '../app.css';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { projects, incidents, stats, auth, demoMode } from '$lib/stores';

	let { children } = $props();

	function toggleDemoMode() {
		demoMode.toggle();
	}

	let currentPath = $derived($page.url.pathname);

	const navItems = [
		{ path: '/', label: 'Surgery Board', icon: 'board' },
		{ path: '/recovery', label: 'Recovery Ward', icon: 'recovery' },
		{ path: '/costs', label: 'Administration', icon: 'costs' },
		{ path: '/settings', label: 'Settings', icon: 'settings' },
	];

	onMount(() => {
		// Initialize demo mode from localStorage
		demoMode.init();

		// Initial load
		projects.load();
		incidents.load();
		stats.load();
		auth.load();

		// Start auto-refresh
		projects.startRefresh();
		incidents.startRefresh();

		return () => {
			projects.stopRefresh();
			incidents.stopRefresh();
		};
	});

	function isActive(path: string): boolean {
		if (path === '/') return currentPath === '/';
		return currentPath.startsWith(path);
	}
</script>

<div class="app">
	<!-- Top Bar -->
	<header class="top-bar">
		<div class="logo">
			<svg class="logo-icon" viewBox="0 0 32 32" fill="none">
				<rect x="2" y="8" width="28" height="16" rx="3" stroke="currentColor" stroke-width="2"/>
				<path d="M8 14h16M8 18h12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
				<circle cx="24" cy="12" r="1.5" fill="currentColor"/>
			</svg>
			<span class="logo-text">CRISPR</span>
			<span class="logo-subtitle">Surgery Center</span>
		</div>

		<nav class="main-nav">
			{#each navItems as item}
				<a
					href={item.path}
					class="nav-item"
					class:active={isActive(item.path)}
				>
					{#if item.icon === 'board'}
						<svg viewBox="0 0 20 20" fill="currentColor">
							<path d="M3 4a1 1 0 011-1h12a1 1 0 011 1v2a1 1 0 01-1 1H4a1 1 0 01-1-1V4zM3 10a1 1 0 011-1h6a1 1 0 011 1v6a1 1 0 01-1 1H4a1 1 0 01-1-1v-6zM14 9a1 1 0 00-1 1v6a1 1 0 001 1h2a1 1 0 001-1v-6a1 1 0 00-1-1h-2z"/>
						</svg>
					{:else if item.icon === 'recovery'}
						<svg viewBox="0 0 20 20" fill="currentColor">
							<path fill-rule="evenodd" d="M6.267 3.455a3.066 3.066 0 001.745-.723 3.066 3.066 0 013.976 0 3.066 3.066 0 001.745.723 3.066 3.066 0 012.812 2.812c.051.643.304 1.254.723 1.745a3.066 3.066 0 010 3.976 3.066 3.066 0 00-.723 1.745 3.066 3.066 0 01-2.812 2.812 3.066 3.066 0 00-1.745.723 3.066 3.066 0 01-3.976 0 3.066 3.066 0 00-1.745-.723 3.066 3.066 0 01-2.812-2.812 3.066 3.066 0 00-.723-1.745 3.066 3.066 0 010-3.976 3.066 3.066 0 00.723-1.745 3.066 3.066 0 012.812-2.812zm7.44 5.252a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"/>
						</svg>
					{:else if item.icon === 'costs'}
						<svg viewBox="0 0 20 20" fill="currentColor">
							<path d="M4 4a2 2 0 00-2 2v1h16V6a2 2 0 00-2-2H4z"/>
							<path fill-rule="evenodd" d="M18 9H2v5a2 2 0 002 2h12a2 2 0 002-2V9zM4 13a1 1 0 011-1h1a1 1 0 110 2H5a1 1 0 01-1-1zm5-1a1 1 0 100 2h1a1 1 0 100-2H9z" clip-rule="evenodd"/>
						</svg>
					{:else if item.icon === 'settings'}
						<svg viewBox="0 0 20 20" fill="currentColor">
							<path fill-rule="evenodd" d="M11.49 3.17c-.38-1.56-2.6-1.56-2.98 0a1.532 1.532 0 01-2.286.948c-1.372-.836-2.942.734-2.106 2.106.54.886.061 2.042-.947 2.287-1.561.379-1.561 2.6 0 2.978a1.532 1.532 0 01.947 2.287c-.836 1.372.734 2.942 2.106 2.106a1.532 1.532 0 012.287.947c.379 1.561 2.6 1.561 2.978 0a1.533 1.533 0 012.287-.947c1.372.836 2.942-.734 2.106-2.106a1.533 1.533 0 01.947-2.287c1.561-.379 1.561-2.6 0-2.978a1.532 1.532 0 01-.947-2.287c.836-1.372-.734-2.942-2.106-2.106a1.532 1.532 0 01-2.287-.947zM10 13a3 3 0 100-6 3 3 0 000 6z" clip-rule="evenodd"/>
						</svg>
					{/if}
					<span>{item.label}</span>
				</a>
			{/each}
		</nav>

		<div class="header-right">
			<div class="mode-toggle">
				<button
					class="toggle-btn"
					class:demo={$demoMode}
					onclick={toggleDemoMode}
					title={$demoMode ? 'Switch to Live Mode' : 'Switch to Demo Mode'}
				>
					<span class="toggle-label">{$demoMode ? 'DEMO' : 'LIVE'}</span>
					<span class="toggle-track">
						<span class="toggle-thumb"></span>
					</span>
				</button>
			</div>
			<div class="status-indicator" class:connected={$auth?.github?.connected}>
				<span class="status-dot"></span>
				<span class="status-text">{$auth?.github?.connected ? 'Systems Online' : 'Setup Required'}</span>
			</div>
		</div>
	</header>

	<!-- Main Content -->
	<main class="main-content">
		{@render children()}
	</main>
</div>

<style>
	.app {
		min-height: 100vh;
		display: flex;
		flex-direction: column;
	}

	.top-bar {
		display: flex;
		align-items: center;
		gap: var(--space-xl);
		padding: var(--space-md) var(--space-xl);
		background: var(--white);
		border-bottom: 1px solid var(--silver);
		position: sticky;
		top: 0;
		z-index: 100;
	}

	.logo {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.logo-icon {
		width: 28px;
		height: 28px;
		color: var(--scrubs);
	}

	.logo-text {
		font-family: var(--font-display);
		font-size: 1.25rem;
		color: var(--obsidian);
		letter-spacing: -0.02em;
	}

	.logo-subtitle {
		font-size: 0.75rem;
		color: var(--slate);
		padding-left: var(--space-sm);
		border-left: 1px solid var(--silver);
		margin-left: var(--space-sm);
	}

	.main-nav {
		display: flex;
		align-items: center;
		gap: var(--space-xs);
		flex: 1;
	}

	.nav-item {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-md);
		color: var(--graphite);
		font-size: 0.875rem;
		font-weight: 500;
		transition: all var(--transition-fast);
		text-decoration: none;
	}

	.nav-item:hover {
		background: var(--paper);
		color: var(--charcoal);
	}

	.nav-item.active {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.nav-item svg {
		width: 18px;
		height: 18px;
	}

	.header-right {
		display: flex;
		align-items: center;
		gap: var(--space-md);
	}

	.status-indicator {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-xs) var(--space-md);
		background: var(--alert-soft);
		border-radius: var(--radius-full);
	}

	.status-indicator.connected {
		background: var(--scrubs-soft);
	}

	.status-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--alert);
	}

	.status-indicator.connected .status-dot {
		background: var(--scrubs);
	}

	.status-text {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--charcoal);
	}

	.mode-toggle {
		display: flex;
		align-items: center;
	}

	.toggle-btn {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-xs) var(--space-sm);
		background: transparent;
		border: 1px solid var(--silver);
		border-radius: var(--radius-md);
		cursor: pointer;
		transition: all var(--transition-fast);
	}

	.toggle-btn:hover {
		background: var(--paper);
		border-color: var(--slate);
	}

	.toggle-label {
		font-size: 0.625rem;
		font-weight: 700;
		letter-spacing: 0.1em;
		color: var(--scrubs);
		min-width: 32px;
		pointer-events: none;
	}

	.toggle-btn.demo .toggle-label {
		color: var(--caution);
	}

	.toggle-track {
		position: relative;
		width: 32px;
		height: 16px;
		background: var(--scrubs);
		border-radius: var(--radius-full);
		transition: background var(--transition-fast);
		pointer-events: none;
	}

	.toggle-btn.demo .toggle-track {
		background: var(--caution);
	}

	.toggle-thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 12px;
		height: 12px;
		background: var(--white);
		border-radius: 50%;
		transition: transform var(--transition-fast);
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
	}

	.toggle-btn.demo .toggle-thumb {
		transform: translateX(16px);
	}

	.main-content {
		flex: 1;
		padding: var(--space-xl);
		max-width: 1600px;
		margin: 0 auto;
		width: 100%;
	}

	@media (max-width: 768px) {
		.top-bar {
			padding: var(--space-sm) var(--space-md);
			gap: var(--space-md);
		}

		.logo-subtitle {
			display: none;
		}

		.nav-item span {
			display: none;
		}

		.main-content {
			padding: var(--space-md);
		}
	}
</style>
