<script lang="ts">
	import '../app.css';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { projects, incidents, stats, auth, demoMode, theme } from '$lib/stores';
	import Toast from '$lib/components/Toast.svelte';

	let { children } = $props();

	let mobileMenuOpen = $state(false);

	function toggleDemoMode() {
		demoMode.toggle();
	}

	function toggleTheme() {
		theme.toggle();
	}

	function toggleMobileMenu() {
		mobileMenuOpen = !mobileMenuOpen;
	}

	function closeMobileMenu() {
		mobileMenuOpen = false;
	}

	let currentPath = $derived($page.url.pathname);

	const navItems = [
		{ path: '/', label: 'Surgery Board', icon: 'board' },
		{ path: '/recovery', label: 'Recovery Ward', icon: 'recovery' },
		{ path: '/costs', label: 'Administration', icon: 'costs' },
		{ path: '/settings', label: 'Settings', icon: 'settings' },
	];

	onMount(() => {
		// Initialize demo mode and theme from localStorage
		demoMode.init();
		theme.init();

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
	<!-- Skip to content link for keyboard navigation -->
	<a href="#main-content" class="skip-link">Skip to main content</a>

	<!-- Top Bar -->
	<header class="top-bar">
		<div class="logo">
			<svg class="logo-icon" viewBox="0 0 32 32" fill="none" aria-hidden="true">
				<rect x="2" y="8" width="28" height="16" rx="3" stroke="currentColor" stroke-width="2"/>
				<path d="M8 14h16M8 18h12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
				<circle cx="24" cy="12" r="1.5" fill="currentColor"/>
			</svg>
			<span class="logo-text">CRISPR</span>
			<span class="logo-subtitle">Surgery Center</span>
		</div>

		<!-- Mobile Menu Button -->
		<button
			class="mobile-menu-btn"
			onclick={toggleMobileMenu}
			aria-label="Toggle navigation menu"
			aria-expanded={mobileMenuOpen}
			aria-controls="main-navigation"
		>
			{#if mobileMenuOpen}
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
					<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
				</svg>
			{:else}
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
					<path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h16" />
				</svg>
			{/if}
		</button>

		<nav id="main-navigation" class="main-nav" class:mobile-open={mobileMenuOpen} aria-label="Main navigation">
			{#each navItems as item}
				<a
					href={item.path}
					class="nav-item"
					class:active={isActive(item.path)}
					onclick={closeMobileMenu}
					aria-current={isActive(item.path) ? 'page' : undefined}
				>
					{#if item.icon === 'board'}
						<svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
							<path d="M3 4a1 1 0 011-1h12a1 1 0 011 1v2a1 1 0 01-1 1H4a1 1 0 01-1-1V4zM3 10a1 1 0 011-1h6a1 1 0 011 1v6a1 1 0 01-1 1H4a1 1 0 01-1-1v-6zM14 9a1 1 0 00-1 1v6a1 1 0 001 1h2a1 1 0 001-1v-6a1 1 0 00-1-1h-2z"/>
						</svg>
					{:else if item.icon === 'recovery'}
						<svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
							<path fill-rule="evenodd" d="M6.267 3.455a3.066 3.066 0 001.745-.723 3.066 3.066 0 013.976 0 3.066 3.066 0 001.745.723 3.066 3.066 0 012.812 2.812c.051.643.304 1.254.723 1.745a3.066 3.066 0 010 3.976 3.066 3.066 0 00-.723 1.745 3.066 3.066 0 01-2.812 2.812 3.066 3.066 0 00-1.745.723 3.066 3.066 0 01-3.976 0 3.066 3.066 0 00-1.745-.723 3.066 3.066 0 01-2.812-2.812 3.066 3.066 0 00-.723-1.745 3.066 3.066 0 010-3.976 3.066 3.066 0 00.723-1.745 3.066 3.066 0 012.812-2.812zm7.44 5.252a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"/>
						</svg>
					{:else if item.icon === 'costs'}
						<svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
							<path d="M4 4a2 2 0 00-2 2v1h16V6a2 2 0 00-2-2H4z"/>
							<path fill-rule="evenodd" d="M18 9H2v5a2 2 0 002 2h12a2 2 0 002-2V9zM4 13a1 1 0 011-1h1a1 1 0 110 2H5a1 1 0 01-1-1zm5-1a1 1 0 100 2h1a1 1 0 100-2H9z" clip-rule="evenodd"/>
						</svg>
					{:else if item.icon === 'settings'}
						<svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
							<path fill-rule="evenodd" d="M11.49 3.17c-.38-1.56-2.6-1.56-2.98 0a1.532 1.532 0 01-2.286.948c-1.372-.836-2.942.734-2.106 2.106.54.886.061 2.042-.947 2.287-1.561.379-1.561 2.6 0 2.978a1.532 1.532 0 01.947 2.287c-.836 1.372.734 2.942 2.106 2.106a1.532 1.532 0 012.287.947c.379 1.561 2.6 1.561 2.978 0a1.533 1.533 0 012.287-.947c1.372.836 2.942-.734 2.106-2.106a1.533 1.533 0 01.947-2.287c1.561-.379 1.561-2.6 0-2.978a1.532 1.532 0 01-.947-2.287c.836-1.372-.734-2.942-2.106-2.106a1.532 1.532 0 01-2.287-.947zM10 13a3 3 0 100-6 3 3 0 000 6z" clip-rule="evenodd"/>
						</svg>
					{/if}
					<span>{item.label}</span>
				</a>
			{/each}
		</nav>

		<div class="header-right">
			<button
				class="night-shift-btn"
				class:active={$theme === 'dark'}
				onclick={toggleTheme}
				title={$theme === 'dark' ? 'Switch to Day Mode' : 'Switch to Night Shift'}
				aria-label={$theme === 'dark' ? 'Switch to Day Mode' : 'Switch to Night Shift'}
				aria-pressed={$theme === 'dark'}
			>
				{#if $theme === 'dark'}
					<svg class="theme-icon" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
						<path fill-rule="evenodd" d="M10 2a1 1 0 011 1v1a1 1 0 11-2 0V3a1 1 0 011-1zm4 8a4 4 0 11-8 0 4 4 0 018 0zm-.464 4.95l.707.707a1 1 0 001.414-1.414l-.707-.707a1 1 0 00-1.414 1.414zm2.12-10.607a1 1 0 010 1.414l-.706.707a1 1 0 11-1.414-1.414l.707-.707a1 1 0 011.414 0zM17 11a1 1 0 100-2h-1a1 1 0 100 2h1zm-7 4a1 1 0 011 1v1a1 1 0 11-2 0v-1a1 1 0 011-1zM5.05 6.464A1 1 0 106.465 5.05l-.708-.707a1 1 0 00-1.414 1.414l.707.707zm1.414 8.486l-.707.707a1 1 0 01-1.414-1.414l.707-.707a1 1 0 011.414 1.414zM4 11a1 1 0 100-2H3a1 1 0 000 2h1z" clip-rule="evenodd"/>
					</svg>
				{:else}
					<svg class="theme-icon" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
						<path d="M17.293 13.293A8 8 0 016.707 2.707a8.001 8.001 0 1010.586 10.586z"/>
					</svg>
				{/if}
				<span class="night-shift-label">Night Shift</span>
			</button>
			<div class="mode-toggle">
				<button
					class="toggle-btn"
					class:demo={$demoMode}
					onclick={toggleDemoMode}
					title={$demoMode ? 'Switch to Live Mode' : 'Switch to Demo Mode'}
					aria-label={$demoMode ? 'Switch to Live Mode' : 'Switch to Demo Mode'}
					aria-pressed={$demoMode}
					role="switch"
				>
					<span class="toggle-label" aria-hidden="true">{$demoMode ? 'DEMO' : 'LIVE'}</span>
					<span class="toggle-track" aria-hidden="true">
						<span class="toggle-thumb"></span>
					</span>
				</button>
			</div>
			<div class="status-indicator" class:connected={$auth?.github?.connected} role="status" aria-live="polite">
				<span class="status-dot" aria-hidden="true"></span>
				<span class="status-text">{$auth?.github?.connected ? 'Systems Online' : 'Setup Required'}</span>
			</div>
		</div>
	</header>

	<!-- Main Content -->
	<main id="main-content" class="main-content">
		{@render children()}
	</main>

	<!-- Toast Notifications -->
	<Toast />
</div>

<style>
	.app {
		min-height: 100vh;
		display: flex;
		flex-direction: column;
	}

	/* Skip link for keyboard navigation */
	.skip-link {
		position: absolute;
		top: -40px;
		left: 0;
		background: var(--scrubs);
		color: white;
		padding: var(--space-sm) var(--space-md);
		z-index: 1000;
		text-decoration: none;
		font-weight: 500;
	}

	.skip-link:focus {
		top: 0;
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

	.night-shift-btn {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-xs) var(--space-sm);
		background: transparent;
		border: 1px solid var(--silver);
		border-radius: var(--radius-md);
		cursor: pointer;
		transition: all var(--transition-fast);
		color: var(--graphite);
		font-family: var(--font-body);
		font-size: 0.75rem;
		font-weight: 500;
	}

	.night-shift-btn:hover {
		background: var(--paper);
		border-color: var(--slate);
		color: var(--charcoal);
	}

	.night-shift-btn.active {
		background: var(--stable-soft);
		border-color: var(--stable);
		color: var(--stable);
	}

	.theme-icon {
		width: 16px;
		height: 16px;
	}

	.night-shift-label {
		letter-spacing: 0.02em;
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

	/* Mobile Menu Button */
	.mobile-menu-btn {
		display: none;
		width: 44px;
		height: 44px;
		align-items: center;
		justify-content: center;
		background: transparent;
		border: 1px solid var(--silver);
		border-radius: var(--radius-md);
		color: var(--graphite);
		cursor: pointer;
		transition: all var(--transition-fast);
	}

	.mobile-menu-btn:hover {
		background: var(--paper);
		color: var(--charcoal);
	}

	.mobile-menu-btn svg {
		width: 24px;
		height: 24px;
	}

	@media (max-width: 768px) {
		.top-bar {
			padding: var(--space-sm) var(--space-md);
			gap: var(--space-sm);
			flex-wrap: wrap;
		}

		.logo {
			flex: 1;
		}

		.logo-subtitle {
			display: none;
		}

		.mobile-menu-btn {
			display: flex;
			order: 2;
		}

		.main-nav {
			display: none;
			position: absolute;
			top: 100%;
			left: 0;
			right: 0;
			background: var(--white);
			border-bottom: 1px solid var(--silver);
			padding: var(--space-md);
			flex-direction: column;
			gap: var(--space-xs);
			box-shadow: var(--shadow-lg);
			z-index: 99;
		}

		.main-nav.mobile-open {
			display: flex;
		}

		.nav-item {
			padding: var(--space-md);
			min-height: 44px;
		}

		.nav-item span {
			display: inline;
		}

		.header-right {
			order: 3;
			width: 100%;
			justify-content: flex-end;
			padding-top: var(--space-sm);
			border-top: 1px solid var(--paper);
			margin-top: var(--space-sm);
		}

		.night-shift-label {
			display: none;
		}

		.status-text {
			display: none;
		}

		.toggle-label {
			display: none;
		}

		.main-content {
			padding: var(--space-md);
		}
	}

	@media (max-width: 480px) {
		.logo-text {
			font-size: 1rem;
		}

		.header-right {
			gap: var(--space-sm);
		}

		.night-shift-btn {
			padding: var(--space-xs);
		}

		.toggle-btn {
			padding: var(--space-xs);
		}

		.status-indicator {
			padding: var(--space-xs) var(--space-sm);
		}
	}
</style>
