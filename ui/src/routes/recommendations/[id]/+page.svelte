<script lang="ts">
	import { page } from '$app/stores';
	import { base } from '$app/paths';
	import { onMount, tick } from 'svelte';
	import { recommendations, toasts, demoMode } from '$lib/stores';
	import { getMockRecommendations } from '$lib/mockData';
	import {
		getRecommendation,
		sendInternistMessage,
		getInternistChatHistory,
		applyInternistSuggestion,
		convertToTriageTicket,
		type Recommendation,
		type InternistChatMessage,
		type InternistChatResponse
	} from '$lib/api';
	import { get } from 'svelte/store';

	let recommendation = $state<Recommendation | null>(null);
	let chatMessages = $state<InternistChatMessage[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let chatInput = $state('');
	let sendingMessage = $state(false);
	let chatContainer: HTMLElement;

	// Action loading states
	let approving = $state(false);
	let rejecting = $state(false);
	let checkingConflicts = $state(false);
	let rebasing = $state(false);
	let implementing = $state(false);
	let convertingToTicket = $state(false);
	let applyingSuggestion = $state<string | null>(null);

	const id = $derived($page.params.id);

	onMount(async () => {
		await loadRecommendation();
	});

	async function loadRecommendation() {
		loading = true;
		error = null;
		try {
			if (get(demoMode)) {
				const mockRecs = getMockRecommendations();
				recommendation = mockRecs.find(r => r.id === id) || null;
				// Mock initial chat with context
				chatMessages = recommendation ? [
					{
						id: 'msg-system',
						role: 'assistant',
						content: `I've analyzed this recommendation in detail. Here's what I found:\n\n**Root Cause:** ${recommendation.analysis?.root_cause || 'Multiple related incidents suggest a systemic issue.'}\n\n**Impact:** ${recommendation.analysis?.impact_assessment || 'This affects error handling across the codebase.'}\n\nI'm ready to discuss, refine, or completely rewrite this recommendation based on your feedback. What would you like to explore?`,
						timestamp: new Date().toISOString()
					}
				] : [];
			} else {
				recommendation = await getRecommendation(id);
				chatMessages = await getInternistChatHistory(id);
				// If no chat history, add a welcome message
				if (chatMessages.length === 0 && recommendation) {
					chatMessages = [{
						id: 'msg-welcome',
						role: 'assistant',
						content: `I've analyzed this recommendation. Let me know if you'd like me to:\n\n• **Explain** the reasoning in more detail\n• **Refine** specific parts of the analysis\n• **Rewrite** the entire recommendation\n• **Discuss** implementation considerations\n\nWhat would you like to explore?`,
						timestamp: new Date().toISOString()
					}];
				}
			}
		} catch (e) {
			console.error('Failed to load recommendation:', e);
			error = (e as Error).message;
		} finally {
			loading = false;
		}
	}

	async function handleApprove() {
		if (!recommendation) return;
		if (get(demoMode)) {
			toasts.success('Recommendation approved and triage ticket created (demo mode)');
			recommendation.status = 'implementing';
			recommendation.approved_at = new Date().toISOString();
			// Add confirmation message to chat
			chatMessages = [...chatMessages, {
				id: `msg-approved-${Date.now()}`,
				role: 'assistant',
				content: '**Recommendation Approved!**\n\nI\'ve created a detailed triage ticket with:\n\n• Comprehensive error context from the original incidents\n• Clear description of the fix approach\n• Specific files to modify\n• Expected behavior after the fix\n• Test cases to verify the solution\n\nThe Writer LLM will now implement the fix. You can track progress on the Surgery Board.',
				timestamp: new Date().toISOString()
			}];
			return;
		}
		approving = true;
		try {
			// First approve the recommendation
			await recommendations.approve(recommendation.id);
			// Then convert it to a triage ticket
			const result = await convertToTriageTicket(recommendation.id);
			toasts.success(`Triage ticket created: ${result.incident_id}`);
			await loadRecommendation();
		} catch (e) {
			toasts.error(`Failed to approve: ${(e as Error).message}`);
		} finally {
			approving = false;
		}
	}

	async function handleReject() {
		if (!recommendation) return;
		const reason = prompt('Reason for rejection (optional):');
		if (get(demoMode)) {
			toasts.success('Recommendation rejected (demo mode)');
			recommendation.status = 'rejected';
			return;
		}
		rejecting = true;
		try {
			await recommendations.reject(recommendation.id, reason || undefined);
			await loadRecommendation();
			toasts.success('Recommendation rejected');
		} catch (e) {
			toasts.error(`Failed to reject: ${(e as Error).message}`);
		} finally {
			rejecting = false;
		}
	}

	async function handleCheckConflicts() {
		if (!recommendation) return;
		if (get(demoMode)) {
			toasts.info('Conflict check completed (demo mode)');
			return;
		}
		checkingConflicts = true;
		try {
			const result = await recommendations.checkConflicts(recommendation.id);
			if (result.has_conflicts) {
				toasts.warning(`Found ${result.conflicting_files.length} conflicting files`);
			} else {
				toasts.success('No conflicts found');
			}
			await loadRecommendation();
		} catch (e) {
			toasts.error(`Failed to check conflicts: ${(e as Error).message}`);
		} finally {
			checkingConflicts = false;
		}
	}

	async function handleRebase() {
		if (!recommendation) return;
		if (get(demoMode)) {
			toasts.success('Rebase completed (demo mode)');
			recommendation.has_conflicts = false;
			return;
		}
		rebasing = true;
		try {
			const result = await recommendations.rebase(recommendation.id);
			if (result.success) {
				toasts.success('Rebase successful');
			} else {
				toasts.warning(`Partial rebase: ${result.remaining_conflicts.length} conflicts remain`);
			}
			await loadRecommendation();
		} catch (e) {
			toasts.error(`Failed to rebase: ${(e as Error).message}`);
		} finally {
			rebasing = false;
		}
	}

	async function handleImplement() {
		if (!recommendation) return;
		if (get(demoMode)) {
			toasts.success('Implementation PR created (demo mode)');
			recommendation.status = 'implementing';
			recommendation.pr_url = 'https://github.com/acme/api/pull/100';
			recommendation.pr_number = 100;
			return;
		}
		implementing = true;
		try {
			await recommendations.implement(recommendation.id);
			await loadRecommendation();
			toasts.success('Implementation PR created');
		} catch (e) {
			toasts.error(`Failed to implement: ${(e as Error).message}`);
		} finally {
			implementing = false;
		}
	}

	async function scrollToBottom() {
		await tick();
		if (chatContainer) {
			chatContainer.scrollTop = chatContainer.scrollHeight;
		}
	}

	async function handleSendMessage() {
		if (!recommendation || !chatInput.trim()) return;
		const userMessage = chatInput.trim();
		chatInput = '';

		// Add user message immediately
		const userMsg: InternistChatMessage = {
			id: `msg-user-${Date.now()}`,
			role: 'user',
			content: userMessage,
			timestamp: new Date().toISOString()
		};
		chatMessages = [...chatMessages, userMsg];
		await scrollToBottom();

		if (get(demoMode)) {
			sendingMessage = true;
			// Simulate thinking delay
			await new Promise(r => setTimeout(r, 1500));

			// Generate mock response based on input
			let responseContent = '';
			let proposedUpdate: InternistChatMessage['proposed_update'] | undefined;

			if (userMessage.toLowerCase().includes('rewrite') || userMessage.toLowerCase().includes('change')) {
				responseContent = `I've drafted a revised version of this recommendation based on your feedback:\n\n**Updated Title:** Implement centralized error boundary with retry logic\n\n**Key Changes:**\n• Added automatic retry with exponential backoff\n• Included error categorization for better debugging\n• Expanded scope to cover both API and UI errors\n\nWould you like me to apply these changes, or should we refine further?`;
				proposedUpdate = {
					title: 'Implement centralized error boundary with retry logic',
					summary: 'A comprehensive error handling solution that includes automatic retry with exponential backoff, error categorization, and unified handling for both API and UI errors.',
					priority: 'high'
				};
			} else if (userMessage.toLowerCase().includes('explain') || userMessage.toLowerCase().includes('why')) {
				responseContent = `Great question! Here's the detailed reasoning:\n\n**Why this approach?**\nThe pattern analysis across 12 recent incidents shows that 8 of them (67%) stem from inconsistent error handling. Each component handles errors differently, leading to:\n\n1. **User confusion** - errors display inconsistently\n2. **Lost context** - error details get swallowed\n3. **No recovery** - users must refresh the page\n\n**Alternative considered:**\nWe could wrap each API call individually, but this leads to code duplication and makes it easy to forget handling in new code.\n\nShould I elaborate on any specific aspect?`;
			} else if (userMessage.toLowerCase().includes('approve') || userMessage.toLowerCase().includes('ready') || userMessage.toLowerCase().includes('good')) {
				responseContent = `This recommendation looks ready for implementation.\n\n**Summary:**\n• Clear problem statement based on 12 analyzed incidents\n• Well-defined solution with specific file changes\n• High confidence (87%) in fix effectiveness\n\nYou can now **Approve** this recommendation, which will convert it into a detailed triage ticket for the Writer LLM to implement. The ticket will include:\n\n• Comprehensive error context\n• Exact files to modify\n• Expected behavior after fix\n• Test cases to verify\n\nClick **Approve & Create Ticket** when ready.`;
			} else {
				responseContent = `I understand. Let me think about that...\n\nBased on your input, I'd suggest we focus on:\n\n1. **Scope clarification** - Are we targeting all API errors or just specific endpoints?\n2. **Error categorization** - Should we distinguish between recoverable and fatal errors?\n3. **User experience** - What should users see when errors occur?\n\nLet me know which aspect you'd like to explore, or if you'd like me to draft a completely new approach.`;
			}

			const assistantMsg: InternistChatMessage = {
				id: `msg-assistant-${Date.now()}`,
				role: 'assistant',
				content: responseContent,
				timestamp: new Date().toISOString(),
				proposed_update: proposedUpdate
			};
			chatMessages = [...chatMessages, assistantMsg];
			sendingMessage = false;
			await scrollToBottom();
			return;
		}

		sendingMessage = true;
		try {
			const response = await sendInternistMessage(recommendation.id, {
				message: userMessage
			});
			chatMessages = [...chatMessages, response.message];
			await scrollToBottom();
		} catch (e) {
			toasts.error(`Failed to send message: ${(e as Error).message}`);
			// Remove the user message on error
			chatMessages = chatMessages.filter(m => m.id !== userMsg.id);
		} finally {
			sendingMessage = false;
		}
	}

	async function handleApplySuggestion(messageId: string) {
		if (!recommendation) return;

		if (get(demoMode)) {
			const msg = chatMessages.find(m => m.id === messageId);
			if (msg?.proposed_update) {
				if (msg.proposed_update.title) recommendation.title = msg.proposed_update.title;
				if (msg.proposed_update.summary) recommendation.summary = msg.proposed_update.summary;
				if (msg.proposed_update.priority) recommendation.priority = msg.proposed_update.priority;
				if (msg.proposed_update.analysis) recommendation.analysis = msg.proposed_update.analysis;
				if (msg.proposed_update.proposed_changes) recommendation.proposed_changes = msg.proposed_update.proposed_changes;
			}
			toasts.success('Changes applied to recommendation');
			// Add confirmation to chat
			chatMessages = [...chatMessages, {
				id: `msg-system-${Date.now()}`,
				role: 'assistant',
				content: '✓ Changes have been applied to the recommendation. You can see the updates above. Would you like to make any more adjustments?',
				timestamp: new Date().toISOString()
			}];
			return;
		}

		applyingSuggestion = messageId;
		try {
			recommendation = await applyInternistSuggestion(recommendation.id, messageId);
			toasts.success('Changes applied to recommendation');
		} catch (e) {
			toasts.error(`Failed to apply changes: ${(e as Error).message}`);
		} finally {
			applyingSuggestion = null;
		}
	}

	async function handleConvertToTicket() {
		if (!recommendation) return;

		if (get(demoMode)) {
			toasts.success('Recommendation converted to triage ticket (demo mode)');
			recommendation.status = 'implementing';
			return;
		}

		convertingToTicket = true;
		try {
			const result = await convertToTriageTicket(recommendation.id);
			toasts.success(`Created triage ticket: ${result.incident_id}`);
			// Redirect to the new incident
			window.location.href = `${base}/`;
		} catch (e) {
			toasts.error(`Failed to convert: ${(e as Error).message}`);
		} finally {
			convertingToTicket = false;
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			handleSendMessage();
		}
	}

	function formatDate(dateStr: string | undefined): string {
		if (!dateStr) return '';
		const date = new Date(dateStr);
		return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric', hour: '2-digit', minute: '2-digit' });
	}

	function getCategoryLabel(category: string | undefined): string {
		switch (category) {
			case 'api_contract': return 'API Contract';
			case 'architecture': return 'Architecture';
			case 'error_pattern': return 'Error Pattern';
			case 'performance': return 'Performance';
			case 'security': return 'Security';
			default: return category || 'Unknown';
		}
	}

	function getStatusLabel(status: string): string {
		switch (status) {
			case 'analyzing': return 'Analyzing';
			case 'proposed': return 'Proposed';
			case 'discussing': return 'Discussing';
			case 'approved': return 'Approved';
			case 'implementing': return 'Implementing';
			case 'completed': return 'Completed';
			case 'rejected': return 'Rejected';
			default: return status;
		}
	}

	function getChangeTypeLabel(type: string): string {
		switch (type) {
			case 'create': return 'New File';
			case 'modify': return 'Modified';
			case 'delete': return 'Deleted';
			default: return type;
		}
	}

	function formatMessageContent(content: string): string {
		// Simple markdown-like formatting
		return content
			// Bold
			.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
			// Italic
			.replace(/\*(.+?)\*/g, '<em>$1</em>')
			// Code
			.replace(/`(.+?)`/g, '<code>$1</code>')
			// Bullet points
			.replace(/^• (.+)$/gm, '<li>$1</li>')
			// Numbered lists
			.replace(/^(\d+)\. (.+)$/gm, '<li>$2</li>')
			// Newlines to breaks
			.replace(/\n/g, '<br>');
	}
</script>

<svelte:head>
	<title>{recommendation?.title || 'Recommendation'} - CRISPR</title>
</svelte:head>

<div class="recommendation-page">
	<nav class="breadcrumb">
		<a href="{base}/">Dashboard</a>
		<span class="separator">/</span>
		<span>Recommendations</span>
		<span class="separator">/</span>
		<span class="current">{recommendation?.title?.slice(0, 30) || 'Loading...'}...</span>
	</nav>

	{#if loading}
		<div class="loading">
			<div class="spinner"></div>
			<span>Loading recommendation...</span>
		</div>
	{:else if error}
		<div class="error-state">
			<h2>Error loading recommendation</h2>
			<p>{error}</p>
			<a href="{base}/" class="btn btn-primary">Return to Dashboard</a>
		</div>
	{:else if !recommendation}
		<div class="error-state">
			<h2>Recommendation not found</h2>
			<p>The recommendation you're looking for doesn't exist or has been removed.</p>
			<a href="{base}/" class="btn btn-primary">Return to Dashboard</a>
		</div>
	{:else}
		<div class="rec-layout">
			<main class="rec-main">
				<!-- Header -->
				<header class="rec-header">
					<div class="rec-badges">
						<span class="rec-category rec-category-{recommendation.category}">{getCategoryLabel(recommendation.category)}</span>
						<span class="rec-status rec-status-{recommendation.status}">{getStatusLabel(recommendation.status)}</span>
						<span class="rec-priority rec-priority-{recommendation.priority}">{recommendation.priority}</span>
					</div>
					<h1>{recommendation.title}</h1>
					{#if recommendation.confidence}
						<div class="rec-confidence">
							<span class="confidence-value">{Math.round(recommendation.confidence * 100)}%</span>
							<span class="confidence-label">confidence</span>
						</div>
					{/if}
				</header>

				<!-- Conflict Warning -->
				{#if recommendation.has_conflicts}
					<div class="conflict-banner">
						<svg viewBox="0 0 20 20" fill="currentColor" width="20" height="20">
							<path fill-rule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clip-rule="evenodd"/>
						</svg>
						<div class="conflict-content">
							<strong>Merge conflicts detected</strong>
							<p>The base code has changed since this recommendation was created. Review and rebase to resolve conflicts.</p>
							{#if recommendation.conflict_details}
								<ul class="conflict-files">
									{#each recommendation.conflict_details as conflict}
										<li><code>{conflict.file_path}</code> - {conflict.description}</li>
									{/each}
								</ul>
							{/if}
						</div>
					</div>
				{/if}

				<!-- Summary -->
				<section class="rec-section">
					<h2>Summary</h2>
					<p class="rec-summary">{recommendation.summary}</p>
				</section>

				<!-- Analysis -->
				{#if recommendation.analysis}
					<section class="rec-section">
						<h2>Analysis</h2>
						<div class="analysis-grid">
							{#if recommendation.analysis.patterns_found}
								<div class="analysis-item">
									<h3>Patterns Found</h3>
									<ul>
										{#each recommendation.analysis.patterns_found as pattern}
											<li>{pattern}</li>
										{/each}
									</ul>
								</div>
							{/if}
							{#if recommendation.analysis.root_cause}
								<div class="analysis-item">
									<h3>Root Cause</h3>
									<p>{recommendation.analysis.root_cause}</p>
								</div>
							{/if}
							{#if recommendation.analysis.impact_assessment}
								<div class="analysis-item">
									<h3>Impact Assessment</h3>
									<p>{recommendation.analysis.impact_assessment}</p>
								</div>
							{/if}
						</div>
					</section>
				{/if}

				<!-- Proposed Changes -->
				{#if recommendation.proposed_changes && recommendation.proposed_changes.length > 0}
					<section class="rec-section">
						<h2>Proposed Changes</h2>
						<div class="changes-list">
							{#each recommendation.proposed_changes as change}
								<div class="change-item">
									<div class="change-header">
										<span class="change-type change-type-{change.change_type}">{getChangeTypeLabel(change.change_type)}</span>
										<code class="change-path">{change.file_path}</code>
									</div>
									<p class="change-description">{change.description}</p>
								</div>
							{/each}
						</div>
					</section>
				{/if}

				<!-- Source Incidents -->
				{#if recommendation.source_incident_ids && recommendation.source_incident_ids.length > 0}
					<section class="rec-section">
						<h2>Source Incidents</h2>
						<p class="section-subtitle">This recommendation is based on analysis of these incidents:</p>
						<div class="incident-links">
							{#each recommendation.source_incident_ids as incidentId}
								<span class="incident-chip">{incidentId}</span>
							{/each}
						</div>
					</section>
				{/if}

				<!-- Internist Chat -->
				<section class="rec-section chat-section">
					<div class="chat-header">
						<h2>
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="20" height="20">
								<path stroke-linecap="round" stroke-linejoin="round" d="M9.75 3.104v5.714a2.25 2.25 0 01-.659 1.591L5 14.5M9.75 3.104c-.251.023-.501.05-.75.082m.75-.082a24.301 24.301 0 014.5 0m0 0v5.714c0 .597.237 1.17.659 1.591L19.8 15.3M14.25 3.104c.251.023.501.05.75.082M19.8 15.3l-1.57.393A9.065 9.065 0 0112 15a9.065 9.065 0 00-6.23.693L5 14.5m14.8.8l1.402 1.402c1.232 1.232.65 3.318-1.067 3.611l-2.553.383A9.1 9.1 0 0112 21a9.1 9.1 0 01-5.582-.804l-2.553-.383c-1.717-.293-2.299-2.379-1.067-3.611L5 14.5" />
							</svg>
							Internist Console
						</h2>
						<span class="chat-subtitle">Discuss, refine, or rewrite this recommendation</span>
					</div>

					<div class="chat-container" bind:this={chatContainer}>
						{#each chatMessages as message}
							<div class="chat-message chat-message-{message.role}">
								<div class="message-avatar">
									{#if message.role === 'assistant'}
										<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="20" height="20">
											<path stroke-linecap="round" stroke-linejoin="round" d="M9.75 3.104v5.714a2.25 2.25 0 01-.659 1.591L5 14.5M9.75 3.104c-.251.023-.501.05-.75.082m.75-.082a24.301 24.301 0 014.5 0m0 0v5.714c0 .597.237 1.17.659 1.591L19.8 15.3M14.25 3.104c.251.023.501.05.75.082M19.8 15.3l-1.57.393A9.065 9.065 0 0112 15a9.065 9.065 0 00-6.23.693L5 14.5m14.8.8l1.402 1.402c1.232 1.232.65 3.318-1.067 3.611A9.1 9.1 0 0112 21a9.1 9.1 0 01-5.582-.804c-1.717-.293-2.299-2.379-1.067-3.611L5 14.5" />
										</svg>
									{:else}
										<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="20" height="20">
											<path stroke-linecap="round" stroke-linejoin="round" d="M15.75 6a3.75 3.75 0 11-7.5 0 3.75 3.75 0 017.5 0zM4.501 20.118a7.5 7.5 0 0114.998 0A17.933 17.933 0 0112 21.75c-2.676 0-5.216-.584-7.499-1.632z" />
										</svg>
									{/if}
								</div>
								<div class="message-content">
									<div class="message-text">{@html formatMessageContent(message.content)}</div>
									{#if message.proposed_update}
										<div class="proposed-update">
											<div class="proposed-header">
												<svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
													<path fill-rule="evenodd" d="M11.3 1.046A1 1 0 0112 2v5h4a1 1 0 01.82 1.573l-7 10A1 1 0 018 18v-5H4a1 1 0 01-.82-1.573l7-10a1 1 0 011.12-.38z" clip-rule="evenodd"/>
												</svg>
												Proposed Changes
											</div>
											{#if message.proposed_update.title}
												<div class="proposed-item">
													<span class="proposed-label">Title:</span>
													<span class="proposed-value">{message.proposed_update.title}</span>
												</div>
											{/if}
											{#if message.proposed_update.summary}
												<div class="proposed-item">
													<span class="proposed-label">Summary:</span>
													<span class="proposed-value">{message.proposed_update.summary}</span>
												</div>
											{/if}
											{#if message.proposed_update.priority}
												<div class="proposed-item">
													<span class="proposed-label">Priority:</span>
													<span class="proposed-value priority-{message.proposed_update.priority}">{message.proposed_update.priority}</span>
												</div>
											{/if}
											<button
												class="btn btn-primary btn-sm apply-btn"
												onclick={() => handleApplySuggestion(message.id)}
												disabled={applyingSuggestion === message.id}
											>
												{applyingSuggestion === message.id ? 'Applying...' : 'Apply These Changes'}
											</button>
										</div>
									{/if}
								</div>
							</div>
						{/each}
						{#if sendingMessage}
							<div class="chat-message chat-message-assistant">
								<div class="message-avatar">
									<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="20" height="20">
										<path stroke-linecap="round" stroke-linejoin="round" d="M9.75 3.104v5.714a2.25 2.25 0 01-.659 1.591L5 14.5M9.75 3.104c-.251.023-.501.05-.75.082m.75-.082a24.301 24.301 0 014.5 0m0 0v5.714c0 .597.237 1.17.659 1.591L19.8 15.3M14.25 3.104c.251.023.501.05.75.082M19.8 15.3l-1.57.393A9.065 9.065 0 0112 15a9.065 9.065 0 00-6.23.693L5 14.5m14.8.8l1.402 1.402c1.232 1.232.65 3.318-1.067 3.611A9.1 9.1 0 0112 21a9.1 9.1 0 01-5.582-.804c-1.717-.293-2.299-2.379-1.067-3.611L5 14.5" />
									</svg>
								</div>
								<div class="message-content">
									<div class="typing-indicator">
										<span></span><span></span><span></span>
									</div>
								</div>
							</div>
						{/if}
					</div>

					<div class="chat-input-area">
						<textarea
							bind:value={chatInput}
							onkeydown={handleKeyDown}
							placeholder="Ask me to explain, refine, or rewrite this recommendation..."
							rows="2"
							disabled={sendingMessage}
						></textarea>
						<button
							class="btn btn-primary send-btn"
							onclick={handleSendMessage}
							disabled={sendingMessage || !chatInput.trim()}
						>
							{#if sendingMessage}
								<div class="btn-spinner"></div>
							{:else}
								<svg viewBox="0 0 20 20" fill="currentColor" width="18" height="18">
									<path d="M10.894 2.553a1 1 0 00-1.788 0l-7 14a1 1 0 001.169 1.409l5-1.429A1 1 0 009 15.571V11a1 1 0 112 0v4.571a1 1 0 00.725.962l5 1.428a1 1 0 001.17-1.408l-7-14z"/>
								</svg>
							{/if}
						</button>
					</div>
				</section>
			</main>

			<aside class="rec-sidebar">
				<!-- Actions -->
				<div class="sidebar-card">
					<h3>Actions</h3>
					<div class="action-buttons">
						{#if recommendation.status === 'proposed' || recommendation.status === 'discussing'}
							<button class="btn btn-success" onclick={handleApprove} disabled={approving || convertingToTicket}>
								{#if approving}
									Approving...
								{:else}
									<svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
										<path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
									</svg>
									Approve & Create Ticket
								{/if}
							</button>
							<button class="btn btn-secondary" onclick={handleReject} disabled={rejecting}>
								{rejecting ? 'Rejecting...' : 'Reject'}
							</button>
						{/if}
						{#if recommendation.status === 'approved'}
							<button class="btn btn-primary" onclick={handleConvertToTicket} disabled={convertingToTicket}>
								{#if convertingToTicket}
									Creating Ticket...
								{:else}
									<svg viewBox="0 0 20 20" fill="currentColor" width="16" height="16">
										<path fill-rule="evenodd" d="M6 2a2 2 0 00-2 2v12a2 2 0 002 2h8a2 2 0 002-2V7.414A2 2 0 0015.414 6L12 2.586A2 2 0 0010.586 2H6zm5 6a1 1 0 10-2 0v2H7a1 1 0 100 2h2v2a1 1 0 102 0v-2h2a1 1 0 100-2h-2V8z" clip-rule="evenodd"/>
									</svg>
									Create Triage Ticket
								{/if}
							</button>
							<button class="btn btn-secondary" onclick={handleImplement} disabled={implementing}>
								{implementing ? 'Creating PR...' : 'Create Implementation PR'}
							</button>
						{/if}
						{#if recommendation.status === 'implementing' && recommendation.pr_url}
							<a href={recommendation.pr_url} target="_blank" class="btn btn-secondary">
								<svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
									<path fill-rule="evenodd" d="M7.177 3.073L9.573.677A.25.25 0 0110 .854v4.792a.25.25 0 01-.427.177L7.177 3.427a.25.25 0 010-.354zM3.75 2.5a.75.75 0 100 1.5.75.75 0 000-1.5zm-2.25.75a2.25 2.25 0 113 2.122v5.256a2.251 2.251 0 11-1.5 0V5.372A2.25 2.25 0 011.5 3.25zM11 2.5h-1V4h1a1 1 0 011 1v5.628a2.251 2.251 0 101.5 0V5A2.5 2.5 0 0011 2.5zm1 10.25a.75.75 0 111.5 0 .75.75 0 01-1.5 0zM3.75 12a.75.75 0 100 1.5.75.75 0 000-1.5z"/>
								</svg>
								View PR #{recommendation.pr_number}
							</a>
						{/if}
						<button class="btn btn-secondary" onclick={handleCheckConflicts} disabled={checkingConflicts}>
							{checkingConflicts ? 'Checking...' : 'Check Conflicts'}
						</button>
						{#if recommendation.has_conflicts}
							<button class="btn btn-secondary" onclick={handleRebase} disabled={rebasing}>
								{rebasing ? 'Rebasing...' : 'Attempt Rebase'}
							</button>
						{/if}
					</div>
				</div>

				<!-- Details -->
				<div class="sidebar-card">
					<h3>Details</h3>
					<dl class="details-list">
						<dt>Status</dt>
						<dd>{getStatusLabel(recommendation.status)}</dd>
						<dt>Priority</dt>
						<dd class="priority-{recommendation.priority}">{recommendation.priority}</dd>
						<dt>Category</dt>
						<dd>{getCategoryLabel(recommendation.category)}</dd>
						<dt>Created</dt>
						<dd>{formatDate(recommendation.created_at)}</dd>
						{#if recommendation.approved_at}
							<dt>Approved</dt>
							<dd>{formatDate(recommendation.approved_at)}</dd>
						{/if}
						{#if recommendation.approved_by}
							<dt>Approved By</dt>
							<dd>{recommendation.approved_by}</dd>
						{/if}
					</dl>
				</div>

				<!-- PR Info -->
				{#if recommendation.pr_url}
					<div class="sidebar-card">
						<h3>Pull Request</h3>
						<a href={recommendation.pr_url} target="_blank" class="pr-link">
							<svg viewBox="0 0 16 16" fill="currentColor" width="16" height="16">
								<path fill-rule="evenodd" d="M7.177 3.073L9.573.677A.25.25 0 0110 .854v4.792a.25.25 0 01-.427.177L7.177 3.427a.25.25 0 010-.354zM3.75 2.5a.75.75 0 100 1.5.75.75 0 000-1.5zm-2.25.75a2.25 2.25 0 113 2.122v5.256a2.251 2.251 0 11-1.5 0V5.372A2.25 2.25 0 011.5 3.25zM11 2.5h-1V4h1a1 1 0 011 1v5.628a2.251 2.251 0 101.5 0V5A2.5 2.5 0 0011 2.5zm1 10.25a.75.75 0 111.5 0 .75.75 0 01-1.5 0zM3.75 12a.75.75 0 100 1.5.75.75 0 000-1.5z"/>
							</svg>
							PR #{recommendation.pr_number}
							{#if recommendation.pr_state}
								<span class="pr-state pr-state-{recommendation.pr_state}">{recommendation.pr_state}</span>
							{/if}
						</a>
						{#if recommendation.pr_branch}
							<p class="pr-branch"><code>{recommendation.pr_branch}</code></p>
						{/if}
					</div>
				{/if}
			</aside>
		</div>
	{/if}
</div>

<style>
	.recommendation-page {
		max-width: 1400px;
		margin: 0 auto;
	}

	.breadcrumb {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		font-size: 0.875rem;
		color: var(--slate);
		margin-bottom: var(--space-xl);
	}

	.breadcrumb a {
		color: var(--scrubs);
		text-decoration: none;
	}

	.breadcrumb a:hover {
		text-decoration: underline;
	}

	.breadcrumb .separator {
		color: var(--silver);
	}

	.breadcrumb .current {
		color: var(--graphite);
	}

	.loading, .error-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--space-3xl);
		text-align: center;
	}

	.spinner {
		width: 40px;
		height: 40px;
		border: 3px solid var(--silver);
		border-top-color: var(--scrubs);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
		margin-bottom: var(--space-md);
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	.rec-layout {
		display: grid;
		grid-template-columns: 1fr 300px;
		gap: var(--space-xl);
	}

	.rec-main {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
	}

	.rec-header {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-xl);
	}

	.rec-badges {
		display: flex;
		gap: var(--space-sm);
		margin-bottom: var(--space-md);
	}

	.rec-category, .rec-status, .rec-priority {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		padding: 4px 8px;
		border-radius: var(--radius-sm);
	}

	.rec-category {
		background: var(--paper);
		color: var(--graphite);
	}

	.rec-category-api_contract { background: var(--critical-soft); color: #B91C1C; }
	.rec-category-architecture { background: var(--vitals-soft); color: #1D4ED8; }
	.rec-category-error_pattern { background: var(--alert-soft); color: #B45309; }
	.rec-category-performance { background: var(--scrubs-soft); color: var(--scrubs-dark); }
	.rec-category-security { background: #FEE2E2; color: #991B1B; }

	.rec-status {
		background: var(--paper);
		color: var(--slate);
	}

	.rec-status-analyzing { background: var(--vitals-soft); color: #1D4ED8; }
	.rec-status-proposed { background: var(--alert-soft); color: #B45309; }
	.rec-status-discussing { background: var(--caution-soft); color: var(--caution); }
	.rec-status-approved { background: var(--scrubs-soft); color: var(--scrubs-dark); }
	.rec-status-implementing { background: var(--scrubs-soft); color: var(--scrubs-dark); }
	.rec-status-completed { background: var(--stable-soft); color: var(--stable-dark); }
	.rec-status-rejected { background: var(--critical-soft); color: #B91C1C; }

	.rec-priority {
		text-transform: capitalize;
	}

	.rec-priority-critical { background: var(--critical-soft); color: #B91C1C; }
	.rec-priority-high { background: var(--alert-soft); color: #B45309; }
	.rec-priority-medium { background: var(--paper); color: var(--graphite); }
	.rec-priority-low { background: var(--paper); color: var(--slate); }

	.rec-header h1 {
		font-size: 1.5rem;
		font-weight: 600;
		color: var(--obsidian);
		margin: 0;
		line-height: 1.3;
	}

	.rec-confidence {
		display: flex;
		align-items: baseline;
		gap: var(--space-xs);
		margin-top: var(--space-md);
	}

	.confidence-value {
		font-family: var(--font-display);
		font-size: 1.25rem;
		color: var(--scrubs-dark);
	}

	.confidence-label {
		font-size: 0.8125rem;
		color: var(--slate);
	}

	.conflict-banner {
		display: flex;
		gap: var(--space-md);
		padding: var(--space-md);
		background: var(--caution-soft);
		border: 1px solid var(--caution);
		border-radius: var(--radius-md);
		color: var(--caution);
	}

	.conflict-banner svg {
		flex-shrink: 0;
	}

	.conflict-content strong {
		display: block;
		margin-bottom: var(--space-xs);
	}

	.conflict-content p {
		margin: 0;
		font-size: 0.875rem;
	}

	.conflict-files {
		margin: var(--space-sm) 0 0;
		padding-left: var(--space-lg);
		font-size: 0.8125rem;
	}

	.conflict-files code {
		background: rgba(0, 0, 0, 0.1);
		padding: 1px 4px;
		border-radius: var(--radius-sm);
	}

	.rec-section {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-lg);
	}

	.rec-section h2 {
		font-size: 1rem;
		font-weight: 600;
		color: var(--obsidian);
		margin: 0 0 var(--space-md);
		padding-bottom: var(--space-sm);
		border-bottom: 1px solid var(--paper);
	}

	.rec-summary {
		font-size: 1rem;
		line-height: 1.6;
		color: var(--charcoal);
		margin: 0;
	}

	.analysis-grid {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	.analysis-item h3 {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--graphite);
		margin: 0 0 var(--space-sm);
	}

	.analysis-item ul {
		margin: 0;
		padding-left: var(--space-lg);
	}

	.analysis-item li {
		font-size: 0.9375rem;
		color: var(--charcoal);
		margin-bottom: var(--space-xs);
	}

	.analysis-item p {
		margin: 0;
		font-size: 0.9375rem;
		color: var(--charcoal);
	}

	.changes-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.change-item {
		padding: var(--space-md);
		background: var(--paper);
		border-radius: var(--radius-md);
	}

	.change-header {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin-bottom: var(--space-sm);
	}

	.change-type {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		padding: 2px 6px;
		border-radius: var(--radius-sm);
	}

	.change-type-create { background: var(--scrubs-soft); color: var(--scrubs-dark); }
	.change-type-modify { background: var(--alert-soft); color: #B45309; }
	.change-type-delete { background: var(--critical-soft); color: #B91C1C; }

	.change-path {
		font-size: 0.8125rem;
		color: var(--charcoal);
	}

	.change-description {
		margin: 0;
		font-size: 0.875rem;
		color: var(--graphite);
	}

	.section-subtitle {
		font-size: 0.875rem;
		color: var(--slate);
		margin: 0 0 var(--space-md);
	}

	.incident-links {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-sm);
	}

	.incident-chip {
		font-size: 0.8125rem;
		padding: var(--space-xs) var(--space-sm);
		background: var(--vitals-soft);
		color: #1D4ED8;
		border-radius: var(--radius-sm);
	}

	/* Chat Interface */
	.chat-section {
		display: flex;
		flex-direction: column;
		padding: 0;
		overflow: hidden;
	}

	.chat-header {
		padding: var(--space-lg);
		border-bottom: 1px solid var(--paper);
	}

	.chat-header h2 {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin: 0;
		padding: 0;
		border: none;
		color: var(--vitals);
	}

	.chat-header h2 svg {
		color: var(--vitals);
	}

	.chat-subtitle {
		display: block;
		font-size: 0.8125rem;
		color: var(--slate);
		margin-top: var(--space-xs);
	}

	.chat-container {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-md);
		max-height: 500px;
		min-height: 300px;
		background: var(--paper);
	}

	.chat-message {
		display: flex;
		gap: var(--space-md);
		margin-bottom: var(--space-md);
	}

	.chat-message-user {
		flex-direction: row-reverse;
	}

	.message-avatar {
		flex-shrink: 0;
		width: 32px;
		height: 32px;
		border-radius: 50%;
		display: flex;
		align-items: center;
		justify-content: center;
		background: var(--silver);
		color: var(--graphite);
	}

	.chat-message-assistant .message-avatar {
		background: var(--vitals-soft);
		color: var(--vitals);
	}

	.chat-message-user .message-avatar {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.message-content {
		max-width: 80%;
		padding: var(--space-md);
		border-radius: var(--radius-lg);
		background: var(--white);
		box-shadow: var(--shadow-sm);
	}

	.chat-message-user .message-content {
		background: var(--scrubs-soft);
		color: var(--scrubs-dark);
	}

	.message-text {
		font-size: 0.9375rem;
		line-height: 1.6;
		color: var(--charcoal);
	}

	.message-text strong {
		font-weight: 600;
		color: var(--obsidian);
	}

	.message-text code {
		background: var(--paper);
		padding: 1px 4px;
		border-radius: var(--radius-sm);
		font-size: 0.875em;
	}

	.chat-message-user .message-text {
		color: var(--scrubs-dark);
	}

	.chat-message-user .message-text code {
		background: rgba(0, 0, 0, 0.1);
	}

	.proposed-update {
		margin-top: var(--space-md);
		padding: var(--space-md);
		background: linear-gradient(135deg, var(--scrubs-soft), #E0F2FE);
		border-radius: var(--radius-md);
		border: 1px solid var(--scrubs);
	}

	.proposed-header {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		font-weight: 600;
		font-size: 0.8125rem;
		color: var(--scrubs-dark);
		margin-bottom: var(--space-sm);
	}

	.proposed-item {
		display: flex;
		gap: var(--space-sm);
		margin-bottom: var(--space-xs);
		font-size: 0.875rem;
	}

	.proposed-label {
		color: var(--slate);
		min-width: 70px;
	}

	.proposed-value {
		color: var(--charcoal);
		flex: 1;
	}

	.proposed-value.priority-high,
	.proposed-value.priority-critical {
		color: var(--alert);
		font-weight: 600;
	}

	.apply-btn {
		margin-top: var(--space-sm);
	}

	.typing-indicator {
		display: flex;
		gap: 4px;
		padding: var(--space-sm) 0;
	}

	.typing-indicator span {
		width: 8px;
		height: 8px;
		background: var(--slate);
		border-radius: 50%;
		animation: typing 1.4s infinite ease-in-out;
	}

	.typing-indicator span:nth-child(2) {
		animation-delay: 0.2s;
	}

	.typing-indicator span:nth-child(3) {
		animation-delay: 0.4s;
	}

	@keyframes typing {
		0%, 60%, 100% { transform: translateY(0); opacity: 0.4; }
		30% { transform: translateY(-4px); opacity: 1; }
	}

	.chat-input-area {
		display: flex;
		gap: var(--space-sm);
		padding: var(--space-md);
		border-top: 1px solid var(--paper);
		background: var(--white);
	}

	.chat-input-area textarea {
		flex: 1;
		padding: var(--space-sm) var(--space-md);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		font-size: 0.9375rem;
		font-family: var(--font-body);
		resize: none;
		line-height: 1.4;
	}

	.chat-input-area textarea:focus {
		outline: none;
		border-color: var(--vitals);
		box-shadow: 0 0 0 3px var(--vitals-soft);
	}

	.chat-input-area textarea:disabled {
		background: var(--paper);
		cursor: not-allowed;
	}

	.send-btn {
		flex-shrink: 0;
		width: 44px;
		height: 44px;
		padding: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
	}

	.send-btn:disabled {
		opacity: 0.5;
	}

	.btn-spinner {
		width: 18px;
		height: 18px;
		border: 2px solid transparent;
		border-top-color: currentColor;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	.btn-sm {
		padding: var(--space-xs) var(--space-sm);
		font-size: 0.8125rem;
	}

	.btn-success {
		background: var(--scrubs);
		color: white;
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.btn-success:hover {
		background: var(--scrubs-dark);
	}

	.btn svg {
		flex-shrink: 0;
	}

	/* Sidebar */
	.rec-sidebar {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.sidebar-card {
		background: var(--white);
		border: 1px solid var(--silver);
		border-radius: var(--radius-lg);
		padding: var(--space-md);
	}

	.sidebar-card h3 {
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--graphite);
		margin: 0 0 var(--space-md);
		padding-bottom: var(--space-sm);
		border-bottom: 1px solid var(--paper);
	}

	.action-buttons {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.action-buttons .btn {
		width: 100%;
		justify-content: center;
	}

	.details-list {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: var(--space-sm) var(--space-md);
		font-size: 0.875rem;
	}

	.details-list dt {
		color: var(--slate);
	}

	.details-list dd {
		margin: 0;
		color: var(--charcoal);
	}

	.details-list .priority-critical { color: #B91C1C; }
	.details-list .priority-high { color: #B45309; }

	.pr-link {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		color: var(--scrubs);
		text-decoration: none;
		font-weight: 500;
	}

	.pr-link:hover {
		text-decoration: underline;
	}

	.pr-state {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		padding: 2px 6px;
		border-radius: var(--radius-sm);
		background: var(--paper);
		color: var(--slate);
	}

	.pr-state-open { background: var(--scrubs-soft); color: var(--scrubs-dark); }
	.pr-state-merged { background: #E9D5FF; color: #7C3AED; }
	.pr-state-closed { background: var(--critical-soft); color: #B91C1C; }

	.pr-branch {
		margin: var(--space-sm) 0 0;
		font-size: 0.75rem;
		color: var(--slate);
	}

	.pr-branch code {
		background: var(--paper);
		padding: 2px 4px;
		border-radius: var(--radius-sm);
	}

	@media (max-width: 1024px) {
		.rec-layout {
			grid-template-columns: 1fr;
		}

		.rec-sidebar {
			order: -1;
		}
	}

	@media (max-width: 768px) {
		.breadcrumb {
			font-size: 0.75rem;
		}

		.rec-header h1 {
			font-size: 1.25rem;
		}
	}
</style>
