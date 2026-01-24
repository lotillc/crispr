import adapter from '@sveltejs/adapter-static';

// Read base path from environment variable for GitHub Pages deployment
const basePath = process.env.PUBLIC_BASE_PATH || '';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: 'index.html',
			precompress: false,
			strict: true
		}),
		paths: {
			base: basePath
		}
	}
};

export default config;
