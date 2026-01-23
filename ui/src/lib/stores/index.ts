import { writable, derived, get, type Readable } from 'svelte/store';
import * as api from '$lib/api';
import type { Project, Incident, AuthStatus, LlmConfig, UsageSummary, IncidentStats } from '$lib/api';
import {
	mockProjects,
	mockIncidents,
	mockAuthStatus,
	mockUsageSummary,
	mockIncidentStats,
	mockUsageDetails,
	getMockIncidentsWithProjects
} from '$lib/mockData';

// Refresh intervals
const REFRESH_INTERVAL = 30000; // 30 seconds

// Demo mode store - persisted to localStorage
function createDemoModeStore() {
	// Start with false, will hydrate from localStorage on client
	const { subscribe, set, update } = writable<boolean>(false);
	let initialized = false;

	return {
		subscribe,
		init() {
			// Only run on client, only once
			if (initialized || typeof window === 'undefined') return;
			initialized = true;
			const stored = localStorage.getItem('crispr_demo_mode');
			if (stored === 'true') {
				set(true);
			}
		},
		toggle() {
			update(v => {
				const newVal = !v;
				if (typeof localStorage !== 'undefined') {
					localStorage.setItem('crispr_demo_mode', String(newVal));
				}
				console.log('Demo mode toggled to:', newVal);
				return newVal;
			});
		},
		set(value: boolean) {
			set(value);
			if (typeof localStorage !== 'undefined') {
				localStorage.setItem('crispr_demo_mode', String(value));
			}
		}
	};
}

export const demoMode = createDemoModeStore();

// Projects store
function createProjectsStore() {
	const { subscribe, set, update } = writable<Project[]>([]);
	let refreshInterval: ReturnType<typeof setInterval> | null = null;

	return {
		subscribe,
		async load() {
			if (get(demoMode)) {
				set(mockProjects);
				return;
			}
			try {
				const projects = await api.getProjects();
				set(Array.isArray(projects) ? projects : []);
			} catch (e) {
				console.error('Failed to load projects:', e);
				set([]);
			}
		},
		async create(repo: string) {
			if (get(demoMode)) {
				const newProject: Project = {
					id: `proj-${Date.now()}`,
					repo,
					enabled: true,
					auto_fix: true,
					watch_logs: true,
					created_at: new Date().toISOString(),
					incident_count: 0
				};
				update(projects => [...projects, newProject]);
				return newProject;
			}
			const project = await api.createProject(repo);
			update(projects => [...projects, project]);
			return project;
		},
		async update(id: string, updates: Partial<Project>) {
			if (get(demoMode)) {
				update(projects => projects.map(p => p.id === id ? { ...p, ...updates } : p));
				return { ...mockProjects.find(p => p.id === id), ...updates } as Project;
			}
			const updated = await api.updateProject(id, updates);
			update(projects => projects.map(p => p.id === id ? updated : p));
			return updated;
		},
		async delete(id: string) {
			if (get(demoMode)) {
				update(projects => projects.filter(p => p.id !== id));
				return;
			}
			await api.deleteProject(id);
			update(projects => projects.filter(p => p.id !== id));
		},
		startRefresh() {
			this.load();
			refreshInterval = setInterval(() => this.load(), REFRESH_INTERVAL);
		},
		stopRefresh() {
			if (refreshInterval) {
				clearInterval(refreshInterval);
				refreshInterval = null;
			}
		}
	};
}

// Incidents store
function createIncidentsStore() {
	const { subscribe, set, update } = writable<Incident[]>([]);
	let refreshInterval: ReturnType<typeof setInterval> | null = null;

	return {
		subscribe,
		async load(params?: { project_id?: string; status?: string }) {
			if (get(demoMode)) {
				let incidents = getMockIncidentsWithProjects();
				if (params?.project_id) {
					incidents = incidents.filter(i => i.project_id === params.project_id);
				}
				if (params?.status) {
					incidents = incidents.filter(i => i.status === params.status);
				}
				set(incidents);
				return;
			}
			try {
				const incidents = await api.getIncidents(params);
				set(Array.isArray(incidents) ? incidents : []);
			} catch (e) {
				console.error('Failed to load incidents:', e);
				set([]);
			}
		},
		async retry(id: string) {
			if (get(demoMode)) {
				update(incidents =>
					incidents.map(i =>
						i.id === id ? { ...i, status: 'detected' as const, fix_attempts: (i.fix_attempts || 0) + 1 } : i
					)
				);
				return;
			}
			await api.retryIncident(id);
			update(incidents =>
				incidents.map(i =>
					i.id === id ? { ...i, status: 'detected' as const } : i
				)
			);
		},
		async ignore(id: string) {
			if (get(demoMode)) {
				update(incidents =>
					incidents.map(i =>
						i.id === id ? { ...i, status: 'ignored' as const } : i
					)
				);
				return;
			}
			await api.ignoreIncident(id);
			update(incidents =>
				incidents.map(i =>
					i.id === id ? { ...i, status: 'ignored' as const } : i
				)
			);
		},
		startRefresh(params?: { project_id?: string; status?: string }) {
			this.load(params);
			refreshInterval = setInterval(() => this.load(params), REFRESH_INTERVAL);
		},
		stopRefresh() {
			if (refreshInterval) {
				clearInterval(refreshInterval);
				refreshInterval = null;
			}
		}
	};
}

