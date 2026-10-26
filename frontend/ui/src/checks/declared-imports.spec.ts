import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { parse } from 'svelte/compiler';
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));
const srcRoot = path.join(uiRoot, 'src');

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

/** The package names one manifest section lists. */
function namesIn(manifest: Fields, key: string): ReadonlySet<string> {
	const section = manifest[key];
	return new Set(isFields(section) ? Object.keys(section) : []);
}

interface Manifest {
	readonly name: string;
	/** `dependencies` and `peerDependencies`: what component code may import. */
	readonly runtime: ReadonlySet<string>;
	/** `devDependencies`: what only the host, the checks and the specs may import. */
	readonly dev: ReadonlySet<string>;
	/** The specifiers the package's own exports map answers: its name, and `name/sub` for each `./sub`. */
	readonly exported: ReadonlySet<string>;
}

/**
 * The specifiers an exports map answers, read as Node reads it: a string, or
 * an object of conditions alone, stands for `.`; otherwise each `.` or `./sub`
 * key. A key with a `*` names a pattern, which no specifier here matches.
 */
function exportedSpecifiers(name: string, exportsField: unknown): ReadonlySet<string> {
	if (typeof exportsField === 'string') return new Set([name]);
	if (!isFields(exportsField)) return new Set();
	const keys = Object.keys(exportsField);
	if (keys.every((key) => !key.startsWith('.'))) return new Set([name]);
	const subpaths = keys.filter((key) => key === '.' || key.startsWith('./'));
	return new Set(subpaths.map((key) => (key === '.' ? name : `${name}/${key.slice(2)}`)));
}

function manifestOf(parsed: unknown): Manifest {
	if (!isFields(parsed) || typeof parsed.name !== 'string') {
		throw new Error('package.json has no name');
	}
	const runtime = new Set([
		...namesIn(parsed, 'dependencies'),
		...namesIn(parsed, 'peerDependencies')
	]);
	return {
		name: parsed.name,
		runtime,
		dev: namesIn(parsed, 'devDependencies'),
		exported: exportedSpecifiers(parsed.name, parsed.exports)
	};
}

function readManifest(): Manifest {
	const parsed: unknown = JSON.parse(readFileSync(path.join(uiRoot, 'package.json'), 'utf8'));
	return manifestOf(parsed);
}

/** Which rules a file answers to, by where it sits under src/. */
type FileKind = 'component' | 'host' | 'check';

function kindOf(relative: string): FileKind {
	const posix = relative.split(path.sep).join('/');
	const isCheck =
		posix.startsWith('checks/') || posix.endsWith('.spec.ts') || posix.endsWith('.svelte.spec.ts');
	if (isCheck) return 'check';
	const isHost = ['routes/', 'gallery/', 'stories/'].some((folder) => posix.startsWith(folder));
	if (isHost) return 'host';
	return 'component';
}

/** The kinds, strictest first: a file never imports from a looser kind. */
const kindsStrictestFirst: readonly FileKind[] = ['component', 'host', 'check'];

const codeExtensions = ['.ts', '.mts', '.cts', '.js', '.mjs', '.cjs', '.svelte'] as const;

/**
 * The loosest kind a relative import of `relative` can land on: the path as
 * written, with each code extension added (a TypeScript `.js` read as `.ts`
 * too), and as a folder's index.
 */
function loosestKindOf(relative: string): FileKind {
	const asTypeScript = relative.replace(/\.([mc]?)js$/, '.$1ts');
	const candidates = [
		relative,
		asTypeScript,
		...codeExtensions.map((extension) => `${relative}${extension}`),
		...codeExtensions.map((extension) => path.join(relative, `index${extension}`))
	];
	const ranks = candidates.map((candidate) => kindsStrictestFirst.indexOf(kindOf(candidate)));
	return kindsStrictestFirst[Math.max(...ranks)] ?? 'check';
}

/** Queries that make Vite hand back a file's text or URL, never run it. */
const inertQueries: readonly string[] = ['?raw', '?url'];

/**
 * True when an import of `relative` with `suffix` (its query and hash, as
 * written) names a file no lint set reads: the last segment has an extension,
 * after its last dot and matched case-sensitively, that is neither code nor
 * `.css`, and the suffix is not exactly `?raw` or `?url`.
 */
