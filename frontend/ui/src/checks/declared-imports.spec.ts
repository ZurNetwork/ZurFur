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
}

function readManifest(): Manifest {
	const parsed: unknown = JSON.parse(readFileSync(path.join(uiRoot, 'package.json'), 'utf8'));
	if (!isFields(parsed) || typeof parsed.name !== 'string') {
		throw new Error('package.json has no name');
	}
	const runtime = new Set([
		...namesIn(parsed, 'dependencies'),
		...namesIn(parsed, 'peerDependencies')
	]);
	return { name: parsed.name, runtime, dev: namesIn(parsed, 'devDependencies') };
}

/** Which rules a file answers to, by where it sits under src/. */
type FileKind = 'component' | 'host' | 'check';

function kindOf(relative: string): FileKind {
	const posix = relative.split(path.sep).join('/');
	const isCheck =
		posix.startsWith('checks/') || posix.endsWith('.spec.ts') || posix.endsWith('.svelte.spec.ts');
	if (isCheck) return 'check';
	if (posix.startsWith('routes/') || posix.startsWith('gallery/')) return 'host';
	return 'component';
}

/** Kit's virtual modules and ui's aliases: lint decides who may use them. */
const aliasPattern = /^\$(app|env|lib|gallery)(\/|$)/;

/** The files outside src/ that a check may import. */
const checkImportableFiles = ['vite.config.ts', 'eslint.config.js', 'package.json'].map((file) =>
	path.join(uiRoot, file)
);

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
		const resolved = path.resolve(srcRoot, path.dirname(file.relative), specifier);
		const insideSrc = resolved.startsWith(srcRoot + path.sep);
		if (insideSrc) return undefined;
		if (kind === 'check' && isCheckImportable(resolved)) return undefined;
		return 'a relative path outside src';
	}
	if (specifier.startsWith('node:')) {
		return kind === 'check' ? undefined : 'a Node built-in outside the checks and specs';
	}
	if (aliasPattern.test(specifier)) return undefined;
	const name = packageName(specifier);
	if (name === manifest.name || manifest.runtime.has(name)) return undefined;
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
