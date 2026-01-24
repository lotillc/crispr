import { writable } from 'svelte/store';

export type ToastType = 'success' | 'error' | 'warning' | 'info';

export interface Toast {
	id: string;
	message: string;
	type: ToastType;
	duration: number;
}

function createToastStore() {
	const { subscribe, update } = writable<Toast[]>([]);

	function generateId(): string {
		return `toast-${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
	}

	function addToast(message: string, type: ToastType = 'info', duration: number = 5000): string {
		const id = generateId();
		const toast: Toast = { id, message, type, duration };

		update(toasts => [...toasts, toast]);

		// Auto-dismiss after duration
		if (duration > 0) {
			setTimeout(() => {
				dismissToast(id);
			}, duration);
		}

		return id;
	}

	function dismissToast(id: string): void {
		update(toasts => toasts.filter(t => t.id !== id));
	}

	function clearAll(): void {
		update(() => []);
	}

	return {
		subscribe,
		addToast,
		dismissToast,
		clearAll,
		// Convenience methods
		success: (message: string, duration?: number) => addToast(message, 'success', duration),
		error: (message: string, duration?: number) => addToast(message, 'error', duration),
		warning: (message: string, duration?: number) => addToast(message, 'warning', duration),
		info: (message: string, duration?: number) => addToast(message, 'info', duration)
	};
}

export const toasts = createToastStore();
