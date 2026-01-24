// Form validation helpers for CRISPR UI
// Uses Svelte 5 runes-compatible patterns

export interface ValidationResult {
	valid: boolean;
	error?: string;
}

/**
 * Validates that a required field is not empty
 * @param value - The value to validate
 * @param fieldName - Human-readable field name for error messages
 */
export function validateRequired(value: string, fieldName: string): ValidationResult {
	const trimmed = value?.trim() ?? '';
	if (!trimmed) {
		return {
			valid: false,
			error: `${fieldName} is required`
		};
	}
	return { valid: true };
}

/**
 * Validates URL format
 * @param value - The URL string to validate
 * @param required - Whether the field is required (default: false)
 */
export function validateUrl(value: string, required: boolean = false): ValidationResult {
	const trimmed = value?.trim() ?? '';

	// If not required and empty, it's valid
	if (!required && !trimmed) {
		return { valid: true };
	}

	// If required and empty, it's invalid
	if (required && !trimmed) {
		return {
			valid: false,
			error: 'URL is required'
		};
	}

	// Check URL format
	try {
		const url = new URL(trimmed);
		// Only allow http and https protocols
		if (!['http:', 'https:'].includes(url.protocol)) {
			return {
				valid: false,
				error: 'URL must use http or https protocol'
			};
		}
		return { valid: true };
	} catch {
		return {
			valid: false,
			error: 'Please enter a valid URL (e.g., https://example.com)'
		};
	}
}

/**
 * Validates repository name format (owner/repo)
 * @param value - The repository string to validate
 */
export function validateRepoName(value: string): ValidationResult {
	const trimmed = value?.trim() ?? '';

	if (!trimmed) {
		return {
			valid: false,
			error: 'Repository is required'
		};
	}

	// Check for owner/repo format
	const parts = trimmed.split('/');
	if (parts.length !== 2) {
		return {
			valid: false,
			error: 'Format should be owner/repository'
		};
	}

	const [owner, repo] = parts;

	// Validate owner
	if (!owner) {
		return {
			valid: false,
			error: 'Owner name is required'
		};
	}

	// Validate repo
	if (!repo) {
		return {
			valid: false,
			error: 'Repository name is required'
		};
	}

	// Check for valid characters (alphanumeric, hyphens, underscores, dots)
	// GitHub allows: a-z, A-Z, 0-9, -, _, .
	const validPattern = /^[a-zA-Z0-9._-]+$/;

	if (!validPattern.test(owner)) {
		return {
			valid: false,
			error: 'Owner can only contain letters, numbers, hyphens, underscores, and dots'
		};
	}

	if (!validPattern.test(repo)) {
		return {
			valid: false,
			error: 'Repository can only contain letters, numbers, hyphens, underscores, and dots'
		};
	}

	// Check for spaces
	if (trimmed.includes(' ')) {
		return {
			valid: false,
			error: 'Repository name cannot contain spaces'
		};
	}

	// Check maximum length (GitHub limits)
	if (owner.length > 39) {
		return {
			valid: false,
			error: 'Owner name cannot exceed 39 characters'
		};
	}

	if (repo.length > 100) {
		return {
			valid: false,
			error: 'Repository name cannot exceed 100 characters'
		};
	}

	return { valid: true };
}

/**
 * Validates API key format (basic non-empty check for now)
 * @param value - The API key to validate
 * @param provider - The LLM provider (anthropic, openai, ollama)
 */
export function validateApiKey(value: string, provider: string): ValidationResult {
	const trimmed = value?.trim() ?? '';

	// Ollama doesn't require an API key
	if (provider === 'ollama') {
		return { valid: true };
	}

	// For Anthropic and OpenAI, API key is required
	if (!trimmed) {
		return {
			valid: false,
			error: 'API key is required for this provider'
		};
	}

	// Basic format validation
	if (provider === 'anthropic' && !trimmed.startsWith('sk-ant-')) {
		// Anthropic keys typically start with sk-ant-
		// But we'll just warn, not block, as format might change
	}

	if (provider === 'openai' && !trimmed.startsWith('sk-')) {
		// OpenAI keys typically start with sk-
		// But we'll just warn, not block
	}

	return { valid: true };
}

/**
 * Validates model selection
 * @param value - The model name/id to validate
 */
export function validateModel(value: string): ValidationResult {
	const trimmed = value?.trim() ?? '';

	if (!trimmed) {
		return {
			valid: false,
			error: 'Model is required'
		};
	}

	return { valid: true };
}

/**
 * Validates provider selection
 * @param value - The provider to validate
 */
export function validateProvider(value: string): ValidationResult {
	const trimmed = value?.trim() ?? '';

	if (!trimmed) {
		return {
			valid: false,
			error: 'Provider is required'
		};
	}

	const validProviders = ['anthropic', 'openai', 'ollama'];
	if (!validProviders.includes(trimmed)) {
		return {
			valid: false,
			error: 'Please select a valid provider'
		};
	}

	return { valid: true };
}

/**
 * Creates a unique ID for accessibility (aria-describedby)
 * @param fieldName - The field name to create ID for
 */
export function createErrorId(fieldName: string): string {
	return `${fieldName.toLowerCase().replace(/\s+/g, '-')}-error`;
}
