import { defineConfig } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';

const fallbackGalleryPort = 5190;

/**
 * Reads the gallery's port from the raw `ZURFUR_GALLERY_PORT` text.
 * Only a plain decimal integer from 1 to 65535 counts; anything else, unset
 * included, falls back to 5190.
 */
export function galleryPort(raw: string | undefined): number {
	if (raw === undefined || !/^[0-9]+$/.test(raw)) return fallbackGalleryPort;
	const port = Number(raw);
	const isTcpPort = Number.isInteger(port) && port >= 1 && port <= 65535;
	return isTcpPort ? port : fallbackGalleryPort;
}

const port = galleryPort(process.env.ZURFUR_GALLERY_PORT);

export default defineConfig({
	// Both servers bind IPv4 loopback only, on a port that must be free: a busy
	// port is an error, never a silent move to the next one.
	server: { host: '127.0.0.1', port, strictPort: true },
	preview: { host: '127.0.0.1', port, strictPort: true },
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter({ strict: true }),
			alias: { $gallery: 'src/gallery' }
		})
	],
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				// The browser tests' own server inherits `server` above, and with it
				// `strictPort`; let it move to the next free port instead, so two
				// workspaces can run their suites at once.
				server: { strictPort: false },
				test: {
					name: 'client',
					browser: {
						enabled: true,
						provider: playwright(),
						instances: [{ browser: 'chromium', headless: true }]
					},
					include: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			},

			{
				extends: './vite.config.ts',
				test: {
					name: 'server',
					environment: 'node',
					include: ['src/**/*.{test,spec}.{js,ts}'],
					exclude: ['src/**/*.svelte.{test,spec}.{js,ts}']
				}
			}
		]
	}
});