// Stats store
function createStatsStore() {
	const { subscribe, set } = writable<IncidentStats | null>(null);

	return {
		subscribe,
		async load() {
			if (get(demoMode)) {
				set(mockIncidentStats);
				return;
			}
			try {
				const stats = await api.getIncidentStats();
				set(stats);
			} catch (e) {
				console.error('Failed to load stats:', e);
				set({
					total: 0,
					by_status: {},
					fixed_today: 0,
					fixed_week: 0
				});
			}
		}
	};
}

// Auth status store
function createAuthStore() {
	const { subscribe, set } = writable<AuthStatus | null>(null);

	return {
		subscribe,
		async load() {
			if (get(demoMode)) {
				set(mockAuthStatus);
				return;
			}
			try {
				const status = await api.getAuthStatus();
				set(status);
			} catch (e) {
				console.error('Failed to load auth status:', e);
				// Clear auth status on error - show as disconnected with connect buttons
				set({
					github: { connected: false, mode: '' },
					slack: { enabled: true, connected: false },
					llm: { configured: false }
				});
			}
		}
	};
}

// LLM config store
function createLlmConfigStore() {
	const { subscribe, set } = writable<LlmConfig | null>(null);

	return {
		subscribe,
		async load() {
			if (get(demoMode)) {
				set({
					reader_provider: 'anthropic',
					reader_model: 'claude-3-haiku-20240307',
					writer_provider: 'anthropic',
					writer_model: 'claude-sonnet-4-20250514'
				});
				return;
			}
			try {
				const config = await api.getLlmConfig();
				set(config);
			} catch (e) {
				console.error('Failed to load LLM config:', e);
			}
		},
		async update(config: Parameters<typeof api.updateLlmConfig>[0]) {
			if (get(demoMode)) {
				// In demo mode, just update the local store
				return;
			}
			await api.updateLlmConfig(config);
			await this.load();
		}
	};
}

// Usage store
function createUsageStore() {
	const { subscribe, set } = writable<UsageSummary | null>(null);

	return {
		subscribe,
		async load(params?: { start_date?: string; end_date?: string }) {
			if (get(demoMode)) {
				set(mockUsageSummary);
				return;
			}
			try {
				const usage = await api.getUsageSummary(params);
				set(usage);
			} catch (e) {
				console.error('Failed to load usage:', e);
			}
		},
		async loadDetails(params?: { start_date?: string; end_date?: string }) {
			if (get(demoMode)) {
				return mockUsageDetails;
			}
			try {
				return await api.getUsageDetails(params);
			} catch (e) {
				console.error('Failed to load usage details:', e);
				return [];
			}
		}
	};
}

// Export stores
export const projects = createProjectsStore();
export const incidents = createIncidentsStore();
export const stats = createStatsStore();
export const auth = createAuthStore();
export const llmConfig = createLlmConfigStore();
export const usage = createUsageStore();

// Derived stores
export const activeIncidents: Readable<Incident[]> = derived(
	incidents,
	$incidents => $incidents.filter(i =>
		!['pr_merged', 'ignored', 'unfixable', 'max_attempts'].includes(i.status)
	)
);

export const pendingPRs: Readable<Incident[]> = derived(
	incidents,
	$incidents => $incidents.filter(i => i.status === 'pr_open')
);

export const projectsWithIssues: Readable<Set<string>> = derived(
	[incidents],
	([$incidents]) => {
		const projectIds = new Set<string>();
		$incidents.forEach(i => {
			if (['detected', 'triaging', 'fixing'].includes(i.status)) {
				projectIds.add(i.project_id);
			}
		});
		return projectIds;
	}
);

// Triage queue - incidents waiting for or undergoing triage
export const triageQueue: Readable<{
	waiting: Incident[];
	inProgress: Incident[];
	total: number;
}> = derived(
	incidents,
	$incidents => {
		const waiting = $incidents.filter(i => i.status === 'detected');
		const inProgress = $incidents.filter(i => i.status === 'triaging' || i.status === 'fixing');
		return {
			waiting,
			inProgress,
			total: waiting.length + inProgress.length
		};
	}
);

// Subscribe to demo mode changes to reload data
demoMode.subscribe(() => {
	// Only reload if we're in the browser
	if (typeof window !== 'undefined') {
		projects.load();
		incidents.load();
		stats.load();
		auth.load();
		usage.load();
	}
});