function isUnlintedTarget(relative: string, suffix: string): boolean {
	if (inertQueries.includes(suffix)) return false;
	const name = path.basename(relative);
	const dot = name.lastIndexOf('.');
	if (dot === -1) return false;
	const extension = name.slice(dot);
	const isCode = codeExtensions.some((code) => code === extension);
	return extension !== '.css' && !isCode;
}

function isLooser(target: FileKind, importer: FileKind): boolean {
	return kindsStrictestFirst.indexOf(target) > kindsStrictestFirst.indexOf(importer);
}

/** Kit's virtual modules and ui's aliases: lint decides who may use them. */
const aliasPattern = /^\$(app|env|lib|gallery)(\/|$)/;

/** The files outside src/ that a check may import. */
const checkImportableFiles = [
	'vite.config.ts',
	'eslint.config.js',
	'package.json',
	'gallery-port.ts',
	'.storybook/main.ts'
].map((file) => path.join(uiRoot, file));

function packageName(specifier: string): string {
	const parts = specifier.split('/');
	const isScoped = specifier.startsWith('@');
	return (isScoped ? parts.slice(0, 2) : parts.slice(0, 1)).join('/');
}

function withoutExtension(file: string): string {
	return file.replace(/\.(ts|js|json)$/, '');
}

function isCheckImportable(resolved: string): boolean {
	return checkImportableFiles.some(
		(file) => file === resolved || withoutExtension(file) === resolved
	);
}

interface ImportRefusal {
	readonly file: string;
	readonly specifier: string;
	readonly reason: string;
}

interface SourceFile {
	/** The path relative to src/. */
	readonly relative: string;
	readonly text: string;
}

