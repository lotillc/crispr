<script lang="ts">
	import { onMount } from 'svelte';
	import { auth, llmConfig, toasts } from '$lib/stores';
	import * as api from '$lib/api';
	import {
		validateRequired,
		validateUrl,
		validateApiKey,
		validateModel,
		validateProvider,
		createErrorId,
		type ValidationResult
	} from '$lib/validation';

	// LLM Configuration
	let readerProvider = $state('');
	let readerModel = $state('');
	let readerApiKey = $state('');
	let readerBaseUrl = $state('');

	let writerProvider = $state('');
	let writerModel = $state('');
	let writerApiKey = $state('');
	let writerBaseUrl = $state('');

	let internistProvider = $state('');
	let internistModel = $state('');
	let internistApiKey = $state('');
	let internistBaseUrl = $state('');

	let saving = $state(false);
	let verifying = $state<'reader' | 'writer' | 'internist' | null>(null);
	let verifyResult = $state<{ success: boolean; error?: string } | null>(null);
	let availableModels = $state<api.LlmModel[]>([]);
	let loadingModels = $state(false);

	// Validation state - track which fields have been touched
	let readerTouched = $state({
		provider: false,
		apiKey: false,
		baseUrl: false,
		model: false
	});
	let writerTouched = $state({
		provider: false,
		apiKey: false,
		baseUrl: false,
		model: false
	});
	let internistTouched = $state({
		provider: false,
		apiKey: false,
		baseUrl: false,
		model: false
	});

	// Save success state
	let readerSaveSuccess = $state(false);
	let writerSaveSuccess = $state(false);
	let internistSaveSuccess = $state(false);

	// Derived validation states for Reader
	let readerProviderValidation = $derived(validateProvider(readerProvider));
	let readerApiKeyValidation = $derived(
		readerProvider && readerProvider !== 'ollama'
			? validateApiKey(readerApiKey, readerProvider)
			: { valid: true }
	);
	// For API key, also check if we have an existing config (key already stored)
	let readerApiKeyRequired = $derived(
		readerProvider &&
		readerProvider !== 'ollama' &&
		$llmConfig?.reader_provider !== readerProvider
	);
	let readerBaseUrlValidation = $derived(
		readerProvider === 'ollama'
			? validateUrl(readerBaseUrl, false)
			: { valid: true }
	);
	let readerModelValidation = $derived(validateModel(readerModel));

	// Derived validation states for Writer
	let writerProviderValidation = $derived(validateProvider(writerProvider));
	let writerApiKeyValidation = $derived(
		writerProvider && writerProvider !== 'ollama'
			? validateApiKey(writerApiKey, writerProvider)
			: { valid: true }
	);
	let writerApiKeyRequired = $derived(
		writerProvider &&
		writerProvider !== 'ollama' &&
		$llmConfig?.writer_provider !== writerProvider
	);
	let writerBaseUrlValidation = $derived(
		writerProvider === 'ollama'
			? validateUrl(writerBaseUrl, false)
			: { valid: true }
	);
	let writerModelValidation = $derived(validateModel(writerModel));

	// Derived validation states for Internist
	let internistProviderValidation = $derived(validateProvider(internistProvider));
	let internistApiKeyValidation = $derived(
		internistProvider && internistProvider !== 'ollama'
			? validateApiKey(internistApiKey, internistProvider)
			: { valid: true }
	);
	let internistApiKeyRequired = $derived(
		internistProvider &&
		internistProvider !== 'ollama' &&
		$llmConfig?.internist_provider !== internistProvider
	);
	let internistBaseUrlValidation = $derived(
		internistProvider === 'ollama'
			? validateUrl(internistBaseUrl, false)
			: { valid: true }
	);
	let internistModelValidation = $derived(validateModel(internistModel));

	// Overall form validity
	let readerFormValid = $derived(
		readerProviderValidation.valid &&
		(readerProvider === 'ollama' || !readerApiKeyRequired || readerApiKeyValidation.valid) &&
		readerBaseUrlValidation.valid &&
		readerModelValidation.valid
	);

	let writerFormValid = $derived(
		writerProviderValidation.valid &&
		(writerProvider === 'ollama' || !writerApiKeyRequired || writerApiKeyValidation.valid) &&
		writerBaseUrlValidation.valid &&
		writerModelValidation.valid
	);

	let internistFormValid = $derived(
		internistProviderValidation.valid &&
		(internistProvider === 'ollama' || !internistApiKeyRequired || internistApiKeyValidation.valid) &&
		internistBaseUrlValidation.valid &&
		internistModelValidation.valid
	);

	// Helper to show error only when touched and invalid
	function shouldShowError(touched: boolean, validation: ValidationResult): boolean {
		return touched && !validation.valid;
	}

	// Slack state
	let slackChannels = $state<api.SlackChannel[]>([]);
	let loadingChannels = $state(false);
	let selectedChannel = $state('');
	let slackMessage = $state('');
	let testingSlack = $state(false);
	let disconnectingSlack = $state(false);

	// Webhook settings
	let webhookSettings = $state<api.WebhookSettings | null>(null);
	let loadingWebhooks = $state(false);
	let savingWebhooks = $state(false);

	// Webhook form state
	let sentryEnabled = $state(false);
	let sentrySecret = $state('');
	let datadogEnabled = $state(false);
	let datadogToken = $state('');
	let grafanaEnabled = $state(false);
	let grafanaToken = $state('');

	// Issue Tracker Integrations
	let jiraStatus = $state<api.JiraConnectionStatus | null>(null);
	let linearStatus = $state<api.LinearConnectionStatus | null>(null);
	let loadingJira = $state(false);
	let loadingLinear = $state(false);
	let connectingJira = $state(false);
	let connectingLinear = $state(false);
	let disconnectingJira = $state(false);
	let disconnectingLinear = $state(false);

	// Jira form
	let jiraExpanded = $state(false);
	let jiraSiteUrl = $state('');
	let jiraEmail = $state('');
	let jiraApiToken = $state('');

	// Linear form
	let linearExpanded = $state(false);
	let linearApiKey = $state('');

	// Token visibility state
	let showDatadogToken = $state(false);
	let showGrafanaToken = $state(false);

	// Generate a secure random token
	function generateToken(): string {
		const array = new Uint8Array(32);
		crypto.getRandomValues(array);
		return Array.from(array, b => b.toString(16).padStart(2, '0')).join('');
	}

	function generateDatadogToken() {
		datadogToken = generateToken();
		showDatadogToken = true;
	}

	function generateGrafanaToken() {
		grafanaToken = generateToken();
		showGrafanaToken = true;
	}

	async function copyToClipboard(text: string) {
		await navigator.clipboard.writeText(text);
		toasts.success('Copied to clipboard');
	}

	function getWebhookUrl(path: string): string {
		return `${window.location.origin}${path}`;
	}

	const providers = [
		{ value: 'anthropic', label: 'Anthropic (Claude)' },
		{ value: 'openai', label: 'OpenAI (GPT)' },
		{ value: 'ollama', label: 'Ollama (Local)' },
	];

	onMount(async () => {
		await auth.load();
		await llmConfig.load();

		// Populate form with existing config
		if ($llmConfig) {
			readerProvider = $llmConfig.reader_provider ?? '';
			readerModel = $llmConfig.reader_model ?? '';
			readerBaseUrl = $llmConfig.reader_base_url ?? '';
			writerProvider = $llmConfig.writer_provider ?? '';
			writerModel = $llmConfig.writer_model ?? '';
			writerBaseUrl = $llmConfig.writer_base_url ?? '';
			internistProvider = $llmConfig.internist_provider ?? '';
			internistModel = $llmConfig.internist_model ?? '';
			internistBaseUrl = $llmConfig.internist_base_url ?? '';
		}

		// Load Slack channels if connected
		if ($auth?.slack?.connected) {
			await loadSlackChannels();
		}

		// Load webhook settings
		await loadWebhookSettings();

		// Load issue tracker statuses
		await loadIssueTrackerStatuses();
	});

	async function loadIssueTrackerStatuses() {
		loadingJira = true;
		loadingLinear = true;
		try {
			jiraStatus = await api.getJiraStatus();
		} catch (e) {
			console.error('Failed to load Jira status:', e);
		} finally {
			loadingJira = false;
		}
		try {
			linearStatus = await api.getLinearStatus();
		} catch (e) {
			console.error('Failed to load Linear status:', e);
		} finally {
			loadingLinear = false;
		}
	}

	async function handleConnectJira() {
		if (!jiraSiteUrl || !jiraEmail || !jiraApiToken) return;
		connectingJira = true;
		try {
			await api.connectJira(jiraSiteUrl, jiraEmail, jiraApiToken);
			jiraStatus = await api.getJiraStatus();
			jiraSiteUrl = '';
			jiraEmail = '';
			jiraApiToken = '';
			toasts.success('Jira connected successfully');
		} catch (e) {
			toasts.error(`Failed to connect Jira: ${(e as Error).message}`);
		} finally {
			connectingJira = false;
		}
	}

	async function handleDisconnectJira() {
		disconnectingJira = true;
		try {
			await api.disconnectJira();
			jiraStatus = { connected: false };
			toasts.info('Jira disconnected');
		} catch (e) {
			toasts.error(`Failed to disconnect Jira: ${(e as Error).message}`);
		} finally {
			disconnectingJira = false;
		}
	}

	async function handleConnectLinear() {
		if (!linearApiKey) return;
		connectingLinear = true;
		try {
			await api.connectLinear(linearApiKey);
			linearStatus = await api.getLinearStatus();
			linearApiKey = '';
			toasts.success('Linear connected successfully');
		} catch (e) {
			toasts.error(`Failed to connect Linear: ${(e as Error).message}`);
		} finally {
			connectingLinear = false;
		}
	}

	async function handleDisconnectLinear() {
		disconnectingLinear = true;
		try {
			await api.disconnectLinear();
			linearStatus = { connected: false };
			toasts.info('Linear disconnected');
		} catch (e) {
			toasts.error(`Failed to disconnect Linear: ${(e as Error).message}`);
		} finally {
			disconnectingLinear = false;
		}
	}

	async function loadWebhookSettings() {
		loadingWebhooks = true;
		try {
			webhookSettings = await api.getWebhookSettings();
			// Populate form
			sentryEnabled = webhookSettings.sentry.enabled;
			datadogEnabled = webhookSettings.datadog.enabled;
			grafanaEnabled = webhookSettings.grafana.enabled;
		} catch (e) {
			console.error('Failed to load webhook settings:', e);
		} finally {
			loadingWebhooks = false;
		}
	}

	async function saveWebhookSettings() {
		savingWebhooks = true;
		try {
			const req: api.UpdateWebhookSettingsRequest = {
				sentry_enabled: sentryEnabled,
				datadog_enabled: datadogEnabled,
				grafana_enabled: grafanaEnabled,
			};
			// Only send secrets if they've been entered (not placeholder)
			if (sentrySecret) req.sentry_secret = sentrySecret;
			if (datadogToken) req.datadog_token = datadogToken;
			if (grafanaToken) req.grafana_token = grafanaToken;

			webhookSettings = await api.updateWebhookSettings(req);
			// Clear the secret inputs after save
			sentrySecret = '';
			datadogToken = '';
			grafanaToken = '';
			toasts.success('Webhook settings saved');
		} catch (e) {
			toasts.error(`Failed to save webhook settings: ${(e as Error).message}`);
		} finally {
			savingWebhooks = false;
		}
	}

	async function loadSlackChannels() {
		loadingChannels = true;
		try {
			slackChannels = await api.getSlackChannels();
		} catch (e) {
			console.error('Failed to load Slack channels:', e);
		} finally {
			loadingChannels = false;
		}
	}

	async function setSlackChannel() {
		if (!selectedChannel) return;
		const channel = slackChannels.find(c => c.id === selectedChannel);
		if (!channel) return;

		try {
			const result = await api.setSlackChannel(channel.id, channel.name);
			slackMessage = result.message;
			await auth.load(); // Refresh auth status
			toasts.success(`Slack channel set to #${channel.name}`);
		} catch (e) {
			slackMessage = (e as Error).message;
			toasts.error(`Failed to set Slack channel: ${(e as Error).message}`);
		}
	}

	async function testSlack() {
		testingSlack = true;
		slackMessage = '';
		try {
			const result = await api.testSlackConnection();
			slackMessage = result.message;
			toasts.success('Test message sent to Slack');
		} catch (e) {
			slackMessage = (e as Error).message;
			toasts.error(`Slack test failed: ${(e as Error).message}`);
		} finally {
			testingSlack = false;
		}
	}

	async function disconnectSlack() {
		disconnectingSlack = true;
		try {
			await api.disconnectSlack();
			await auth.load();
			slackChannels = [];
			slackMessage = '';
			toasts.info('Slack disconnected');
		} catch (e) {
			console.error('Failed to disconnect Slack:', e);
			toasts.error(`Failed to disconnect Slack: ${(e as Error).message}`);
		} finally {
			disconnectingSlack = false;
		}
	}

	async function verifyLlm(role: 'reader' | 'writer' | 'internist') {
		verifying = role;
		verifyResult = null;

		let config;
		if (role === 'reader') {
			config = { provider: readerProvider, api_key: readerApiKey || undefined, model: readerModel, base_url: readerBaseUrl || undefined };
		} else if (role === 'writer') {
			config = { provider: writerProvider, api_key: writerApiKey || undefined, model: writerModel, base_url: writerBaseUrl || undefined };
		} else {
			config = { provider: internistProvider, api_key: internistApiKey || undefined, model: internistModel, base_url: internistBaseUrl || undefined };
		}

		try {
			verifyResult = await api.verifyLlm(config);
		} catch (e) {
			verifyResult = { success: false, error: (e as Error).message };
		} finally {
			verifying = null;
		}
	}

	async function loadModels(role: 'reader' | 'writer' | 'internist') {
		loadingModels = true;
		let config;
		if (role === 'reader') {
			config = { provider: readerProvider, api_key: readerApiKey || undefined, base_url: readerBaseUrl || undefined };
		} else if (role === 'writer') {
			config = { provider: writerProvider, api_key: writerApiKey || undefined, base_url: writerBaseUrl || undefined };
		} else {
			config = { provider: internistProvider, api_key: internistApiKey || undefined, base_url: internistBaseUrl || undefined };
		}

		try {
			availableModels = await api.listLlmModels(config);
		} catch (e) {
			console.error('Failed to load models:', e);
			availableModels = [];
		} finally {
			loadingModels = false;
		}
	}

	async function saveLlmConfig(role: 'reader' | 'writer' | 'internist') {
		saving = true;
		// Reset success states
		if (role === 'reader') readerSaveSuccess = false;
		else if (role === 'writer') writerSaveSuccess = false;
		else internistSaveSuccess = false;

		let config;
		if (role === 'reader') {
			config = { provider: readerProvider, api_key: readerApiKey || undefined, model: readerModel, base_url: readerBaseUrl || undefined, role: 'reader' as const };
		} else if (role === 'writer') {
			config = { provider: writerProvider, api_key: writerApiKey || undefined, model: writerModel, base_url: writerBaseUrl || undefined, role: 'writer' as const };
		} else {
			config = { provider: internistProvider, api_key: internistApiKey || undefined, model: internistModel, base_url: internistBaseUrl || undefined, role: 'internist' as const };
		}

		try {
			await llmConfig.update(config);
			// Clear API key field after save (it's stored securely)
			if (role === 'reader') {
				readerApiKey = '';
				readerSaveSuccess = true;
				// Clear success after 3 seconds
				setTimeout(() => { readerSaveSuccess = false; }, 3000);
			} else if (role === 'writer') {
				writerApiKey = '';
				writerSaveSuccess = true;
				setTimeout(() => { writerSaveSuccess = false; }, 3000);
			} else {
				internistApiKey = '';
				internistSaveSuccess = true;
				setTimeout(() => { internistSaveSuccess = false; }, 3000);
			}
			const roleName = role === 'reader' ? 'Reader' : role === 'writer' ? 'Writer' : 'Internist';
			toasts.success(`${roleName} LLM configuration saved`);
		} catch (e) {
			console.error('Failed to save LLM config:', e);
			toasts.error(`Failed to save LLM config: ${(e as Error).message}`);
		} finally {
			saving = false;
		}
	}

	// Mark all fields as touched for validation display
	function touchAllReaderFields() {
		readerTouched = {
			provider: true,
			apiKey: true,
			baseUrl: true,
			model: true
		};
	}

	function touchAllWriterFields() {
		writerTouched = {
			provider: true,
			apiKey: true,
			baseUrl: true,
			model: true
		};
	}

	function touchAllInternistFields() {
		internistTouched = {
			provider: true,
			apiKey: true,
			baseUrl: true,
			model: true
		};
	}
