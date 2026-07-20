import { defineConfig } from 'vitest/config';
import { playwright } from '@vitest/browser-playwright';
import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';

// The dev server port Caddy proxies its catch-all to (ZMVP-150). 5174, not the
// Vite default 5173 — that belongs to frontend/auth. strictPort makes the port
// deterministic so Caddy's upstream can't silently drift to an incremented port.
// Any value that isn't a real TCP port falls back — unset/empty (Number('') is
// 0, a random ephemeral bind that would 502 Caddy's fixed upstream) but also
// garbage like '-1' or '5174.5', which would otherwise fail the dev server
// confusingly instead of falling back.
const parsedWebPort = Number(process.env.ZURFUR_WEB_PORT);
const parsedWebPortIsValid =
	Number.isInteger(parsedWebPort) && parsedWebPort >= 1 && parsedWebPort <= 65535;
const webPort = parsedWebPortIsValid ? parsedWebPort : 5174;

export default defineConfig({
	// Bind IPv4 loopback explicitly: Vite's default `localhost` resolves to IPv6
	// `::1` on some hosts, but the Caddyfile upstream (and axum) are 127.0.0.1, so
	// an IPv6-only bind makes Caddy's catch-all 502. Pin both ends to 127.0.0.1.
	server: { host: '127.0.0.1', port: webPort, strictPort: true },
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter()
		})
	],
	test: {
		expect: { requireAssertions: true },
		projects: [
			{
				extends: './vite.config.ts',
				test: {
					name: 'client',
					browser: {
						enabled: true,
						provider: playwright(),
						instances: [{ browser: 'chromium', headless: true }]
					},
					include: ['src/**/*.svelte.{test,spec}.{js,ts}'],
					exclude: ['src/lib/server/**']
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