/** Why one import is refused, or undefined when it is allowed. */
function refusalOf(file: SourceFile, specifier: string, manifest: Manifest): string | undefined {
	const kind = kindOf(file.relative);
	if (specifier.startsWith('/')) return 'a path starting with /';
	if (specifier.startsWith('.')) {
		// Vite drops the query and the hash before it resolves a path.
		const bare = specifier.replace(/[?#].*$/s, '');
		const suffix = specifier.slice(bare.length);
		const resolved = path.resolve(srcRoot, path.dirname(file.relative), bare);
		const insideSrc = resolved.startsWith(srcRoot + path.sep);
		if (insideSrc) {
			const relative = path.relative(srcRoot, resolved);
			if (isLooser(loosestKindOf(relative), kind)) {
				return 'a relative import into a looser file set';
			}
			const isUnlinted = isUnlintedTarget(relative, suffix);
			return isUnlinted && isLooser('check', kind)
				? 'a relative import of a file lint does not read'
				: undefined;
		}
		if (kind === 'check' && isCheckImportable(resolved)) return undefined;
		return 'a relative path outside src';
	}
	if (specifier.startsWith('node:')) {
		return kind === 'check' ? undefined : 'a Node built-in outside the checks and specs';
	}
	if (aliasPattern.test(specifier)) return undefined;
	const name = packageName(specifier);
	if (name === manifest.name) {
		return manifest.exported.has(specifier) ? undefined : 'a path the package does not export';
	}
	if (manifest.runtime.has(name)) return undefined;
	if (manifest.dev.has(name)) {
		return kind === 'component' ? 'a dev-only package in component code' : undefined;
	}
	return 'a package ui does not declare';
}

/** The script blocks of a `.svelte` file, as plain text. */
function svelteScripts(text: string): readonly string[] {
	const ast = parse(text, { modern: true });
	return [ast.module, ast.instance].flatMap((script) => {
		// The compiler answers undefined, not the null its types promise, for a missing block.
		if (!script) return [];
		const program = script.content;
		if (!('start' in program) || typeof program.start !== 'number') {
			throw new Error('a svelte script has no start');
		}
		if (!('end' in program) || typeof program.end !== 'number') {
			throw new Error('a svelte script has no end');
		}
		return [text.slice(program.start, program.end)];
	});
}

/** Every module specifier a file imports, re-exports or loads dynamically. */
function specifiersOf(file: SourceFile): readonly string[] {
	const scripts = file.relative.endsWith('.svelte') ? svelteScripts(file.text) : [file.text];
	return scripts.flatMap((script) =>
		ts.preProcessFile(script, true, true).importedFiles.map((imported) => imported.fileName)
	);
}

function importRefusals(
	files: readonly SourceFile[],
	manifest: Manifest
): readonly ImportRefusal[] {
	return files.flatMap((file) =>
		specifiersOf(file).flatMap((specifier) => {
			const reason = refusalOf(file, specifier, manifest);
			return reason === undefined ? [] : [{ file: file.relative, specifier, reason }];
		})
	);
}

const sourcePattern = /\.(ts|mts|cts|js|mjs|cjs|svelte)$/;

function realSourceFiles(): readonly SourceFile[] {
	return readdirSync(srcRoot, { recursive: true, encoding: 'utf8' })
		.filter((relative) => sourcePattern.test(relative))
		.map((relative) => ({
			relative,
			text: readFileSync(path.join(srcRoot, relative), 'utf8')
		}));
}

const manifest = readManifest();

function refusalsFor(relative: string, text: string): readonly ImportRefusal[] {
	const file: SourceFile = { relative, text };
	return importRefusals([file], manifest);
}

/** ui's manifest with the two subpaths the host imports in its exports map. */
const manifestWithSubpaths = manifestOf({
	name: manifest.name,
	peerDependencies: Object.fromEntries([...manifest.runtime].map((name) => [name, '*'])),
	devDependencies: Object.fromEntries([...manifest.dev].map((name) => [name, '*'])),
	exports: {
		'.': { types: './src/lib/index.ts', svelte: './src/lib/index.ts' },
		'./styles': { types: './src/lib/styles/index.ts', svelte: './src/lib/styles/index.ts' },
		'./tokens.json': './src/lib/styles/tokens.json',
		'./tokens.additions.json': './src/lib/styles/tokens.additions.json'
	}
});

function reasonsWithSubpaths(relative: string, text: string): readonly string[] {
	const file: SourceFile = { relative, text };
	return importRefusals([file], manifestWithSubpaths).map((refusal) => refusal.reason);
}

describe('declared imports', () => {
	it('finds the imports of the real tree, svelte scripts included', () => {
		const svelteImports = realSourceFiles()
			.filter((file) => file.relative.endsWith('.svelte'))
			.flatMap(specifiersOf);
		expect(svelteImports).toContain('svelte');
	});

	it('passes the real tree', () => {
		expect(importRefusals(realSourceFiles(), manifest)).toEqual([]);
	});

	it.each([
		['an undeclared package', 'lib/x.ts', "import { superForm } from 'sveltekit-superforms';"],
		['a dev-only package in component code', 'lib/x.ts', "import { expect } from 'vitest';"],
		['a dev-only package in a lazy module', 'lib/lazy/x.ts', "export * from 'vitest';"],
		['a path starting with /', 'lib/x.ts', "import '/src/x';"],
		['a path starting with / in a check', 'checks/x.spec.ts', "import '/src/x';"],
		['a dynamic import starting with /', 'lib/x.ts', "export const m = import('/src/x');"],
		['a path through node_modules', 'lib/x.ts', "import '../../node_modules/web/x';"],
		['a path into web', 'lib/x.ts', "import '../../../web/src/lib/server/x';"],
		['a check importing outside ui', 'checks/x.spec.ts', "import '../../../web/vite.config';"],
		['a Node built-in in component code', 'lib/x.ts', "import { readFileSync } from 'node:fs';"],
		['a Node built-in in the host', 'gallery/x.ts', "import path from 'node:path';"],
		['a Node built-in named without node:', 'checks/x.spec.ts', "import fs from 'fs';"]
	])('refuses %s', (_label, relative, text) => {
		expect(refusalsFor(relative, text)).toHaveLength(1);
	});

	const svelteReexport =
		'<script lang="ts" module>\n\texport { styleFindings } from \'../checks/scan/style\';\n</script>\n';

	it.each([
		[
			"the public entry's export * of a check",
			'lib/index.ts',
			"export * from '../checks/scan/style';"
		],
		[
			"the public entry's named re-export of a check",
			'lib/index.ts',
			"export { styleFindings } from '../checks/scan/style';"
		],
		['a component importing a page', 'lib/x.ts', "import '../routes/+layout';"],
		['a component importing the frame', 'lib/x.ts', "import '../gallery/registry';"],
		['a component importing a spec', 'lib/x.ts', "export { y } from './x.spec';"],
		['a type-only import of a check', 'lib/x.ts', "import type { T } from '../checks/x';"],
		['a dynamic import of a page', 'lib/x.ts', "export const r = import('../routes/x');"],
		['a type query of a check', 'lib/x.ts', "export type T = typeof import('../checks/x');"],
		['a check by folder', 'lib/x.ts', "import '../checks';"],
		['a check with a query', 'lib/x.ts', "import text from '../checks/x.ts?raw';"],
		['a spec named with .js', 'lib/x.ts', "import './x.spec.js';"],
		['the motion adapter importing a page', 'lib/motion/x.ts', "import '../../routes/x';"],
		['a stray file importing a page', 'foo.ts', "import './routes/x';"],
		['the frame importing a check', 'gallery/x.ts', "import '../checks/x';"],
		['a page importing a spec', 'routes/x.ts', "import '../gallery/x.spec';"],
		['a component importing a story', 'lib/x.ts', "import '../stories/frame/x';"],
		["a component's script re-exporting a check", 'lib/X.svelte', svelteReexport]
	])('refuses %s, a relative import into a looser file set', (_label, relative, text) => {
		expect(refusalsFor(relative, text).map((refusal) => refusal.reason)).toEqual([
			'a relative import into a looser file set'
		]);
	});

	it.each([
		['a re-export of an unread extension', 'lib/index.ts', "export * from './evil.es';"],
		['an upper-case code extension', 'lib/x.ts', "import './evil.JS';"],
		['an asset with no query', 'lib/x.ts', "import './a.svg';"],
		['an asset with another query', 'lib/x.ts', "import u from './a.svg?inline';"],
		['a dotted TypeScript name without its extension', 'lib/x.ts', "import './a.b';"],
		['the frame importing an unread extension', 'gallery/x.ts', "import './x.es';"]
	])('refuses %s, a relative import of a file lint does not read', (_label, relative, text) => {
		expect(refusalsFor(relative, text).map((refusal) => refusal.reason)).toEqual([
			'a relative import of a file lint does not read'
		]);
	});

	it.each([
		['a component importing the motion adapter', 'lib/x.ts', "import { x } from './motion/x';"],
		['a component importing the lazy loader', 'lib/x.ts', "import { x } from './lazy/x';"],
		['the motion adapter importing a component', 'lib/motion/x.ts', "import { x } from '../x';"],
		['a page importing a component', 'routes/x.ts', "import { x } from '../lib/x';"],
		['a page importing the frame', 'routes/x.ts', "import { x } from '../gallery/x';"],
		['the frame importing a page', 'gallery/x.ts', "import { x } from '../routes/x';"],
		['a check importing a component', 'checks/x.ts', "import { x } from '../lib/x';"],
		['a spec importing its sibling', 'lib/x.spec.ts', "import { x } from './x';"],
		['a component importing a raw text file', 'lib/x.ts', "import text from './a.txt?raw';"],
		['a component importing an asset URL', 'lib/x.ts', "import u from './a.svg?url';"],
		['a component importing a stylesheet', 'lib/x.ts', "import './styles/tokens.css';"],
		['a component importing an extensionless file', 'lib/x.ts', "import x from './y';"],
		['a dotted TypeScript name with its extension', 'lib/x.ts', "import './a.b.js';"],
		['a check importing an asset', 'checks/x.spec.ts', "import './a.svg';"]
	])('allows %s', (_label, relative, text) => {
		expect(refusalsFor(relative, text)).toEqual([]);
	});

	describe("the package's own exports", () => {
		it.each([
			['the styles in a route', 'routes/+layout.ts', "import '@zurfur/ui/styles';"],
			['the styles in the frame', 'gallery/x.ts', "import '@zurfur/ui/styles';"],
			[
				'the tokens in a route',
				'routes/components/colour/x.ts',
				"import tokens from '@zurfur/ui/tokens.json';"
			],
			['the tokens in the frame', 'gallery/x.ts', "import tokens from '@zurfur/ui/tokens.json';"],
			[
				'the token additions in the frame',
				'gallery/x.ts',
				"import additions from '@zurfur/ui/tokens.additions.json';"
			],
			['the public entry in a route', 'routes/x.ts', "import * as ui from '@zurfur/ui';"]
		])('allows %s, once the exports map names it', (_label, relative, text) => {
			expect(reasonsWithSubpaths(relative, text)).toEqual([]);
		});

		it.each([
			['a file inside the styles', "import '@zurfur/ui/src/lib/styles/tokens.css';"],
			['a path below the styles', "import '@zurfur/ui/styles/x';"],
			['the tokens with a query', "import tokens from '@zurfur/ui/tokens.json?raw';"],
			['the additions with a query', "import a from '@zurfur/ui/tokens.additions.json?raw';"],
			['the styles in another letter case', "import '@zurfur/ui/Styles';"],
			['the styles with a trailing slash', "import '@zurfur/ui/styles/';"]
		])('refuses %s, a path the package does not export', (_label, text) => {
			expect(reasonsWithSubpaths('routes/x.ts', text)).toEqual([
				'a path the package does not export'
			]);
		});

		it("refuses a subpath the real exports map doesn't name", () => {
			const reasons = refusalsFor('routes/x.ts', "import '@zurfur/ui/src/lib/x';").map(
				(refusal) => refusal.reason
			);
			expect(reasons).toEqual(['a path the package does not export']);
		});

		it.each([
			['a string', './src/lib/index.ts', ['@zurfur/ui']],
			['conditions alone', { types: './a.ts', svelte: './a.ts' }, ['@zurfur/ui']],
			[
				'subpaths',
				{ '.': './a.ts', './styles': './s.ts', './tokens.json': './t.json' },
				['@zurfur/ui', '@zurfur/ui/styles', '@zurfur/ui/tokens.json']
			],
			['no exports map', undefined, []]
		])('reads an exports map of %s', (_label, exportsField, expected) => {
			expect([...exportedSpecifiers('@zurfur/ui', exportsField)]).toEqual(expected);
		});
	});

	it('refuses an undeclared package in a svelte script', () => {
		const text =
			'<script lang="ts">\n\timport { superForm } from \'sveltekit-superforms\';\n</script>\n';
		expect(refusalsFor('lib/X.svelte', text)).toEqual([
			{
				file: 'lib/X.svelte',
				specifier: 'sveltekit-superforms',
				reason: 'a package ui does not declare'
			}
		]);
	});

	it.each([
		['a peer dependency in component code', 'lib/x.ts', "import { untrack } from 'svelte';"],
		['a relative file inside src', 'lib/x.ts', "import { y } from './y';"],
		['a dev-only package in a spec', 'lib/x.spec.ts', "import { expect } from 'vitest';"],
		['a dev-only package in the host', 'gallery/x.ts', "import { page } from 'vitest/browser';"],
		['a dev-only package in a story', 'stories/x.ts', "import { fn } from 'storybook/test';"],
		[
			'a check importing the main Storybook config',
			'checks/x.spec.ts',
			"import '../../.storybook/main.ts';"
		],
		['a check importing the port rule', 'checks/x.spec.ts', "import '../../gallery-port.ts';"],
		['a Node built-in in a check', 'checks/x.spec.ts', "import path from 'node:path';"],
		['the package itself', 'routes/x.ts', "import * as ui from '@zurfur/ui';"],
		['a Kit alias', 'gallery/x.ts', "import { resolve } from '$app/paths';"],
		['a check importing the vite config', 'checks/x.spec.ts', "import '../../vite.config';"],
		['a check importing the lint config', 'checks/x.spec.ts', "import '../../eslint.config.js';"],
		['a check importing the manifest', 'checks/x.spec.ts', "import '../../package.json';"]
	])('allows %s', (_label, relative, text) => {
		expect(refusalsFor(relative, text)).toEqual([]);
	});
});
