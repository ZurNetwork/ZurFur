import { describe, expect, it } from 'vitest';
import { readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const srcRoot = fileURLToPath(new URL('../', import.meta.url));

/**
 * A file or folder name that runs outside the static pages, ships a server
 * route, or is JSX, which no lint set, scan or tsconfig here reads. Each is
 * tested against every segment of the path, since Kit also reads a folder's
 * `index` as hooks or the service worker.
 */
const forbiddenNames: readonly RegExp[] = [
	/^\+server\./,
	/\.server\./,
	/^hooks\./,
	/^service-worker\./,
	/\.[jt]sx$/i
];

/** A folder SvelteKit treats as hooks or the service worker, or that holds the server half. */
const forbiddenFolders: ReadonlySet<string> = new Set(['hooks', 'service-worker', 'server']);

/**
 * The extensions a file under src/ may have, read after the last dot of its
 * name and matched case-sensitively: the code (`.ts`, which `.d.ts`,
 * `.svelte.ts` and `.spec.ts` end in, and `.svelte`), the stylesheets the scans
 * read (`.css`), the node files (`.json`) and Kit's page shell (`.html`). Any
 * other file, or one with no extension, is refused: Vite or Kit may still build
 * it in while no lint or scan reads it. A new kind is added here deliberately,
 * with the lint or scan that reads it.
 */
const allowedExtensions: ReadonlySet<string> = new Set([
	'.ts',
	'.svelte',
	'.css',
	'.json',
	'.html'
]);

function hasAllowedExtension(name: string): boolean {
	const dot = name.lastIndexOf('.');
	return dot !== -1 && allowedExtensions.has(name.slice(dot));
}

/** The paths, relative to src/, that the package must never hold. */
function forbiddenFiles(paths: readonly string[]): readonly string[] {
	return paths.filter((relative) => {
		const segments = relative.split(/[/\\]/);
		const name = segments.at(-1) ?? '';
		const folders = segments.slice(0, -1);
		const hasForbiddenName = segments.some((segment) =>
			forbiddenNames.some((pattern) => pattern.test(segment))
		);
		const hasForbiddenFolder = folders.some((folder) => forbiddenFolders.has(folder));
		return hasForbiddenName || hasForbiddenFolder || !hasAllowedExtension(name);
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
		['a JSX file in upper case', 'lib/X.JSX'],
		['an unread extension', 'lib/evil.es'],
		['a code extension in upper case', 'lib/evil.JS'],
		['TypeScript in mixed case', 'lib/x.Ts'],
		['a component in mixed case', 'lib/X.Svelte'],
		['a text file', 'lib/notes.txt'],
		['an ES module in JavaScript', 'lib/x.mjs'],
		['a file with no extension', 'hooks'],
		['client hooks as a folder', 'hooks.client/index.ts'],
		['server hooks as a folder', 'hooks.server/index.ts'],
		['universal hooks as a folder', 'hooks/index.ts'],
		['a hooks folder anywhere', 'lib/hooks.client/x.ts']
	])('refuses %s', (_label, relative) => {
		expect(forbiddenFiles([relative])).toEqual([relative]);
	});

	it.each([
		['a component', 'lib/Button.svelte'],
		['a page', 'routes/+page.svelte'],
		['a universal load', 'routes/+layout.ts'],
		['a name holding "server" mid-word', 'lib/observer.ts'],
		['a folder named like a server', 'lib/servers/x.ts'],
		['the page shell', 'app.html'],
		['a type declaration', 'lib/x.d.ts'],
		['a runes module', 'lib/x.svelte.ts'],
		['a stylesheet', 'lib/styles/tokens.css'],
		['a node file', 'NODE.inf.json'],
		['a nested node file', 'lib/a/NODE.inf.json']
	])('allows %s', (_label, relative) => {
		expect(forbiddenFiles([relative])).toEqual([]);
	});
});
