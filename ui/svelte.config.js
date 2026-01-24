import adapter from '@sveltejs/adapter-static';

// Read base path from environment variable for GitHub Pages deployment
// Use BASE_PATH for build-time configuration
const dev = process.argv.includes('dev');
const basePath = dev ? '' : (process.env.BASE_PATH || '');

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: '404.html',
			precompress: false,
			strict: true
		}),
		paths: {
			base: basePath
		},
		prerender: {
			handleHttpError: 'warn'
		}
	}
};

export default config;
