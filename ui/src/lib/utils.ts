/**
 * CRISPR UI Shared Utilities
 * Centralized formatting and helper functions used across the application
 */

/**
 * Format a date string to a human-readable "time ago" string
 * @param date - ISO date string or Date object
 * @returns Human readable time ago (e.g., "just now", "5m ago", "2h ago", "3d ago")
 */
export function formatTimeAgo(date: string | Date): string {
	const dateObj = typeof date === 'string' ? new Date(date) : date;
	const now = new Date();
	const diffMs = now.getTime() - dateObj.getTime();
	const diffMins = Math.floor(diffMs / 60000);

	if (diffMins < 1) return 'just now';
	if (diffMins < 60) return `${diffMins}m ago`;
	const diffHours = Math.floor(diffMins / 60);
	if (diffHours < 24) return `${diffHours}h ago`;
	const diffDays = Math.floor(diffHours / 24);
	return `${diffDays}d ago`;
}

/**
 * Format a number as currency (USD)
 * @param amount - The amount to format
 * @param decimals - Number of decimal places (default: 2)
 * @returns Formatted currency string (e.g., "$12.34")
 */
export function formatCurrency(amount: number, decimals: number = 2): string {
	return `$${amount.toFixed(decimals)}`;
}

/**
 * Format a large number with K/M suffixes for readability
 * @param num - The number to format
 * @returns Formatted number string (e.g., "1.2K", "3.5M")
 */
export function formatNumber(num: number): string {
	if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
	if (num >= 1000) return `${(num / 1000).toFixed(1)}K`;
	return num.toString();
}

/**
 * Incident status values from the CRISPR API
 */
export type IncidentStatus =
	| 'detected'
	| 'triaging'
	| 'pending_fix'
	| 'fixing'
	| 'pr_open'
	| 'pr_merged'
	| 'unfixable'
	| 'max_attempts'
	| 'ignored';

/**
 * Map incident status to a human-readable display label
 * Uses medical terminology for the surgery theme
 * @param status - The incident status from the API
 * @returns Human readable status label
 */
export function getStatusLabel(status: IncidentStatus | string): string {
	switch (status) {
		case 'detected':
			return 'Waiting';
		case 'triaging':
			return 'Analyzing';
		case 'pending_fix':
			return 'Queued';
		case 'fixing':
			return 'Operating';
		case 'pr_open':
			return 'Awaiting Merge';
		case 'pr_merged':
			return 'Discharged';
		case 'unfixable':
			return 'Manual Review';
		case 'max_attempts':
			return 'Specialist Needed';
		case 'ignored':
			return 'Dismissed';
		default:
			return status;
	}
}

/**
 * Badge color variants matching the application's design system
 */
export type StatusColor =
	| 'success'
	| 'warning'
	| 'error'
	| 'info'
	| 'neutral'
	| 'surgery'
	| 'observation'
	| 'healthy'
	| 'critical'
	| 'discharged';

/**
 * Map incident status to a CSS color variant for styling badges
 * @param status - The incident status from the API
 * @returns CSS color variant name for use with Badge component
 */
export function getStatusColor(status: IncidentStatus | string): StatusColor {
	switch (status) {
		case 'detected':
		case 'pending_fix':
			return 'observation';
		case 'triaging':
		case 'fixing':
			return 'surgery';
		case 'pr_open':
			return 'healthy';
		case 'pr_merged':
			return 'discharged';
		case 'unfixable':
		case 'max_attempts':
			return 'critical';
		case 'ignored':
			return 'neutral';
		default:
			return 'neutral';
	}
}

/**
 * Service health status types
 */
export type ServiceStatus = 'healthy' | 'observation' | 'surgery' | 'critical';

/**
 * Map service health status to a human-readable label
 * @param status - The service status
 * @returns Human readable status label
 */
export function getServiceStatusLabel(status: ServiceStatus): string {
	switch (status) {
		case 'healthy':
			return 'Stable';
		case 'observation':
			return 'Under Observation';
		case 'surgery':
			return 'In Surgery';
		case 'critical':
			return 'Critical';
		default:
			return status;
	}
}

/**
 * Map service status to a CSS color variant
 * @param status - The service status
 * @returns CSS color variant name
 */
export function getServiceStatusColor(status: ServiceStatus): StatusColor {
	switch (status) {
		case 'healthy':
			return 'success';
		case 'observation':
			return 'info';
		case 'surgery':
			return 'warning';
		case 'critical':
			return 'error';
		default:
			return 'neutral';
	}
}

/**
 * Format a date to a localized string
 * @param date - ISO date string or Date object
 * @returns Localized date string
 */
export function formatDateTime(date: string | Date): string {
	const dateObj = typeof date === 'string' ? new Date(date) : date;
	return dateObj.toLocaleString();
}

/**
 * Clamp a number between min and max values
 * @param value - The value to clamp
 * @param min - Minimum value
 * @param max - Maximum value
 * @returns Clamped value
 */
export function clamp(value: number, min: number, max: number): number {
	return Math.min(Math.max(value, min), max);
}

/**
 * Get confidence level category from a confidence score
 * @param confidence - Confidence score (0-1)
 * @returns Confidence level category
 */
export function getConfidenceLevel(confidence: number): 'high' | 'medium' | 'low' {
	if (confidence >= 0.9) return 'high';
	if (confidence >= 0.7) return 'medium';
	return 'low';
}
