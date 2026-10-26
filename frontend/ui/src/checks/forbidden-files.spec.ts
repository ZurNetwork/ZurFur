import { describe, expect, it } from 'vitest';
import { readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const srcRoot = fileURLToPath(new URL('../', import.meta.url));

/**
 * A file name that runs outside the static pages, ships a server route, or is
 * JSX, which no lint set, scan or tsconfig here reads.
 */
const forbiddenNames: readonly RegExp[] = [
	/^\+server\./,
	/\.server\./,
	/^hooks\./,
	/^service-worker\./,
	/\.[jt]sx$/i
];

/** A folder SvelteKit treats as the service worker, or that holds the server half. */
const forbiddenFolders: ReadonlySet<string> = new Set(['service-worker', 'server']);

/** The paths, relative to src/, that the package must never hold. */
function forbiddenFiles(paths: readonly string[]): readonly string[] {
	return paths.filter((relative) => {
		const segments = relative.split(/[/\\]/);
		const name = segments.at(-1) ?? '';
		const folders = segments.slice(0, -1);
		const hasForbiddenName = forbiddenNames.some((pattern) => pattern.test(name));
		const hasForbiddenFolder = folders.some((folder) => forbiddenFolders.has(folder));
		return hasForbiddenName || hasForbiddenFolder;
	});
}

function realFiles(): readonly string[] {
	return readdirSync(srcRoot, { recursive: true, withFileTypes: true })
		.filter((entry) => entry.isFile())
		.map((entry) => path.relative(srcRoot, path.join(entry.parentPath, entry.name)));
}

describe('forbidden files', () => {
	it('lists the real tree', () => {
		expect(realFiles()).toContain(path.join('lib', 'index.ts'));
	});

	it('passes the real tree', () => {
		expect(forbiddenFiles(realFiles())).toEqual([]);
	});

	it.each([
		['a server route', 'routes/+server.ts'],
		['a nested server route', 'routes/components/button/+server.js'],
		['a server load', 'routes/+page.server.ts'],
		['a server module', 'lib/data.server.ts'],
		['server hooks', 'hooks.server.ts'],
		['client hooks', 'hooks.client.ts'],
		['universal hooks', 'hooks.ts'],
		['hooks anywhere', 'lib/hooks.js'],
		['a service worker', 'service-worker.ts'],
		['a service worker in JavaScript', 'service-worker.js'],
		['a service worker folder', 'service-worker/index.ts'],
		['a server folder', 'lib/server/x.ts'],
		['a nested server folder', 'lib/a/server/b/x.svelte'],
		['a TSX file', 'lib/x.tsx'],
		['a JSX file', 'lib/X.jsx'],
		['a TSX page', 'routes/+page.tsx'],
		['a JSX file in upper case', 'lib/X.JSX']
	])('refuses %s', (_label, relative) => {
		expect(forbiddenFiles([relative])).toEqual([relative]);
	});

	it.each([
		['a component', 'lib/Button.svelte'],
		['a page', 'routes/+page.svelte'],
		['a universal load', 'routes/+layout.ts'],
		['a name holding "server" mid-word', 'lib/observer.ts'],
		['a folder named like a server', 'lib/servers/x.ts']
	])('allows %s', (_label, relative) => {
		expect(forbiddenFiles([relative])).toEqual([]);
	});
});