</script>

<svelte:head>
	<title>Settings - CRISPR</title>
</svelte:head>

<div class="settings-page">
	<header class="page-header">
		<h1>Settings</h1>
		<p class="page-subtitle">Configure integrations and LLM providers</p>
	</header>

	<!-- Connections Section -->
	<section class="settings-section">
		<h2>Connections</h2>
		<p class="section-desc">Connect external services for full functionality</p>

		<div class="connection-grid">
			<!-- GitHub -->
			<div class="connection-card">
				<div class="connection-icon github">
					<svg viewBox="0 0 24 24" fill="currentColor">
						<path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
					</svg>
				</div>
				<div class="connection-info">
					<h3>GitHub</h3>
					<p>Required for reading code and creating pull requests</p>
				</div>
				<div class="connection-status">
					{#if $auth?.github?.connected && $auth.github.user}
						<span class="status-badge connected">
							<svg viewBox="0 0 20 20" fill="currentColor">
								<path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
							</svg>
							Connected as {$auth.github.user.login}
						</span>
					{:else if $auth?.github?.oauth_url}
						<a href={$auth.github.oauth_url} class="btn btn-primary">
							Connect GitHub
						</a>
					{:else}
						<a href={api.getGithubAuthUrl()} class="btn btn-primary">
							Connect GitHub
						</a>
					{/if}
				</div>
			</div>

			<!-- Slack -->
			<div class="connection-card slack-card">
				<div class="connection-main">
					<div class="connection-icon slack">
						<svg viewBox="0 0 24 24" fill="currentColor">
							<path d="M5.042 15.165a2.528 2.528 0 0 1-2.52 2.523A2.528 2.528 0 0 1 0 15.165a2.527 2.527 0 0 1 2.522-2.52h2.52v2.52zM6.313 15.165a2.527 2.527 0 0 1 2.521-2.52 2.527 2.527 0 0 1 2.521 2.52v6.313A2.528 2.528 0 0 1 8.834 24a2.528 2.528 0 0 1-2.521-2.522v-6.313zM8.834 5.042a2.528 2.528 0 0 1-2.521-2.52A2.528 2.528 0 0 1 8.834 0a2.528 2.528 0 0 1 2.521 2.522v2.52H8.834zM8.834 6.313a2.528 2.528 0 0 1 2.521 2.521 2.528 2.528 0 0 1-2.521 2.521H2.522A2.528 2.528 0 0 1 0 8.834a2.528 2.528 0 0 1 2.522-2.521h6.312zM18.956 8.834a2.528 2.528 0 0 1 2.522-2.521A2.528 2.528 0 0 1 24 8.834a2.528 2.528 0 0 1-2.522 2.521h-2.522V8.834zM17.688 8.834a2.528 2.528 0 0 1-2.523 2.521 2.527 2.527 0 0 1-2.52-2.521V2.522A2.527 2.527 0 0 1 15.165 0a2.528 2.528 0 0 1 2.523 2.522v6.312zM15.165 18.956a2.528 2.528 0 0 1 2.523 2.522A2.528 2.528 0 0 1 15.165 24a2.527 2.527 0 0 1-2.52-2.522v-2.522h2.52zM15.165 17.688a2.527 2.527 0 0 1-2.52-2.523 2.526 2.526 0 0 1 2.52-2.52h6.313A2.527 2.527 0 0 1 24 15.165a2.528 2.528 0 0 1-2.522 2.523h-6.313z"/>
						</svg>
					</div>
					<div class="connection-info">
						<h3>Slack</h3>
						<p>Optional. Get notified when PRs are created</p>
					</div>
					<div class="connection-status">
						{#if $auth?.slack?.connected}
							<span class="status-badge connected">
								<svg viewBox="0 0 20 20" fill="currentColor">
									<path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
								</svg>
								{$auth.slack.team_name}
							</span>
						{:else if $auth?.slack?.enabled === false}
							<span class="status-badge disabled">Not Enabled</span>
						{:else}
							<a href={api.getSlackAuthUrl()} class="btn btn-secondary">
								Connect Slack
							</a>
						{/if}
					</div>
				</div>

				<!-- Slack Channel Selection -->
				{#if $auth?.slack?.connected}
					<div class="slack-config">
						<div class="channel-selector">
							<label class="channel-label" for="slack-channel-select" id="slack-channel-label">Notification Channel</label>
							<div class="channel-controls">
								{#if loadingChannels}
									<span class="loading-text" role="status">Loading channels...</span>
								{:else}
									<select id="slack-channel-select" bind:value={selectedChannel} class="channel-select" aria-labelledby="slack-channel-label">
										<option value="">
											{$auth.slack.channel_name ? `#${$auth.slack.channel_name}` : 'Select channel...'}
										</option>
										{#each slackChannels as channel}
											<option value={channel.id}>
												{channel.is_private ? '🔒' : '#'}{channel.name}
											</option>
										{/each}
									</select>
									<button
										class="btn btn-secondary"
										onclick={setSlackChannel}
										disabled={!selectedChannel}
										aria-label="Set selected Slack channel for notifications"
									>
										Set Channel
									</button>
								{/if}
							</div>
						</div>

						{#if $auth.slack.channel_name}
							<div class="slack-actions">
								<button
									class="btn btn-ghost"
									onclick={testSlack}
									disabled={testingSlack}
									aria-label="Send test message to Slack channel"
								>
									{testingSlack ? 'Sending...' : 'Send Test Message'}
								</button>
								<button
									class="btn btn-ghost disconnect"
									onclick={disconnectSlack}
									disabled={disconnectingSlack}
									aria-label="Disconnect Slack integration"
								>
									{disconnectingSlack ? 'Disconnecting...' : 'Disconnect'}
								</button>
							</div>
						{/if}

						{#if slackMessage}
							<p class="slack-message">{slackMessage}</p>
						{/if}
					</div>
				{/if}
			</div>
		</div>
	</section>

	<!-- Webhook Integrations -->
	<section class="settings-section">
		<h2>Webhook Integrations</h2>
		<p class="section-desc">Receive errors from external monitoring platforms</p>

		{#if loadingWebhooks}
			<div class="loading-state">Loading webhook settings...</div>
		{:else}
			<div class="webhook-grid">
				<!-- Sentry -->
				<div class="webhook-card">
					<div class="webhook-header">
						<div class="webhook-icon sentry">
							<svg viewBox="0 0 24 24" fill="currentColor">
								<path d="M13.91 2.53c-.59-.29-1.31-.03-1.59.57L9.95 8.3c2.38 1.38 4.07 3.69 4.52 6.4h3.48c-.52-4.62-3.52-8.51-8.04-10.17zM3.35 17.78c-.24.47-.07 1.04.39 1.29l3.89 2c.47.24 1.04.07 1.29-.39.24-.47.07-1.04-.39-1.29l-3.89-2c-.47-.24-1.04-.07-1.29.39zm6.29-5.78c0-2.47-2-4.47-4.47-4.47-.47 0-.91.08-1.35.21l1.74 3c.35.6.14 1.37-.46 1.71-.6.35-1.37.14-1.71-.46L1.65 9c-.52.9-.82 1.93-.82 3.03 0 3.31 2.69 6 6 6h7.41c-.35-2.14-1.67-3.97-3.6-5.03z"/>
							</svg>
						</div>
						<div class="webhook-info">
							<h3>Sentry</h3>
							<p>Receive error events from Sentry</p>
						</div>
						<label class="toggle">
							<input type="checkbox" bind:checked={sentryEnabled} />
							<span class="toggle-slider"></span>
						</label>
					</div>
					{#if sentryEnabled}
						<div class="webhook-config">
							<div class="webhook-instructions">
								<h4>Setup Instructions</h4>
								<ol>
									<li>In Sentry, go to <strong>Settings → Integrations → Webhooks</strong></li>
									<li>Add the webhook URL below</li>
									<li>Set a <strong>Secret</strong> in Sentry, then enter it here</li>
									<li>Select which events to send (Issues, Errors, etc.)</li>
								</ol>
								<div class="webhook-requirement">
									<strong>Required:</strong> Name your Sentry project as <code>owner-repo</code> (e.g., <code>acme-api</code> for <code>acme/api</code>), or add a <code>project</code> tag to your events with value <code>owner/repo</code>
								</div>
							</div>
							<div class="field">
								<label class="field-label" for="sentry-secret">Webhook Secret (from Sentry)</label>
								<input
									id="sentry-secret"
									type="password"
									bind:value={sentrySecret}
									placeholder={webhookSettings?.sentry.has_secret ? '••••••••' : 'Paste secret from Sentry'}
									class="field-input"
								/>
							</div>
							<div class="webhook-url-field">
								<label class="field-label">Webhook URL</label>
								<div class="copyable-field">
									<code>{getWebhookUrl('/webhooks/sentry')}</code>
									<button class="btn btn-ghost btn-sm" onclick={() => copyToClipboard(getWebhookUrl('/webhooks/sentry'))} aria-label="Copy webhook URL">
										<svg viewBox="0 0 20 20" fill="currentColor" class="copy-icon">
											<path d="M8 2a1 1 0 000 2h2a1 1 0 100-2H8z"/>
											<path d="M3 5a2 2 0 012-2 3 3 0 003 3h2a3 3 0 003-3 2 2 0 012 2v6h-4.586l1.293-1.293a1 1 0 00-1.414-1.414l-3 3a1 1 0 000 1.414l3 3a1 1 0 001.414-1.414L10.414 13H15v3a2 2 0 01-2 2H5a2 2 0 01-2-2V5zM15 11h2a1 1 0 110 2h-2v-2z"/>
										</svg>
									</button>
								</div>
							</div>
						</div>
					{/if}
				</div>

				<!-- Datadog -->
				<div class="webhook-card">
					<div class="webhook-header">
						<div class="webhook-icon datadog">
							<svg viewBox="0 0 24 24" fill="currentColor">
								<path d="M12.29 2.31c-.14-.05-.29-.06-.43-.03-.14.03-.27.1-.37.2l-2.66 2.66c-.2.2-.29.49-.23.77.06.28.25.51.51.62l.08.03 1.6.53-1.03 3.1-2.83-.94c-.28-.09-.58-.03-.81.16-.23.19-.35.48-.33.77l.28 3.93c.02.29.18.55.43.71.25.16.56.19.84.08l2.88-1.15 1.04 3.13-1.62.54c-.28.09-.49.32-.56.6-.07.28.01.58.21.78l2.66 2.66c.1.1.23.17.37.2.14.03.29.02.43-.03l8.09-2.99c.27-.1.47-.33.54-.61.07-.28-.01-.58-.21-.78l-2.66-2.66c-.2-.2-.49-.28-.77-.23-.28.06-.52.25-.63.51l-.52 1.58-3.11-1.04.94-2.83c.09-.28.03-.58-.16-.81-.19-.23-.48-.36-.77-.33l-3.93.28c-.29.02-.56.18-.72.43-.16.25-.2.56-.09.84l1.15 2.88-3.12 1.04-.53-1.61c-.09-.28-.32-.49-.6-.56-.28-.07-.58 0-.78.21l-2.66 2.66c-.1.1-.17.23-.2.37-.03.14-.02.29.03.43l2.99 8.09c.1.27.33.47.61.54.28.07.58-.01.78-.21l2.66-2.66c.2-.2.28-.49.23-.77-.05-.28-.25-.52-.51-.63l-1.58-.52 1.04-3.11 2.83.94c.28.09.58.03.81-.16.23-.19.36-.48.33-.77l-.28-3.93c-.02-.29-.18-.56-.43-.72-.25-.16-.56-.2-.84-.09l-2.88 1.15-1.04-3.12 1.61-.53c.28-.09.49-.32.56-.6.07-.28 0-.58-.21-.78l-2.66-2.66c-.2-.2-.49-.28-.77-.23-.28.05-.52.25-.63.51l-.52 1.58-3.11-1.04.94-2.83c.09-.28.03-.58-.16-.81-.19-.23-.48-.36-.77-.33l-3.93.28c-.29.02-.56.18-.72.43-.16.25-.2.56-.09.84l1.15 2.88-3.12 1.04-.53-1.61c-.09-.28-.32-.49-.6-.56-.28-.07-.58 0-.78.21l-2.66 2.66c-.1.1-.17.23-.2.37-.03.14-.02.29.03.43l2.99 8.09z"/>
							</svg>
						</div>
						<div class="webhook-info">
							<h3>Datadog</h3>
							<p>Receive monitor alerts from Datadog</p>
						</div>
						<label class="toggle">
							<input type="checkbox" bind:checked={datadogEnabled} />
							<span class="toggle-slider"></span>
						</label>
					</div>
					{#if datadogEnabled}
						<div class="webhook-config">
							<div class="webhook-instructions">
								<h4>Setup Instructions</h4>
								<ol>
									<li>Generate a token below, then <strong>save settings</strong></li>
									<li>In Datadog, go to <strong>Integrations → Webhooks</strong></li>
									<li>Create a new webhook with the URL below</li>
									<li>Add custom header: <code>X-Datadog-Token: <em>your-token</em></code></li>
								</ol>
								<div class="webhook-requirement">
									<strong>Required:</strong> Add tag <code>project:owner/repo</code> to your monitors (e.g., <code>project:acme/api</code>)
								</div>
							</div>
							<div class="field">
								<label class="field-label" for="datadog-token">Authentication Token</label>
								<div class="token-field">
									<input
										id="datadog-token"
										type={showDatadogToken ? 'text' : 'password'}
										bind:value={datadogToken}
										placeholder={webhookSettings?.datadog.has_secret ? '••••••••' : 'Click Generate to create token'}
										class="field-input"
										readonly={!showDatadogToken && !datadogToken}
									/>
									<button class="btn btn-secondary btn-sm" onclick={generateDatadogToken}>
										Generate
									</button>
									{#if datadogToken}
										<button class="btn btn-ghost btn-sm" onclick={() => copyToClipboard(datadogToken)} aria-label="Copy token">
											<svg viewBox="0 0 20 20" fill="currentColor" class="copy-icon">
												<path d="M8 2a1 1 0 000 2h2a1 1 0 100-2H8z"/>
												<path d="M3 5a2 2 0 012-2 3 3 0 003 3h2a3 3 0 003-3 2 2 0 012 2v6h-4.586l1.293-1.293a1 1 0 00-1.414-1.414l-3 3a1 1 0 000 1.414l3 3a1 1 0 001.414-1.414L10.414 13H15v3a2 2 0 01-2 2H5a2 2 0 01-2-2V5zM15 11h2a1 1 0 110 2h-2v-2z"/>
											</svg>
										</button>
									{/if}
								</div>
								{#if showDatadogToken && datadogToken}
									<span class="field-hint field-hint-warning">Copy this token and paste it in Datadog before saving</span>
								{/if}
							</div>
							<div class="webhook-url-field">
								<label class="field-label">Webhook URL</label>
								<div class="copyable-field">
									<code>{getWebhookUrl('/webhooks/datadog')}</code>
									<button class="btn btn-ghost btn-sm" onclick={() => copyToClipboard(getWebhookUrl('/webhooks/datadog'))} aria-label="Copy webhook URL">
										<svg viewBox="0 0 20 20" fill="currentColor" class="copy-icon">
											<path d="M8 2a1 1 0 000 2h2a1 1 0 100-2H8z"/>
											<path d="M3 5a2 2 0 012-2 3 3 0 003 3h2a3 3 0 003-3 2 2 0 012 2v6h-4.586l1.293-1.293a1 1 0 00-1.414-1.414l-3 3a1 1 0 000 1.414l3 3a1 1 0 001.414-1.414L10.414 13H15v3a2 2 0 01-2 2H5a2 2 0 01-2-2V5zM15 11h2a1 1 0 110 2h-2v-2z"/>
										</svg>
									</button>
								</div>
							</div>
						</div>
					{/if}
				</div>

				<!-- Grafana -->
				<div class="webhook-card">
					<div class="webhook-header">
						<div class="webhook-icon grafana">
							<svg viewBox="0 0 24 24" fill="currentColor">
								<path d="M22.16 7.82c-.02-.2-.04-.39-.07-.58-.04-.25-.17-.48-.37-.64-.2-.16-.45-.24-.7-.22-.25.02-.48.13-.65.31-.17.18-.27.42-.28.67 0 .05 0 .11.01.16-.07-.02-.14-.04-.21-.05-.19-.03-.39 0-.56.1-.17.1-.3.25-.38.43-.08.18-.1.38-.06.57.04.19.14.37.29.5-.1.07-.2.16-.28.26-.14.17-.22.39-.23.61-.01.22.05.44.18.62.13.18.31.32.52.39-.04.12-.07.24-.08.37-.03.22.02.45.13.64.11.19.28.35.49.44.21.09.44.11.66.06-.01.12 0 .25.03.37.05.21.17.4.33.54.16.14.37.23.58.24.21.01.43-.04.61-.16.08.11.18.21.29.29.17.13.38.2.59.21.21.01.43-.05.61-.17.18-.12.32-.3.4-.5.12.05.25.08.38.1.22.02.44-.03.63-.14.19-.11.34-.28.42-.48.08-.2.1-.42.05-.63.12-.03.23-.08.34-.14.19-.11.34-.28.43-.48.09-.2.11-.43.05-.64-.06-.21-.18-.4-.36-.53.09-.09.16-.19.22-.31.11-.19.15-.41.11-.63-.04-.22-.15-.42-.32-.57-.17-.15-.38-.24-.6-.25.04-.12.07-.25.08-.38.02-.22-.03-.44-.15-.63-.12-.19-.3-.33-.51-.41-.21-.08-.44-.09-.66-.03.01-.13-.01-.26-.04-.38-.06-.21-.18-.4-.35-.53-.17-.13-.38-.21-.59-.21-.21 0-.42.07-.59.21-.08-.1-.17-.18-.28-.26-.17-.12-.37-.19-.58-.19-.21 0-.42.07-.59.2-.17.13-.3.31-.35.52zm-10.18 1.09c.58 0 1.05.47 1.05 1.05s-.47 1.05-1.05 1.05-1.05-.47-1.05-1.05.47-1.05 1.05-1.05zm0 7.14c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5z"/>
							</svg>
						</div>
						<div class="webhook-info">
							<h3>Grafana</h3>
							<p>Receive alert notifications from Grafana</p>
						</div>
						<label class="toggle">
							<input type="checkbox" bind:checked={grafanaEnabled} />
							<span class="toggle-slider"></span>
						</label>
					</div>
					{#if grafanaEnabled}
						<div class="webhook-config">
							<div class="webhook-instructions">
								<h4>Setup Instructions</h4>
								<ol>
									<li>Generate a token below, then <strong>save settings</strong></li>
									<li>In Grafana, go to <strong>Alerting → Contact points</strong></li>
									<li>Create a new contact point with type <strong>Webhook</strong></li>
									<li>Set the URL below and add header:<br/><code>Authorization: Bearer <em>your-token</em></code></li>
								</ol>
								<div class="webhook-requirement">
									<strong>Required:</strong> Add label <code>project: owner/repo</code> to your alert rules (e.g., <code>project: acme/api</code>)
								</div>
							</div>
							<div class="field">
								<label class="field-label" for="grafana-token">Bearer Token</label>
								<div class="token-field">
									<input
										id="grafana-token"
										type={showGrafanaToken ? 'text' : 'password'}
										bind:value={grafanaToken}
										placeholder={webhookSettings?.grafana.has_secret ? '••••••••' : 'Click Generate to create token'}
										class="field-input"
										readonly={!showGrafanaToken && !grafanaToken}
									/>
									<button class="btn btn-secondary btn-sm" onclick={generateGrafanaToken}>
										Generate
									</button>
									{#if grafanaToken}
										<button class="btn btn-ghost btn-sm" onclick={() => copyToClipboard(grafanaToken)} aria-label="Copy token">
											<svg viewBox="0 0 20 20" fill="currentColor" class="copy-icon">
												<path d="M8 2a1 1 0 000 2h2a1 1 0 100-2H8z"/>
												<path d="M3 5a2 2 0 012-2 3 3 0 003 3h2a3 3 0 003-3 2 2 0 012 2v6h-4.586l1.293-1.293a1 1 0 00-1.414-1.414l-3 3a1 1 0 000 1.414l3 3a1 1 0 001.414-1.414L10.414 13H15v3a2 2 0 01-2 2H5a2 2 0 01-2-2V5zM15 11h2a1 1 0 110 2h-2v-2z"/>
											</svg>
										</button>
									{/if}
								</div>
								{#if showGrafanaToken && grafanaToken}
									<span class="field-hint field-hint-warning">Copy this token and paste it in Grafana before saving</span>
								{/if}
							</div>
							<div class="webhook-url-field">
								<label class="field-label">Webhook URL</label>
								<div class="copyable-field">
									<code>{getWebhookUrl('/webhooks/grafana')}</code>
									<button class="btn btn-ghost btn-sm" onclick={() => copyToClipboard(getWebhookUrl('/webhooks/grafana'))} aria-label="Copy webhook URL">
										<svg viewBox="0 0 20 20" fill="currentColor" class="copy-icon">
											<path d="M8 2a1 1 0 000 2h2a1 1 0 100-2H8z"/>
											<path d="M3 5a2 2 0 012-2 3 3 0 003 3h2a3 3 0 003-3 2 2 0 012 2v6h-4.586l1.293-1.293a1 1 0 00-1.414-1.414l-3 3a1 1 0 000 1.414l3 3a1 1 0 001.414-1.414L10.414 13H15v3a2 2 0 01-2 2H5a2 2 0 01-2-2V5zM15 11h2a1 1 0 110 2h-2v-2z"/>
										</svg>
									</button>
								</div>
							</div>
						</div>
					{/if}
				</div>
			</div>

			<div class="webhook-actions">
				<button class="btn btn-primary" onclick={saveWebhookSettings} disabled={savingWebhooks}>
					{savingWebhooks ? 'Saving...' : 'Save Webhook Settings'}
				</button>
			</div>
		{/if}
	</section>

	<!-- Issue Tracker Integrations -->
	<section class="settings-section">
		<h2>Issue Tracker Integrations</h2>
		<p class="section-desc">Connect issue trackers to sync incident state to external systems</p>

		<div class="webhook-grid">
			<!-- Jira -->
			<div class="webhook-card">
				<div class="webhook-header">
					<div class="webhook-icon jira">
						<svg viewBox="0 0 24 24" fill="currentColor">
							<path d="M11.571 11.513H0a5.218 5.218 0 0 0 5.232 5.215h2.13v2.057A5.215 5.215 0 0 0 12.575 24V12.518a1.005 1.005 0 0 0-1.004-1.005zm5.723-5.756H5.736a5.215 5.215 0 0 0 5.212 5.214h2.13v2.058a5.218 5.218 0 0 0 5.215 5.214V6.758a1.001 1.001 0 0 0-1-1.001zM23.013 0H11.455a5.215 5.215 0 0 0 5.212 5.215h2.129v2.057A5.215 5.215 0 0 0 24 12.483V1.005A1.005 1.005 0 0 0 23.013 0z"/>
						</svg>
					</div>
					<div class="webhook-info">
						<h3>Jira</h3>
						<p>Create and update Jira issues for incidents</p>
					</div>
					{#if jiraStatus?.connected}
						<span class="status-badge connected compact">
							<svg viewBox="0 0 20 20" fill="currentColor">
								<path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
							</svg>
							Connected
						</span>
					{:else}
						<label class="toggle">
							<input type="checkbox" bind:checked={jiraExpanded} />
							<span class="toggle-slider"></span>
						</label>
					{/if}
				</div>
				{#if jiraStatus?.connected}
					<div class="webhook-config">
						<div class="connected-row">
							<div class="connected-details">
								<span class="connected-site">{jiraStatus.site_url}</span>
								<span class="connected-user">{jiraStatus.user_email || jiraStatus.connected_by}</span>
							</div>
							<button
								class="btn btn-ghost btn-sm disconnect"
								onclick={handleDisconnectJira}
								disabled={disconnectingJira}
							>
								{disconnectingJira ? 'Disconnecting...' : 'Disconnect'}
							</button>
						</div>
					</div>
				{:else if jiraExpanded}
					<div class="webhook-config">
						<div class="webhook-instructions">
							<h4>Connect Your Jira Account</h4>
							<p>Enter your Atlassian credentials to enable Jira integration.</p>
						</div>
						<div class="field">
							<label class="field-label" for="jira-site">Jira Site URL</label>
							<input
								id="jira-site"
								type="text"
								bind:value={jiraSiteUrl}
								placeholder="https://your-domain.atlassian.net"
								class="field-input"
							/>
						</div>
						<div class="field">
							<label class="field-label" for="jira-email">Email</label>
							<input
								id="jira-email"
								type="email"
								bind:value={jiraEmail}
								placeholder="you@company.com"
								class="field-input"
							/>
						</div>
						<div class="field">
							<label class="field-label" for="jira-token">API Token</label>
							<input
								id="jira-token"
								type="password"
								bind:value={jiraApiToken}
								placeholder="Your Jira API token"
								class="field-input"
							/>
							<span class="field-hint">
								Generate at <a href="https://id.atlassian.com/manage-profile/security/api-tokens" target="_blank" rel="noopener">Atlassian API Tokens</a>
							</span>
						</div>
						<div class="config-actions">
							<button
								class="btn btn-primary"
								onclick={handleConnectJira}
								disabled={connectingJira || !jiraSiteUrl || !jiraEmail || !jiraApiToken}
							>
								{connectingJira ? 'Connecting...' : 'Connect Jira'}
							</button>
						</div>
					</div>
				{/if}
			</div>

			<!-- Linear -->
			<div class="webhook-card">
				<div class="webhook-header">
					<div class="webhook-icon linear">
						<svg viewBox="0 0 24 24" fill="currentColor">
							<path d="M3.833 1.333c-.644 0-1.166.523-1.166 1.167v19c0 .644.522 1.167 1.166 1.167h16.334c.644 0 1.166-.523 1.166-1.167V2.5c0-.644-.522-1.167-1.166-1.167H3.833zm6.334 5.334a.833.833 0 1 0 0 1.666h7.5a.833.833 0 1 0 0-1.666h-7.5zm-3.334.833a.833.833 0 1 1 1.667 0 .833.833 0 0 1-1.667 0zm3.334 3.333a.833.833 0 1 0 0 1.667h7.5a.833.833 0 0 0 0-1.667h-7.5zm-3.334.834a.833.833 0 1 1 1.667 0 .833.833 0 0 1-1.667 0zm3.334 3.333a.833.833 0 0 0 0 1.667h7.5a.833.833 0 0 0 0-1.667h-7.5zm-3.334.833a.833.833 0 1 1 1.667 0 .833.833 0 0 1-1.667 0z"/>
						</svg>
					</div>
					<div class="webhook-info">
						<h3>Linear</h3>
						<p>Create and update Linear issues for incidents</p>
					</div>
					{#if linearStatus?.connected}
						<span class="status-badge connected compact">
							<svg viewBox="0 0 20 20" fill="currentColor">
								<path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
							</svg>
							Connected
						</span>
					{:else}
						<label class="toggle">
							<input type="checkbox" bind:checked={linearExpanded} />
							<span class="toggle-slider"></span>
						</label>
					{/if}
				</div>
				{#if linearStatus?.connected}
					<div class="webhook-config">
						<div class="connected-row">
							<div class="connected-details">
								<span class="connected-site">{linearStatus.organization_name || 'Linear'}</span>
								<span class="connected-user">{linearStatus.connected_by}</span>
							</div>
							<button
								class="btn btn-ghost btn-sm disconnect"
								onclick={handleDisconnectLinear}
								disabled={disconnectingLinear}
							>
								{disconnectingLinear ? 'Disconnecting...' : 'Disconnect'}
							</button>
						</div>
					</div>
				{:else if linearExpanded}
					<div class="webhook-config">
						<div class="webhook-instructions">
							<h4>Connect Your Linear Account</h4>
							<p>Enter your Linear API key to enable Linear integration.</p>
						</div>
						<div class="field">
							<label class="field-label" for="linear-token">API Key</label>
							<input
								id="linear-token"
								type="password"
								bind:value={linearApiKey}
								placeholder="lin_api_..."
								class="field-input"
							/>
							<span class="field-hint">
								Generate at <a href="https://linear.app/settings/api" target="_blank" rel="noopener">Linear API Settings</a>
							</span>
						</div>
						<div class="config-actions">
							<button
								class="btn btn-primary"
								onclick={handleConnectLinear}
								disabled={connectingLinear || !linearApiKey}
							>
								{connectingLinear ? 'Connecting...' : 'Connect Linear'}
							</button>
						</div>
					</div>
				{/if}
			</div>
		</div>

		<div class="tracker-info">
			<p><strong>Note:</strong> GitHub Issues are always available when GitHub is connected. Once connected here, enable Jira or Linear per-project in service settings.</p>
		</div>
	</section>

	<!-- LLM Configuration -->
	<section class="settings-section">
		<h2>LLM Configuration</h2>
		<p class="section-desc">Configure the AI models used for analyzing errors and generating fixes</p>

		<div class="llm-grid">
			<!-- Reader LLM -->
			<div class="llm-card">
				<div class="llm-header">
					<div class="llm-title">
						<h3>Reader (Analysis)</h3>
						<span class="llm-badge reader">Lower Cost</span>
					</div>
					<p>Used for triaging errors and gathering context. Can be a smaller, cheaper model.</p>
				</div>

				<div class="llm-form">
					<div class="field">
						<label class="field-label" for="reader-provider">Provider</label>
						<select
							id="reader-provider"
							bind:value={readerProvider}
							class="field-select"
							class:field-invalid={shouldShowError(readerTouched.provider, readerProviderValidation)}
							aria-invalid={shouldShowError(readerTouched.provider, readerProviderValidation)}
							aria-describedby={shouldShowError(readerTouched.provider, readerProviderValidation) ? 'reader-provider-error' : undefined}
							onblur={() => readerTouched.provider = true}
						>
							<option value="">Select provider...</option>
							{#each providers as p}
								<option value={p.value}>{p.label}</option>
							{/each}
						</select>
						{#if shouldShowError(readerTouched.provider, readerProviderValidation)}
							<span id="reader-provider-error" class="field-error" role="alert">
								{readerProviderValidation.error}
							</span>
						{/if}
					</div>

					{#if readerProvider === 'ollama'}
						<div class="field">
							<label class="field-label" for="reader-baseurl">Base URL</label>
							<input
								id="reader-baseurl"
								type="text"
								bind:value={readerBaseUrl}
								placeholder="http://localhost:11434"
								class="field-input"
								class:field-invalid={shouldShowError(readerTouched.baseUrl, readerBaseUrlValidation)}
								aria-invalid={shouldShowError(readerTouched.baseUrl, readerBaseUrlValidation)}
								aria-describedby={shouldShowError(readerTouched.baseUrl, readerBaseUrlValidation) ? 'reader-baseurl-error' : undefined}
								onblur={() => readerTouched.baseUrl = true}
							/>
							{#if shouldShowError(readerTouched.baseUrl, readerBaseUrlValidation)}
								<span id="reader-baseurl-error" class="field-error" role="alert">
									{readerBaseUrlValidation.error}
								</span>
							{/if}
						</div>
					{:else if readerProvider}
						<div class="field">
							<label class="field-label" for="reader-apikey">
								API Key
								{#if $llmConfig?.reader_provider === readerProvider}
									<span class="field-hint-inline">(already configured)</span>
								{/if}
							</label>
							<input
								id="reader-apikey"
								type="password"
								bind:value={readerApiKey}
								placeholder={$llmConfig?.reader_provider === readerProvider ? '••••••••' : 'Enter API key'}
								class="field-input"
								class:field-invalid={shouldShowError(readerTouched.apiKey, readerApiKeyValidation) && readerApiKeyRequired}
								aria-invalid={shouldShowError(readerTouched.apiKey, readerApiKeyValidation) && readerApiKeyRequired ? true : undefined}
								aria-describedby={shouldShowError(readerTouched.apiKey, readerApiKeyValidation) && readerApiKeyRequired ? 'reader-apikey-error' : undefined}
								onblur={() => readerTouched.apiKey = true}
							/>
							{#if shouldShowError(readerTouched.apiKey, readerApiKeyValidation) && readerApiKeyRequired}
								<span id="reader-apikey-error" class="field-error" role="alert">
									{readerApiKeyValidation.error}
								</span>
							{/if}
						</div>
					{/if}

					{#if readerProvider}
						<div class="field">
							<label class="field-label" for="reader-model">Model</label>
							<div class="model-input">
								<input
									id="reader-model"
									type="text"
									bind:value={readerModel}
									placeholder="e.g., claude-3-haiku-20240307"
									class="field-input"
									class:field-invalid={shouldShowError(readerTouched.model, readerModelValidation)}
									aria-invalid={shouldShowError(readerTouched.model, readerModelValidation)}
									aria-describedby={shouldShowError(readerTouched.model, readerModelValidation) ? 'reader-model-error' : undefined}
									onblur={() => readerTouched.model = true}
								/>
								<button
									class="btn btn-ghost"
									onclick={() => loadModels('reader')}
									disabled={loadingModels}
									aria-label="Load available models for reader LLM"
								>
									{loadingModels ? 'Loading...' : 'List Models'}
								</button>
							</div>
							{#if shouldShowError(readerTouched.model, readerModelValidation)}
								<span id="reader-model-error" class="field-error" role="alert">
									{readerModelValidation.error}
								</span>
							{/if}
						</div>
					{/if}

					{#if verifyResult && verifying === null}
						<div class="verify-result" class:success={verifyResult.success}>
							{verifyResult.success ? 'Connection verified!' : verifyResult.error}
						</div>
					{/if}

					{#if readerSaveSuccess}
						<div class="save-success" role="status">
							Configuration saved successfully!
						</div>
					{/if}

					<div class="llm-actions">
						<button
							class="btn btn-secondary"
							onclick={() => verifyLlm('reader')}
							disabled={!readerProvider || verifying === 'reader'}
						>
							{verifying === 'reader' ? 'Verifying...' : 'Verify'}
						</button>
						<button
							class="btn btn-primary"
							onclick={() => {
								touchAllReaderFields();
								if (readerFormValid) saveLlmConfig('reader');
							}}
							disabled={!readerFormValid || saving}
						>
							{saving ? 'Saving...' : 'Save Reader'}
						</button>
					</div>
				</div>
			</div>

			<!-- Writer LLM -->
			<div class="llm-card">
				<div class="llm-header">
					<div class="llm-title">
						<h3>Writer (Fix Generation)</h3>
						<span class="llm-badge writer">Higher Quality</span>
					</div>
					<p>Used for generating code fixes. Should be a capable coding model.</p>
				</div>

				<div class="llm-form">
					<div class="field">
						<label class="field-label" for="writer-provider">Provider</label>
						<select
							id="writer-provider"
							bind:value={writerProvider}
							class="field-select"
							class:field-invalid={shouldShowError(writerTouched.provider, writerProviderValidation)}
							aria-invalid={shouldShowError(writerTouched.provider, writerProviderValidation)}
							aria-describedby={shouldShowError(writerTouched.provider, writerProviderValidation) ? 'writer-provider-error' : undefined}
							onblur={() => writerTouched.provider = true}
						>
							<option value="">Select provider...</option>
							{#each providers as p}
								<option value={p.value}>{p.label}</option>
							{/each}
						</select>
						{#if shouldShowError(writerTouched.provider, writerProviderValidation)}
							<span id="writer-provider-error" class="field-error" role="alert">
								{writerProviderValidation.error}
							</span>
						{/if}
					</div>

					{#if writerProvider === 'ollama'}
						<div class="field">
							<label class="field-label" for="writer-baseurl">Base URL</label>
							<input
								id="writer-baseurl"
								type="text"
								bind:value={writerBaseUrl}
								placeholder="http://localhost:11434"
								class="field-input"
								class:field-invalid={shouldShowError(writerTouched.baseUrl, writerBaseUrlValidation)}
								aria-invalid={shouldShowError(writerTouched.baseUrl, writerBaseUrlValidation)}
								aria-describedby={shouldShowError(writerTouched.baseUrl, writerBaseUrlValidation) ? 'writer-baseurl-error' : undefined}
								onblur={() => writerTouched.baseUrl = true}
							/>
							{#if shouldShowError(writerTouched.baseUrl, writerBaseUrlValidation)}
								<span id="writer-baseurl-error" class="field-error" role="alert">
									{writerBaseUrlValidation.error}
								</span>
							{/if}
						</div>
					{:else if writerProvider}
						<div class="field">
							<label class="field-label" for="writer-apikey">
								API Key
								{#if $llmConfig?.writer_provider === writerProvider}
									<span class="field-hint-inline">(already configured)</span>
								{/if}
							</label>
							<input
								id="writer-apikey"
								type="password"
								bind:value={writerApiKey}
								placeholder={$llmConfig?.writer_provider === writerProvider ? '••••••••' : 'Enter API key'}
								class="field-input"
								class:field-invalid={shouldShowError(writerTouched.apiKey, writerApiKeyValidation) && writerApiKeyRequired}
								aria-invalid={shouldShowError(writerTouched.apiKey, writerApiKeyValidation) && writerApiKeyRequired ? true : undefined}
								aria-describedby={shouldShowError(writerTouched.apiKey, writerApiKeyValidation) && writerApiKeyRequired ? 'writer-apikey-error' : undefined}
								onblur={() => writerTouched.apiKey = true}
							/>
							{#if shouldShowError(writerTouched.apiKey, writerApiKeyValidation) && writerApiKeyRequired}
								<span id="writer-apikey-error" class="field-error" role="alert">
									{writerApiKeyValidation.error}
								</span>
							{/if}
						</div>
					{/if}

					{#if writerProvider}
						<div class="field">
							<label class="field-label" for="writer-model">Model</label>
							<div class="model-input">
								<input
									id="writer-model"
									type="text"
									bind:value={writerModel}
									placeholder="e.g., claude-3-5-sonnet-20241022"
									class="field-input"
									class:field-invalid={shouldShowError(writerTouched.model, writerModelValidation)}
									aria-invalid={shouldShowError(writerTouched.model, writerModelValidation)}
									aria-describedby={shouldShowError(writerTouched.model, writerModelValidation) ? 'writer-model-error' : undefined}
									onblur={() => writerTouched.model = true}
								/>
								<button
									class="btn btn-ghost"
									onclick={() => loadModels('writer')}
									disabled={loadingModels}
									aria-label="Load available models for writer LLM"
								>
									{loadingModels ? 'Loading...' : 'List Models'}
								</button>
							</div>
							{#if shouldShowError(writerTouched.model, writerModelValidation)}
								<span id="writer-model-error" class="field-error" role="alert">
									{writerModelValidation.error}
								</span>
							{/if}
						</div>
					{/if}

					{#if writerSaveSuccess}
						<div class="save-success" role="status">
							Configuration saved successfully!
						</div>
					{/if}

					<div class="llm-actions">
						<button
							class="btn btn-secondary"
							onclick={() => verifyLlm('writer')}
							disabled={!writerProvider || verifying === 'writer'}
						>
							{verifying === 'writer' ? 'Verifying...' : 'Verify'}
						</button>
						<button
							class="btn btn-primary"
							onclick={() => {
								touchAllWriterFields();
								if (writerFormValid) saveLlmConfig('writer');
							}}
							disabled={!writerFormValid || saving}
						>
							{saving ? 'Saving...' : 'Save Writer'}
						</button>
					</div>
				</div>
			</div>

			<!-- Internist LLM -->
			<div class="llm-card">
				<div class="llm-header">
					<div class="llm-title">
						<h3>Internist (Pattern Analysis)</h3>
						<span class="llm-badge internist">High Reasoning</span>
					</div>
					<p>Used for analyzing patterns across incidents and recommending architectural fixes. Optional but enables the Internal Medicine feature.</p>
				</div>

				<div class="llm-form">
					<div class="field">
						<label class="field-label" for="internist-provider">Provider</label>
						<select
							id="internist-provider"
							bind:value={internistProvider}
							class="field-select"
							class:field-invalid={shouldShowError(internistTouched.provider, internistProviderValidation)}
							aria-invalid={shouldShowError(internistTouched.provider, internistProviderValidation)}
							aria-describedby={shouldShowError(internistTouched.provider, internistProviderValidation) ? 'internist-provider-error' : undefined}
							onblur={() => internistTouched.provider = true}
						>
							<option value="">Select provider...</option>
							{#each providers as p}
								<option value={p.value}>{p.label}</option>
							{/each}
						</select>
						{#if shouldShowError(internistTouched.provider, internistProviderValidation)}
							<span id="internist-provider-error" class="field-error" role="alert">
								{internistProviderValidation.error}
							</span>
						{/if}
					</div>

					{#if internistProvider}
						{#if internistProvider === 'ollama'}
							<div class="field">
								<label class="field-label" for="internist-base-url">Base URL</label>
								<input
									id="internist-base-url"
									type="url"
									bind:value={internistBaseUrl}
									placeholder="http://localhost:11434"
									class="field-input"
									class:field-invalid={shouldShowError(internistTouched.baseUrl, internistBaseUrlValidation)}
									aria-invalid={shouldShowError(internistTouched.baseUrl, internistBaseUrlValidation)}
									aria-describedby={shouldShowError(internistTouched.baseUrl, internistBaseUrlValidation) ? 'internist-url-error' : undefined}
									onblur={() => internistTouched.baseUrl = true}
								/>
								{#if shouldShowError(internistTouched.baseUrl, internistBaseUrlValidation)}
									<span id="internist-url-error" class="field-error" role="alert">
										{internistBaseUrlValidation.error}
									</span>
								{/if}
							</div>
						{:else}
							<div class="field">
								<label class="field-label" for="internist-api-key">
									API Key
									{#if $llmConfig?.internist_provider === internistProvider}
										<span class="key-stored">(stored)</span>
									{/if}
								</label>
								<input
									id="internist-api-key"
									type="password"
									bind:value={internistApiKey}
									placeholder={$llmConfig?.internist_provider === internistProvider ? '••••••••' : 'Enter API key'}
									class="field-input"
									class:field-invalid={shouldShowError(internistTouched.apiKey, internistApiKeyValidation) && internistApiKeyRequired}
									aria-invalid={shouldShowError(internistTouched.apiKey, internistApiKeyValidation) && internistApiKeyRequired}
									aria-describedby={shouldShowError(internistTouched.apiKey, internistApiKeyValidation) && internistApiKeyRequired ? 'internist-key-error' : undefined}
									onblur={() => internistTouched.apiKey = true}
								/>
								{#if shouldShowError(internistTouched.apiKey, internistApiKeyValidation) && internistApiKeyRequired}
									<span id="internist-key-error" class="field-error" role="alert">
										{internistApiKeyValidation.error}
									</span>
								{/if}
							</div>
						{/if}

						<div class="field">
							<label class="field-label" for="internist-model">Model</label>
							<div class="model-input">
								<input
									id="internist-model"
									type="text"
									bind:value={internistModel}
									placeholder="e.g., claude-3-5-sonnet-20241022"
									class="field-input"
									class:field-invalid={shouldShowError(internistTouched.model, internistModelValidation)}
									aria-invalid={shouldShowError(internistTouched.model, internistModelValidation)}
									aria-describedby={shouldShowError(internistTouched.model, internistModelValidation) ? 'internist-model-error' : undefined}
									onblur={() => internistTouched.model = true}
								/>
								<button
									class="btn btn-ghost"
									onclick={() => loadModels('internist')}
									disabled={loadingModels}
									aria-label="Load available models for internist LLM"
								>
									{loadingModels ? 'Loading...' : 'List Models'}
								</button>
							</div>
							{#if shouldShowError(internistTouched.model, internistModelValidation)}
								<span id="internist-model-error" class="field-error" role="alert">
									{internistModelValidation.error}
								</span>
							{/if}
						</div>
					{/if}

					{#if internistSaveSuccess}
						<div class="save-success" role="status">
							Configuration saved successfully!
						</div>
					{/if}

					<div class="llm-actions">
						<button
							class="btn btn-secondary"
							onclick={() => verifyLlm('internist')}
							disabled={!internistProvider || verifying === 'internist'}
						>
							{verifying === 'internist' ? 'Verifying...' : 'Verify'}
						</button>
						<button
							class="btn btn-primary"
							onclick={() => {
								touchAllInternistFields();
								if (internistFormValid) saveLlmConfig('internist');
							}}
							disabled={!internistFormValid || saving}
						>
							{saving ? 'Saving...' : 'Save Internist'}
						</button>
					</div>
				</div>
			</div>
		</div>

		<!-- Available Models -->
		{#if availableModels.length > 0}
			<div class="models-list">
				<h4>Available Models</h4>
				<div class="models-grid">
					{#each availableModels as model}
						<button
							class="model-chip"
							onclick={() => {
								// Set model based on which provider matches
								if (readerProvider && model.id) readerModel = model.id;
								if (writerProvider && model.id) writerModel = model.id;
								if (internistProvider && model.id) internistModel = model.id;
							}}
						>
							{model.name || model.id}
						</button>
					{/each}
				</div>
			</div>
		{/if}
	</section>
</div>

<style>
	.settings-page {
		max-width: 1000px;
	}

	.page-header {
		margin-bottom: var(--space-xl);
	}

	.page-header h1 {
		font-size: 1.75rem;
		margin-bottom: var(--space-xs);
	}

	.page-subtitle {
		color: var(--graphite);
	}

	/* Sections */
	.settings-section {
		margin-bottom: var(--space-2xl);
	}

	.settings-section h2 {
		font-family: var(--font-body);
		font-size: 1.125rem;
		font-weight: 600;
		margin-bottom: var(--space-xs);
	}

	.section-desc {
		font-size: 0.875rem;
		color: var(--graphite);
		margin-bottom: var(--space-lg);
	}

	/* Connection Cards */
	.connection-grid {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.connection-card {
		display: flex;
		flex-direction: column;
		padding: var(--space-lg);
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
	}

	.connection-main {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
	}

	.connection-card:not(.slack-card) {
		flex-direction: row;
		align-items: center;
		gap: var(--space-lg);
	}

	.connection-icon {
		width: 48px;
		height: 48px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-md);
		flex-shrink: 0;
	}

	.connection-icon.github {
		background: var(--obsidian);
		color: white;
	}

	.connection-icon.slack {
		background: #4A154B;
		color: white;
	}

	.connection-icon svg {
		width: 24px;
		height: 24px;
	}

	.connection-info {
		flex: 1;
	}

	.connection-info h3 {
		font-family: var(--font-body);
		font-size: 1rem;
		font-weight: 600;
		margin-bottom: var(--space-xs);
	}

	.connection-info p {
		font-size: 0.8125rem;
		color: var(--graphite);
	}

	.status-badge {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-full);
		font-size: 0.8125rem;
		font-weight: 500;
	}

	.status-badge.connected {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.status-badge.disabled {
		background: var(--paper);
		color: var(--slate);
	}

	.status-badge svg {
		width: 14px;
		height: 14px;
	}

	/* Slack Config */
	.slack-config {
		margin-top: var(--space-lg);
		padding-top: var(--space-lg);
		border-top: 1px solid var(--paper);
	}

	.channel-selector {
		display: flex;
		align-items: center;
		gap: var(--space-md);
	}

	.channel-label {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--charcoal);
		min-width: 140px;
	}

	.channel-controls {
		display: flex;
		gap: var(--space-sm);
		flex: 1;
	}

	.channel-select {
		flex: 1;
		max-width: 300px;
	}

	.loading-text {
		font-size: 0.875rem;
		color: var(--slate);
	}

	.slack-actions {
		display: flex;
		gap: var(--space-sm);
		margin-top: var(--space-md);
	}

	.slack-actions .disconnect {
		color: var(--critical);
	}

	.slack-actions .disconnect:hover {
		background: var(--critical-soft);
	}

	.slack-message {
		margin-top: var(--space-sm);
		font-size: 0.8125rem;
		color: var(--graphite);
	}

	/* LLM Cards */
	.llm-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--space-lg);
	}

	.llm-card {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-lg);
	}

	.llm-header {
		margin-bottom: var(--space-lg);
		padding-bottom: var(--space-lg);
		border-bottom: 1px solid var(--paper);
	}

	.llm-title {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin-bottom: var(--space-sm);
	}

	.llm-title h3 {
		font-family: var(--font-body);
		font-size: 1rem;
		font-weight: 600;
	}

	.llm-badge {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		padding: var(--space-xs) var(--space-sm);
		border-radius: var(--radius-sm);
	}

	.llm-badge.reader {
		background: var(--vitals-soft);
		color: #1D4ED8;
	}

	.llm-badge.writer {
		background: var(--stable-soft);
		color: #6D28D9;
	}

	.llm-badge.internist {
		background: var(--alert-soft);
		color: #B45309;
	}

	.llm-header p {
		font-size: 0.8125rem;
		color: var(--graphite);
	}

	.llm-form {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.field-label {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--charcoal);
	}

	.field-input,
	.field-select {
		width: 100%;
		padding: var(--space-sm) var(--space-md);
		border: 1px solid var(--silver);
		border-radius: var(--radius-md);
		font-size: 0.9375rem;
		background: var(--white);
	}

	.model-input {
		display: flex;
		gap: var(--space-sm);
	}

	.model-input .field-input {
		flex: 1;
	}

	.verify-result {
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-md);
		font-size: 0.8125rem;
		background: var(--critical-soft);
		color: #B91C1C;
	}

	.verify-result.success {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.llm-actions {
		display: flex;
		gap: var(--space-sm);
		justify-content: flex-end;
		margin-top: var(--space-sm);
	}

	/* Models List */
	.models-list {
		margin-top: var(--space-lg);
		padding: var(--space-lg);
		background: var(--paper);
		border-radius: var(--radius-lg);
	}

	.models-list h4 {
		font-family: var(--font-body);
		font-size: 0.875rem;
		font-weight: 600;
		margin-bottom: var(--space-md);
	}

	.models-grid {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
	}

	.model-chip {
		padding: var(--space-xs) var(--space-sm);
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-sm);
		font-size: 0.8125rem;
		font-family: 'SF Mono', monospace;
		cursor: pointer;
		transition: all var(--transition-fast);
	}

	.model-chip:hover {
		border-color: var(--scrubs);
		background: var(--scrubs-soft);
	}

	@media (max-width: 768px) {
		.settings-page {
			padding: 0;
		}

		.page-header h1 {
			font-size: 1.5rem;
		}

		.llm-grid {
			grid-template-columns: 1fr;
		}

		.llm-card {
			padding: var(--space-md);
		}

		.llm-header {
			padding-bottom: var(--space-md);
			margin-bottom: var(--space-md);
		}

		.connection-card {
			padding: var(--space-md);
		}

		.connection-main {
			flex-direction: column;
			text-align: center;
			gap: var(--space-md);
		}

		.connection-icon {
			width: 56px;
			height: 56px;
		}

		.connection-icon svg {
			width: 28px;
			height: 28px;
		}

		.connection-status .btn {
			min-height: 44px;
			width: 100%;
		}

		.channel-selector {
			flex-direction: column;
			align-items: flex-start;
			gap: var(--space-sm);
		}

		.channel-label {
			min-width: unset;
		}

		.channel-controls {
			width: 100%;
			flex-direction: column;
		}

		.channel-select {
			max-width: none;
			min-height: 44px;
		}

		.channel-controls .btn {
			min-height: 44px;
		}

		.slack-actions {
			flex-direction: column;
		}

		.slack-actions .btn {
			min-height: 44px;
		}

		/* Form fields */
		.field-input,
		.field-select {
			min-height: 44px;
		}

		.model-input {
			flex-direction: column;
		}

		.model-input .btn {
			min-height: 44px;
		}

		.llm-actions {
			flex-direction: column;
		}

		.llm-actions .btn {
			width: 100%;
			min-height: 44px;
		}

		/* Models list */
		.models-list {
			padding: var(--space-md);
		}

		.model-chip {
			padding: var(--space-sm) var(--space-md);
			min-height: 44px;
			display: flex;
			align-items: center;
		}
	}

	@media (max-width: 480px) {
		.page-header h1 {
			font-size: 1.25rem;
		}

		.settings-section h2 {
			font-size: 1rem;
		}

		.llm-title {
			flex-direction: column;
			align-items: flex-start;
			gap: var(--space-xs);
		}
	}

	/* Validation Styles */
	.field-invalid {
		border-color: var(--critical) !important;
		background-color: var(--critical-soft) !important;
	}

	.field-invalid:focus {
		box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.2) !important;
		border-color: var(--critical) !important;
	}

	.field-error {
		display: block;
		font-size: 0.75rem;
		color: var(--critical);
		margin-top: var(--space-xs);
		line-height: 1.4;
	}

	.field-hint-inline {
		font-weight: 400;
		font-size: 0.75rem;
		color: var(--slate);
		margin-left: var(--space-xs);
	}

	.save-success {
		padding: var(--space-sm) var(--space-md);
		border-radius: var(--radius-md);
		font-size: 0.8125rem;
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
		animation: fadeIn var(--transition-fast) ease-out;
	}

	@keyframes fadeIn {
		from { opacity: 0; transform: translateY(-4px); }
		to { opacity: 1; transform: translateY(0); }
	}

	/* Disabled button styling */
	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.btn-primary:disabled {
		background: var(--slate);
	}

	.btn-primary:disabled:hover {
		background: var(--slate);
	}

	/* Webhook Integrations */
	.loading-state {
		padding: var(--space-lg);
		text-align: center;
		color: var(--slate);
		font-size: 0.875rem;
	}

	.webhook-grid {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.webhook-card {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		overflow: hidden;
	}

	.webhook-header {
		display: flex;
		align-items: center;
		gap: var(--space-lg);
		padding: var(--space-lg);
	}

	.webhook-icon {
		width: 48px;
		height: 48px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-md);
		flex-shrink: 0;
	}

	.webhook-icon svg {
		width: 24px;
		height: 24px;
	}

	.webhook-icon.sentry {
		background: #362D59;
		color: white;
	}

	.webhook-icon.datadog {
		background: #632CA6;
		color: white;
	}

	.webhook-icon.grafana {
		background: #F46800;
		color: white;
	}

	.webhook-info {
		flex: 1;
	}

	.webhook-info h3 {
		font-family: var(--font-body);
		font-size: 1rem;
		font-weight: 600;
		margin-bottom: var(--space-xs);
	}

	.webhook-info p {
		font-size: 0.8125rem;
		color: var(--graphite);
	}

	.webhook-config {
		padding: var(--space-lg);
		padding-top: 0;
	}

	.webhook-config .field {
		margin-bottom: var(--space-md);
	}

	.field-hint {
		font-size: 0.75rem;
		color: var(--slate);
		margin-top: var(--space-xs);
	}

	.webhook-url {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-sm) var(--space-md);
		background: var(--paper);
		border-radius: var(--radius-md);
		font-size: 0.8125rem;
	}

	.webhook-url .label {
		color: var(--slate);
	}

	.webhook-url code {
		font-family: 'SF Mono', monospace;
		color: var(--charcoal);
	}

	.webhook-actions {
		margin-top: var(--space-lg);
		display: flex;
		justify-content: flex-end;
	}

	/* Webhook Instructions */
	.webhook-instructions {
		background: var(--paper);
		border-radius: var(--radius-md);
		padding: var(--space-md);
		margin-bottom: var(--space-lg);
	}

	.webhook-instructions h4 {
		font-family: var(--font-body);
		font-size: 0.8125rem;
		font-weight: 600;
		margin-bottom: var(--space-sm);
		color: var(--charcoal);
	}

	.webhook-instructions ol {
		margin: 0;
		padding-left: var(--space-lg);
		font-size: 0.8125rem;
		color: var(--graphite);
		line-height: 1.6;
	}

	.webhook-instructions li {
		margin-bottom: var(--space-xs);
	}

	.webhook-instructions code {
		font-family: 'SF Mono', monospace;
		font-size: 0.75rem;
		background: var(--white);
		padding: 2px 6px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--silver);
	}

	.webhook-instructions em {
		color: var(--slate);
		font-style: normal;
	}

	.webhook-requirement {
		margin-top: var(--space-md);
		padding: var(--space-sm) var(--space-md);
		background: var(--warning-soft, #FEF3C7);
		border-radius: var(--radius-sm);
		font-size: 0.8125rem;
		color: var(--warning-dark, #92400E);
		border-left: 3px solid var(--warning, #F59E0B);
	}

	.webhook-requirement code {
		background: rgba(255,255,255,0.5);
		border-color: var(--warning, #F59E0B);
	}

	/* Token Field */
	.token-field {
		display: flex;
		gap: var(--space-sm);
	}

	.token-field .field-input {
		flex: 1;
		font-family: 'SF Mono', monospace;
		font-size: 0.875rem;
	}

	/* Copyable Field */
	.webhook-url-field {
		margin-top: var(--space-md);
	}

	.copyable-field {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		padding: var(--space-sm) var(--space-md);
		background: var(--paper);
		border-radius: var(--radius-md);
		border: 1px solid var(--silver);
	}

	.copyable-field code {
		flex: 1;
		font-family: 'SF Mono', monospace;
		font-size: 0.8125rem;
		color: var(--charcoal);
		word-break: break-all;
	}

	.copy-icon {
		width: 16px;
		height: 16px;
	}

	/* Small button variant */
	.btn-sm {
		padding: var(--space-xs) var(--space-sm);
		font-size: 0.8125rem;
	}

	/* Warning hint */
	.field-hint-warning {
		color: var(--warning-dark, #B45309);
		font-weight: 500;
	}

	/* Toggle Switch */
	.toggle {
		position: relative;
		display: inline-block;
		width: 48px;
		height: 28px;
		flex-shrink: 0;
	}

	.toggle input {
		opacity: 0;
		width: 0;
		height: 0;
	}

	.toggle-slider {
		position: absolute;
		cursor: pointer;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background-color: var(--silver);
		transition: var(--transition-fast);
		border-radius: var(--radius-full);
	}

	.toggle-slider:before {
		position: absolute;
		content: "";
		height: 22px;
		width: 22px;
		left: 3px;
		bottom: 3px;
		background-color: white;
		transition: var(--transition-fast);
		border-radius: 50%;
		box-shadow: 0 2px 4px rgba(0, 0, 0, 0.15);
	}

	.toggle input:checked + .toggle-slider {
		background-color: var(--scrubs);
	}

	.toggle input:checked + .toggle-slider:before {
		transform: translateX(20px);
	}

	.toggle input:focus + .toggle-slider {
		box-shadow: 0 0 0 3px var(--scrubs-soft);
	}

	@media (max-width: 768px) {
		.webhook-header {
			flex-wrap: wrap;
			gap: var(--space-md);
		}

		.webhook-info {
			flex: 1 1 calc(100% - 120px);
		}

		.webhook-config {
			padding: var(--space-md);
			padding-top: 0;
		}

		.webhook-actions {
			flex-direction: column;
		}

		.webhook-actions .btn {
			width: 100%;
			min-height: 44px;
		}
	}

	/* Issue Tracker Integrations */
	.webhook-icon.jira {
		background: #0052CC;
		color: white;
	}

	.webhook-icon.linear {
		background: #5E6AD2;
		color: white;
	}

	.status-badge.compact {
		padding: var(--space-xs) var(--space-sm);
		font-size: 0.75rem;
	}

	.connected-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-sm) 0;
	}

	.connected-details {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.connected-site {
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--charcoal);
	}

	.connected-user {
		font-size: 0.75rem;
		color: var(--slate);
	}

	.config-actions {
		display: flex;
		gap: var(--space-sm);
		margin-top: var(--space-sm);
	}

	.tracker-info {
		margin-top: var(--space-lg);
		padding: var(--space-md);
		background: var(--vitals-soft);
		border-radius: var(--radius-md);
		font-size: 0.8125rem;
		color: var(--charcoal);
	}

	.tracker-info p {
		margin: 0;
	}

	.btn.disconnect {
		color: var(--critical);
	}

	.btn.disconnect:hover {
		background: var(--critical-soft);
	}
</style>
