<script lang="ts">
	import { onMount } from 'svelte';
	import { auth, llmConfig } from '$lib/stores';
	import * as api from '$lib/api';

	// LLM Configuration
	let readerProvider = $state('');
	let readerModel = $state('');
	let readerApiKey = $state('');
	let readerBaseUrl = $state('');

	let writerProvider = $state('');
	let writerModel = $state('');
	let writerApiKey = $state('');
	let writerBaseUrl = $state('');

	let saving = $state(false);
	let verifying = $state<'reader' | 'writer' | null>(null);
	let verifyResult = $state<{ success: boolean; error?: string } | null>(null);
	let availableModels = $state<api.LlmModel[]>([]);
	let loadingModels = $state(false);

	// Slack state
	let slackChannels = $state<api.SlackChannel[]>([]);
	let loadingChannels = $state(false);
	let selectedChannel = $state('');
	let slackMessage = $state('');
	let testingSlack = $state(false);
	let disconnectingSlack = $state(false);

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
		}

		// Load Slack channels if connected
		if ($auth?.slack?.connected) {
			await loadSlackChannels();
		}
	});

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
		} catch (e) {
			slackMessage = (e as Error).message;
		}
	}

	async function testSlack() {
		testingSlack = true;
		slackMessage = '';
		try {
			const result = await api.testSlackConnection();
			slackMessage = result.message;
		} catch (e) {
			slackMessage = (e as Error).message;
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
		} catch (e) {
			console.error('Failed to disconnect Slack:', e);
		} finally {
			disconnectingSlack = false;
		}
	}

	async function verifyLlm(role: 'reader' | 'writer') {
		verifying = role;
		verifyResult = null;

		const config = role === 'reader'
			? { provider: readerProvider, api_key: readerApiKey || undefined, model: readerModel, base_url: readerBaseUrl || undefined }
			: { provider: writerProvider, api_key: writerApiKey || undefined, model: writerModel, base_url: writerBaseUrl || undefined };

		try {
			verifyResult = await api.verifyLlm(config);
		} catch (e) {
			verifyResult = { success: false, error: (e as Error).message };
		} finally {
			verifying = null;
		}
	}

	async function loadModels(role: 'reader' | 'writer') {
		loadingModels = true;
		const config = role === 'reader'
			? { provider: readerProvider, api_key: readerApiKey || undefined, base_url: readerBaseUrl || undefined }
			: { provider: writerProvider, api_key: writerApiKey || undefined, base_url: writerBaseUrl || undefined };

		try {
			availableModels = await api.listLlmModels(config);
		} catch (e) {
			console.error('Failed to load models:', e);
			availableModels = [];
		} finally {
			loadingModels = false;
		}
	}

	async function saveLlmConfig(role: 'reader' | 'writer') {
		saving = true;
		const config = role === 'reader'
			? { provider: readerProvider, api_key: readerApiKey || undefined, model: readerModel, base_url: readerBaseUrl || undefined, role: 'reader' as const }
			: { provider: writerProvider, api_key: writerApiKey || undefined, model: writerModel, base_url: writerBaseUrl || undefined, role: 'writer' as const };

		try {
			await llmConfig.update(config);
			// Clear API key field after save (it's stored securely)
			if (role === 'reader') readerApiKey = '';
			else writerApiKey = '';
		} catch (e) {
			console.error('Failed to save LLM config:', e);
		} finally {
			saving = false;
		}
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
					{#if $auth?.github?.connected}
						<span class="status-badge connected">
							<svg viewBox="0 0 20 20" fill="currentColor">
								<path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
							</svg>
							Connected as {$auth.github.user?.login}
						</span>
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
						{:else if $auth?.slack?.enabled}
							<a href={api.getSlackAuthUrl()} class="btn btn-secondary">
								Connect Slack
							</a>
						{:else}
							<span class="status-badge disabled">Not Enabled</span>
						{/if}
					</div>
				</div>

				<!-- Slack Channel Selection -->
				{#if $auth?.slack?.connected}
					<div class="slack-config">
						<div class="channel-selector">
							<span class="channel-label">Notification Channel</span>
							<div class="channel-controls">
								{#if loadingChannels}
									<span class="loading-text">Loading channels...</span>
								{:else}
									<select bind:value={selectedChannel} class="channel-select">
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
								>
									{testingSlack ? 'Sending...' : 'Send Test Message'}
								</button>
								<button
									class="btn btn-ghost disconnect"
									onclick={disconnectSlack}
									disabled={disconnectingSlack}
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
						<select id="reader-provider" bind:value={readerProvider} class="field-select">
							<option value="">Select provider...</option>
							{#each providers as p}
								<option value={p.value}>{p.label}</option>
							{/each}
						</select>
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
							/>
						</div>
					{:else if readerProvider}
						<div class="field">
							<label class="field-label" for="reader-apikey">API Key</label>
							<input
								id="reader-apikey"
								type="password"
								bind:value={readerApiKey}
								placeholder={$llmConfig?.reader_provider === readerProvider ? '••••••••' : 'Enter API key'}
								class="field-input"
							/>
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
								/>
								<button
									class="btn btn-ghost"
									onclick={() => loadModels('reader')}
									disabled={loadingModels}
								>
									{loadingModels ? 'Loading...' : 'List Models'}
								</button>
							</div>
						</div>
					{/if}

					{#if verifyResult && verifying === null}
						<div class="verify-result" class:success={verifyResult.success}>
							{verifyResult.success ? 'Connection verified!' : verifyResult.error}
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
							onclick={() => saveLlmConfig('reader')}
							disabled={!readerProvider || !readerModel || saving}
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
						<select id="writer-provider" bind:value={writerProvider} class="field-select">
							<option value="">Select provider...</option>
							{#each providers as p}
								<option value={p.value}>{p.label}</option>
							{/each}
						</select>
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
							/>
						</div>
					{:else if writerProvider}
						<div class="field">
							<label class="field-label" for="writer-apikey">API Key</label>
							<input
								id="writer-apikey"
								type="password"
								bind:value={writerApiKey}
								placeholder={$llmConfig?.writer_provider === writerProvider ? '••••••••' : 'Enter API key'}
								class="field-input"
							/>
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
								/>
								<button
									class="btn btn-ghost"
									onclick={() => loadModels('writer')}
									disabled={loadingModels}
								>
									{loadingModels ? 'Loading...' : 'List Models'}
								</button>
							</div>
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
							onclick={() => saveLlmConfig('writer')}
							disabled={!writerProvider || !writerModel || saving}
						>
							{saving ? 'Saving...' : 'Save Writer'}
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
		.llm-grid {
			grid-template-columns: 1fr;
		}

		.connection-main {
			flex-direction: column;
			text-align: center;
		}

		.channel-selector {
			flex-direction: column;
			align-items: flex-start;
		}

		.channel-controls {
			width: 100%;
		}
	}
</style>
