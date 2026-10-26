import { afterAll, describe, expect, it } from 'vitest';
import {
	lstatSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	rmSync,
	symlinkSync,
	writeFileSync
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';

/**
 * src/ as the runner reached it, from the spec's own path: Vite resolves
 * `import.meta.url` through links, so a symlinked src/ would read as its target.
 */
function srcRoot(): string {
	const { testPath } = expect.getState();
	if (testPath === undefined) throw new Error('no test path outside a test');
	return path.resolve(path.dirname(testPath), '..');
}

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

/**
 * A folder SvelteKit treats as hooks or the service worker, one that holds the
 * server half, or a `node_modules` folder, which ESLint skips and through which
 * Vite resolves a bare import before it reaches the declared packages.
 */
const forbiddenFolders: ReadonlySet<string> = new Set([
	'hooks',
	'service-worker',
	'server',
	'node_modules'
]);

/**
 * The extensions a file under src/ may have, read after the last dot of its
 * name and matched case-sensitively: the code (`.ts`, which `.d.ts`,
 * `.svelte.ts` and `.spec.ts` end in, and `.svelte`) and the stylesheets the
 * scans read (`.css`). Any other file, or one with no extension, is refused:
 * Vite or Kit may still build it in while no lint or scan reads it. A new kind
 * is added here deliberately, with the lint or scan that reads it.
 */
const allowedExtensions: ReadonlySet<string> = new Set(['.ts', '.svelte', '.css']);

/**
 * The JSON file a folder may hold, its node (the two token files below are the
 * only others). Any other `.json` could be a `package.json` or `tsconfig.json`
 * that sends an import outside src/.
 */
const nodeFileName = 'NODE.inf.json';

/** Kit's page shell, the one HTML file, and only at src/'s root. */
const pageShell = 'app.html';

/**
 * The two token files, the only other JSON files, allowed by exact path
 * relative to src/. `tokens.json` is read by the copy step and its match spec,
 * the generator, and, through the package's `./tokens.json` export, the
 * gallery's token pages; `tokens.additions.json` by the generator, its spec
 * and, through the package's `./tokens.additions.json` export, the same pages.
 */
const tokenFiles: ReadonlySet<string> = new Set([
	'lib/styles/tokens.json',
	'lib/styles/tokens.additions.json'
]);

/** A runes module, a `.svelte.ts` or `.svelte.js`: only the motion adapter keeps one. */
const runesModule = /\.svelte\.[jt]s$/;

/** The one folder, relative to src/, whose files may be runes modules. */
const runesFolder = 'lib/motion/';

function isAllowedFile(relative: string, name: string): boolean {
	if (name === nodeFileName || relative === pageShell) return true;
	if (tokenFiles.has(relative)) return true;
	const dot = name.lastIndexOf('.');
	return dot !== -1 && allowedExtensions.has(name.slice(dot));
}

/**
 * The paths, relative to src/ and `/`-separated, that the package must never
 * hold. A backslash in any segment is refused: on Linux it is part of a name,
 * but it reads as a separator elsewhere, so such a name could pass as another path.
 */
function forbiddenFiles(paths: readonly string[]): readonly string[] {
	return paths.filter((relative) => {
		const segments = relative.split('/');
		const name = segments.at(-1) ?? '';
		const folders = segments.slice(0, -1);
		const hasForbiddenName = segments.some((segment) =>
			forbiddenNames.some((pattern) => pattern.test(segment))
		);
		const hasForbiddenFolder = folders.some((folder) => forbiddenFolders.has(folder));
		const hasBackslash = segments.some((segment) => segment.includes('\\'));
		const isStrayRunesModule = runesModule.test(name) && !relative.startsWith(runesFolder);
		return (
			hasForbiddenName ||
			hasForbiddenFolder ||
			hasBackslash ||
			isStrayRunesModule ||
			!isAllowedFile(relative, name)
		);
	});
}

/** What an entry is in itself, read with lstat so a link is never followed. */
type EntryKind = 'file' | 'folder' | 'other';

function kindOf(absolute: string): EntryKind {
	const stats = lstatSync(absolute);
	if (stats.isFile()) return 'file';
	if (stats.isDirectory()) return 'folder';
	return 'other';
}

interface Tree {
	/** The plain files, relative to the root. */
	readonly files: readonly string[];
	/** Every entry that is neither a plain file nor a real folder, relative to the root; `.` is the root itself. */
	readonly strays: readonly string[];
}

/**
 * Walks `root` without following links, giving `/`-separated paths: it
 * descends only into real folders, and a symlink (to a file or a folder),
 * FIFO, socket or device is a stray, since no lint or scan walks into it
 * while Vite and TypeScript follow it.
 */
function walkTree(root: string): Tree {
	// A trailing slash would make lstat follow a link at the root itself.
	const base = path.resolve(root);
	if (kindOf(base) !== 'folder') return { files: [], strays: ['.'] };
	const files: string[] = [];
	const strays: string[] = [];
	const visit = (folder: string): void => {
		for (const name of readdirSync(path.join(base, folder))) {
			const relative = folder === '' ? name : `${folder}/${name}`;
			const kind = kindOf(path.join(base, relative));
			if (kind === 'folder') visit(relative);
			else if (kind === 'file') files.push(relative);
			else strays.push(relative);
		}
	};
	visit('');
	return { files: files.toSorted(), strays: strays.toSorted() };
}

describe('the tree under src/', () => {
	const scratch = mkdtempSync(path.join(os.tmpdir(), 'ui-forbidden-files-'));
	const plantedSrc = path.join(scratch, 'src');
	const outside = path.join(scratch, 'outside');
	mkdirSync(path.join(plantedSrc, 'lib'), { recursive: true });
	mkdirSync(outside);
	writeFileSync(path.join(plantedSrc, 'lib', 'index.ts'), "export * from './vendor';\n");
	writeFileSync(path.join(outside, 'index.ts'), "export const r = () => fetch('/x');\n");
	symlinkSync('../../outside', path.join(plantedSrc, 'lib', 'vendor'), 'dir');
	symlinkSync('../../outside/index.ts', path.join(plantedSrc, 'lib', 'linked.ts'), 'file');
	symlinkSync('../nowhere.ts', path.join(plantedSrc, 'lib', 'dangling.ts'), 'file');
	const linkedSrc = path.join(scratch, 'linked-src');
	symlinkSync('src', linkedSrc, 'dir');

	afterAll(() => {
		rmSync(scratch, { recursive: true, force: true });
	});

	it('holds only plain files and real folders in the real tree', () => {
		expect(walkTree(srcRoot()).strays).toEqual([]);
	});

	it('lists the real tree', () => {
		expect(walkTree(srcRoot()).files).toContain('lib/index.ts');
	});

	it('refuses a symlinked folder, a symlinked file and a dangling link, and walks into none', () => {
		const tree = walkTree(plantedSrc);
		const expected: Tree = {
			files: ['lib/index.ts'],
			strays: ['lib/dangling.ts', 'lib/linked.ts', 'lib/vendor']
		};
		expect(tree).toEqual(expected);
	});

	it.each([
		['a symlinked root', linkedSrc],
		['a symlinked root with a trailing slash', `${linkedSrc}${path.sep}`]
	])('refuses %s', (_label, root) => {
		expect(walkTree(root)).toEqual({ files: [], strays: ['.'] });
	});
});

describe('forbidden files', () => {
	it('passes the real tree', () => {
		expect(forbiddenFiles(walkTree(srcRoot()).files)).toEqual([]);
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
		['a hooks folder anywhere', 'lib/hooks.client/x.ts'],
		['a package manifest', 'lib/vendor/package.json'],
		['a tsconfig', 'lib/tsconfig.json'],
		['any other JSON file', 'lib/x.json'],
		['another JSON file beside the tokens', 'lib/styles/other.json'],
		['the token file name one folder up', 'lib/tokens.json'],
		['the token file name one folder down', 'lib/styles/nested/tokens.json'],
		['the token file name in the gallery', 'routes/tokens.json'],
		['the token additions file name in the gallery', 'routes/tokens.additions.json'],
		['an HTML file in lib', 'lib/evil.html'],
		['a page shell below the root', 'routes/app.html'],
		['a page shell in another case', 'App.html'],
		['a node_modules folder', 'lib/node_modules/x/index.ts'],
		['a node_modules folder at the root', 'node_modules/svelte/index.ts'],
		['a token file path written with backslashes as one name', 'lib\\styles\\tokens.json'],
		['a backslash inside a file name', 'lib/x\\y.ts'],
		['a backslash inside a folder name', 'lib/x\\y/z.ts'],
		['a runes module in the seam', 'lib/environment/x.svelte.ts'],
		['a runes module in a control', 'lib/controls/state.svelte.ts'],
		['a runes module in the gallery', 'gallery/skin-store.svelte.ts'],
		['a runes module at the root', 'x.svelte.ts'],
		['a runes module named like the adapter', 'lib/motionx/x.svelte.ts'],
		['a JavaScript runes module in the adapter', 'lib/motion/x.svelte.js']
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
		['a runes module in the motion adapter', 'lib/motion/x.svelte.ts'],
		['a nested runes module in the motion adapter', 'lib/motion/deep/x.svelte.ts'],
		['a component spec', 'lib/controls/Toggle.svelte.spec.ts'],
		['a stylesheet', 'lib/styles/tokens.css'],
		['the token file', 'lib/styles/tokens.json'],
		['the token additions file', 'lib/styles/tokens.additions.json'],
		['a node file', 'NODE.inf.json'],
		['a nested node file', 'lib/a/NODE.inf.json']
	])('allows %s', (_label, relative) => {
		expect(forbiddenFiles([relative])).toEqual([]);
	});
});
