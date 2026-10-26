import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import ts from 'typescript';
import config, { galleryPort } from '../../vite.config.ts';

const loopback = '127.0.0.1';

/** Keys whose presence lets any website read the dev or preview server. */
const openingKeys = ['allowedHosts', 'cors'] as const;

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

/** Why a `server` or `preview` block would be reachable beyond loopback, or bind the wrong port. */
function serverRefusals(block: unknown, expectedPort: number): readonly string[] {
	if (!isFields(block)) return ['the block is missing'];
	const refusals: string[] = [];
	if (block.host !== loopback) refusals.push(`host is ${JSON.stringify(block.host)}`);
	if (block.strictPort !== true) refusals.push('strictPort is not true');
	if (block.port !== expectedPort) refusals.push(`port is ${JSON.stringify(block.port)}`);
	for (const key of openingKeys) {
		if (Object.hasOwn(block, key)) refusals.push(`${key} is set`);
	}
	return refusals;
}

const hostOrPortFlag = /--(?:host|port)/i;

/** The header and body lines of a Justfile recipe, or undefined when it has none. */
function recipeLines(justfile: string, recipe: string): readonly string[] | undefined {
	const lines = justfile.split('\n');
	const header = new RegExp(`^@?${recipe}(?:\\s[^:]*)?:(?!=)`);
	const start = lines.findIndex((line) => header.test(line));
	if (start === -1) return undefined;
	const rest = lines.slice(start + 1);
	const end = rest.findIndex((line) => line.trim() !== '' && !/^\s/.test(line));
	const body = end === -1 ? rest : rest.slice(0, end);
	return [lines[start] ?? '', ...body];
}

function justfileRefusals(justfile: string): readonly string[] {
	const lines = recipeLines(justfile, 'gallery');
	if (lines === undefined) return ['the gallery recipe is missing'];
	return lines.filter((line) => hostOrPortFlag.test(line)).map((line) => line.trim());
}

const servingScripts = ['dev', 'build', 'preview'] as const;

function scriptRefusals(manifest: unknown): readonly string[] {
	const scripts = isFields(manifest) ? manifest.scripts : undefined;
	if (!isFields(scripts)) return ['scripts are missing'];
	return servingScripts.flatMap((name) => {
		const script = scripts[name];
		if (typeof script !== 'string') return [`${name} is missing`];
		return hostOrPortFlag.test(script) ? [`${name}: ${script}`] : [];
	});
}

const repoJustfile = readFileSync(new URL('../../../../Justfile', import.meta.url), 'utf8');
const uiManifest: unknown = JSON.parse(
	readFileSync(new URL('../../package.json', import.meta.url), 'utf8')
);

const port = galleryPort(process.env.ZURFUR_GALLERY_PORT);

describe('galleryPort', () => {
	it.each([undefined, '', 'abc', '0', '-1', '65536', '5190.5', '0x1F90', ' 6000', '6000 '])(
		'falls back to 5190 for %j',
		(raw) => {
			expect(galleryPort(raw)).toBe(5190);
		}
	);

	it.each([
		['6000', 6000],
		['1', 1],
		['65535', 65535]
	])('reads %j as %i', (raw, expected) => {
		expect(galleryPort(raw)).toBe(expected);
	});
});

describe('the serve config', () => {
	it.each(['server', 'preview'] as const)('binds %s to loopback on the gallery port', (key) => {
		expect(serverRefusals(config[key], port)).toEqual([]);
	});

	const goodBlock = { host: loopback, port: 5190, strictPort: true };

	it.each([
		['every interface', { ...goodBlock, host: '0.0.0.0' }],
		['host: true', { ...goodBlock, host: true }],
		['localhost', { ...goodBlock, host: 'localhost' }],
		['IPv6 any', { ...goodBlock, host: '::' }],
		['no host', { port: 5190, strictPort: true }],
		['a loose port', { ...goodBlock, strictPort: false }],
		['no strictPort', { host: loopback, port: 5190 }],
		['another port', { ...goodBlock, port: 5191 }],
		['allowedHosts: true', { ...goodBlock, allowedHosts: true }],
		['an allowedHosts list', { ...goodBlock, allowedHosts: ['.example.org'] }],
		['cors: true', { ...goodBlock, cors: true }],
		['cors: false', { ...goodBlock, cors: false }],
		['no block', undefined]
	])('refuses %s', (_name, block) => {
		expect(serverRefusals(block, 5190)).not.toEqual([]);
	});

	it('passes a loopback block', () => {
		expect(serverRefusals(goodBlock, 5190)).toEqual([]);
	});
});

/**
 * Names the vite config never holds, as a key, a property access, an identifier or a
 * string: `files` lets Kit rename the hooks and the service worker past the
 * forbidden-files check, and `configFile` makes vite-plugin-svelte read a svelte
 * config file after all.
 */
const refusedNames: ReadonlySet<string> = new Set(['files', 'configFile']);

/** True for any spelling of "service worker": the package has none, so the config names none. */
function mentionsServiceWorker(text: string): boolean {
	return text
		.toLowerCase()
		.replace(/[\s_-]/g, '')
		.includes('serviceworker');
}

const kitPluginModule = '@sveltejs/kit/vite';
const kitPlugin = 'sveltekit';

/** Every node under `node`, depth first. */
function descendants(node: ts.Node): readonly ts.Node[] {
	const found: ts.Node[] = [];
	ts.forEachChild(node, (child) => {
		found.push(child, ...descendants(child));
	});
	return found;
}

/** The text of a node that can spell a name: an identifier, a string or a template part. */
function spelledText(node: ts.Node): string | undefined {
	if (ts.isIdentifier(node) || ts.isPrivateIdentifier(node)) return node.text;
	if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text;
	if (ts.isTemplateHead(node) || ts.isTemplateMiddle(node) || ts.isTemplateTail(node)) {
		return node.text;
	}
	return undefined;
}

function isStringText(node: ts.Node): boolean {
	return ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node);
}

function where(source: ts.SourceFile, node: ts.Node): string {
	const { line } = source.getLineAndCharacterOfPosition(node.getStart(source));
	return `line ${String(line + 1)}`;
}

function nameRefusals(source: ts.SourceFile): readonly string[] {
	return descendants(source).flatMap((node) => {
		const text = spelledText(node);
		if (text === undefined) return [];
		if (refusedNames.has(text)) return [`${text} at ${where(source, node)}`];
		if (mentionsServiceWorker(text)) return [`a service worker at ${where(source, node)}`];
		return [];
	});
}

/** The plugin module is named once: in the plain named import `{ sveltekit }`. */
function pluginImportRefusals(source: ts.SourceFile): readonly string[] {
	const mentions = descendants(source).filter(
		(node) => isStringText(node) && spelledText(node) === kitPluginModule
	);
	if (mentions.length !== 1) {
		return [`${kitPluginModule} is named ${String(mentions.length)} times`];
	}
	const declaration = mentions[0]?.parent;
	if (declaration === undefined || !ts.isImportDeclaration(declaration)) {
		return [`${kitPluginModule} is loaded other than by an import declaration`];
	}
	const clause = declaration.importClause;
	const bindings = clause?.namedBindings;
	const [binding, ...others] =
		bindings !== undefined && ts.isNamedImports(bindings) ? bindings.elements : [];
	const isPlain =
		clause?.name === undefined &&
		clause?.phaseModifier === undefined &&
		binding !== undefined &&
		others.length === 0 &&
		!binding.isTypeOnly &&
		binding.propertyName === undefined &&
		binding.name.text === kitPlugin;
	return isPlain ? [] : [`${kitPluginModule} is not imported as plain { ${kitPlugin} }`];
}

/** Why one object under the plugin's options could hide a key from the name scan. */
function objectRefusals(
	source: ts.SourceFile,
	object: ts.ObjectLiteralExpression
): readonly string[] {
	return object.properties.flatMap((property) => {
		if (ts.isSpreadAssignment(property)) return [`a spread at ${where(source, property)}`];
		const key = property.name;
		const isOpaqueKey = ts.isComputedPropertyName(key) && !isStringText(key.expression);
		return isOpaqueKey ? [`a key computed from an expression at ${where(source, key)}`] : [];
	});
}

/**
 * `sveltekit` is called once, with one object literal, and every object under it
 * spells out its keys, so none can hide from the name scan.
 */
function pluginCallRefusals(source: ts.SourceFile): readonly string[] {
	const references = descendants(source).filter(
		(node): node is ts.Identifier =>
			ts.isIdentifier(node) && node.text === kitPlugin && !ts.isImportSpecifier(node.parent)
	);
	const calls = references.flatMap((reference) => {
		const call = reference.parent;
		return ts.isCallExpression(call) && call.expression === reference ? [call] : [];
	});
	if (calls.length !== references.length) return [`${kitPlugin} is used other than by a call`];
	const [call, ...extraCalls] = calls;
	if (call === undefined || extraCalls.length > 0) {
		return [`${kitPlugin} is called ${String(calls.length)} times, not once`];
	}
	const [options, ...extraArguments] = call.arguments;
	if (
		options === undefined ||
		extraArguments.length > 0 ||
		!ts.isObjectLiteralExpression(options)
	) {
		return [`the ${kitPlugin}(…) argument is not one object literal`];
	}
	return [options, ...descendants(options)]
		.filter(ts.isObjectLiteralExpression)
		.flatMap((object) => objectRefusals(source, object));
}

/** Why the vite config could rename Kit's hooks or service worker, or load a svelte config file. */
function configRefusals(text: string): readonly string[] {
	const source = ts.createSourceFile('vite.config.ts', text, ts.ScriptTarget.Latest, true);
	return [...nameRefusals(source), ...pluginImportRefusals(source), ...pluginCallRefusals(source)];
}

/**
 * Kit's `load_config`, behind `svelte-kit sync`, prerender and analyse, reads
 * `svelte.config.{js,ts}` whenever loading the vite config fails, and
 * vite-plugin-svelte reads one unless told not to; any of them could rename files.
 */
const svelteConfigFile = /^svelte\.config\.[mc]?[jt]s$/i;

function svelteConfigRefusals(names: readonly string[]): readonly string[] {
	return names.filter((name) => svelteConfigFile.test(name));
}

const viteConfigText = readFileSync(new URL('../../vite.config.ts', import.meta.url), 'utf8');
const uiFolderNames = readdirSync(new URL('../../', import.meta.url));

describe("Kit's inline options", () => {
	it('passes the real vite config', () => {
		expect(configRefusals(viteConfigText)).toEqual([]);
	});

	/** A whole vite config around one `sveltekit(…)` argument, with optional lines before it. */
	const planted = (options: string, prelude = ''): string =>
		[
			"import { defineConfig } from 'vitest/config';",
			"import { sveltekit } from '@sveltejs/kit/vite';",
			prelude,
			`export default defineConfig({ plugins: [sveltekit(${options})] });`
		].join('\n');

	const realOptions =
		"{ compilerOptions: { runes: ({ filename }) => (filename.includes('x') ? undefined : true) }, adapter: adapter({ strict: true }), alias: { $gallery: 'src/gallery' } }";
	const importsOptions = "import options from './kit.config.js';";

	it('passes options shaped like the real ones', () => {
		expect(configRefusals(planted(realOptions))).toEqual([]);
	});

	const arg = 'argument is not one object literal';
	const notPlain = 'is not imported as plain { sveltekit }';

	it.each([
		[
			'files.serviceWorker',
			planted("{ files: { serviceWorker: 'src/sw.ts' } }"),
			'files at line 4'
		],
		['files.hooks.client', planted("{ files: { hooks: { client: 'src/c.ts' } } }"), 'files at'],
		[
			'files.hooks.universal',
			planted("{ files: { hooks: { universal: 'src/u.ts' } } }"),
			'files at'
		],
		['a string key files', planted("{ 'files': {} }"), 'files at'],
		['a computed string key files', planted("{ ['files']: {} }"), 'files at'],
		['a template key files', planted('{ [`files`]: {} }'), 'files at'],
		['a shorthand files', planted('{ files }', "const files = { routes: 'src/r' };"), 'files at'],
		['files set outside the call', planted('{}', "const kit = {}; kit['files'] = {};"), 'files at'],
		[
			"Kit's serviceWorker option",
			planted('{ serviceWorker: { register: true } }'),
			'a service worker'
		],
		['a serviceWorker string', planted('{}', "const sw = 'serviceWorker';"), 'a service worker'],
		[
			'a service-worker path',
			planted('{}', "const sw = 'src/service-worker.ts';"),
			'a service worker'
		],
		[
			'a serviceWorker template',
			planted('{}', 'const sw = `${1}serviceWorker`;'),
			'a service worker'
		],
		[
			'a svelte config file for vite-plugin-svelte',
			planted("{ vitePlugin: { configFile: 'x.js' } }"),
			'configFile at'
		],
		['a spread of the options', planted('{ ...options }', importsOptions), 'a spread at line 4'],
		['options from a variable', planted('options', importsOptions), arg],
		['options from a call', planted('Object.assign({}, options)', importsOptions), arg],
		['no options', planted(''), arg],
		['a second argument', planted('{}, options', importsOptions), arg],
		[
			'a spread inside compilerOptions',
			planted('{ compilerOptions: { ...compiler } }', "import compiler from './c.js';"),
			'a spread at'
		],
		[
			'a key computed from a name',
			planted('{ [key]: {} }', "import { key } from './k.js';"),
			'a key computed from an expression'
		],
		[
			'a second sveltekit call',
			planted('{}', `const second = sveltekit(options);\n${importsOptions}`),
			'sveltekit is called 2 times'
		],
		[
			'sveltekit passed on uncalled',
			planted('{}', `const kit = sveltekit;\n${importsOptions}`),
			'sveltekit is used other than by a call'
		],
		[
			'sveltekit imported under another name',
			"import { sveltekit as kit } from '@sveltejs/kit/vite';\nexport default { plugins: [kit({})] };",
			notPlain
		],
		[
			'the plugin module as a namespace',
			"import * as kit from '@sveltejs/kit/vite';\nexport default { plugins: [kit.sveltekit({})] };",
			notPlain
		],
		[
			'a type-only import',
			"import type { sveltekit } from '@sveltejs/kit/vite';\nexport default { plugins: [sveltekit({})] };",
			notPlain
		],
		[
			'the plugin module loaded dynamically',
			planted('{}', "const late = await import('@sveltejs/kit/vite');"),
			'@sveltejs/kit/vite is named 2 times'
		]
	])('refuses %s', (_label, text, reason) => {
		expect(configRefusals(text)).toContainEqual(expect.stringContaining(reason));
	});
});

describe('the svelte config file', () => {
	it('is absent: Kit falls back to it whenever loading the vite config fails', () => {
		expect(svelteConfigRefusals(uiFolderNames)).toEqual([]);
	});

	it.each([
		'svelte.config.js',
		'svelte.config.mjs',
		'svelte.config.cjs',
		'svelte.config.ts',
		'svelte.config.mts',
		'svelte.config.cts',
		'Svelte.Config.JS'
	])('refuses %s', (name) => {
		expect(svelteConfigRefusals(['package.json', name, 'vite.config.ts'])).toEqual([name]);
	});

	it('passes the files a package holds', () => {
		expect(
			svelteConfigRefusals(['package.json', 'vite.config.ts', 'svelte.config.json', 'src'])
		).toEqual([]);
	});
});

describe('the gallery recipe', () => {
	it('exists and passes no host or port', () => {
		expect(recipeLines(repoJustfile, 'gallery')).toBeDefined();
		expect(justfileRefusals(repoJustfile)).toEqual([]);
	});

	const recipe = (line: string): string =>
		`other:\n    echo --host\n\ngallery:\n    just frontend-install\n${line}\n\nlater:\n    echo --port\n`;

	it.each([
		'    yarn --cwd frontend/ui run preview --host',
		'    yarn --cwd frontend/ui run preview --host=0.0.0.0',
		'    yarn --cwd frontend/ui run preview --host 0.0.0.0',
		'    yarn --cwd frontend/ui run preview --port 80',
		'    yarn --cwd frontend/ui run preview --port=80',
		'\tyarn --cwd frontend/ui run preview --HOST'
	])('refuses %j', (line) => {
		expect(justfileRefusals(recipe(line))).not.toEqual([]);
	});

	it('refuses a flag on the recipe line itself', () => {
		expect(justfileRefusals('gallery: (serve "--host")\n    echo\n')).not.toEqual([]);
	});

	it('refuses a missing recipe', () => {
		expect(justfileRefusals('galleryx:\n    echo\ngallery-build:\n    echo\n')).toEqual([
			'the gallery recipe is missing'
		]);
	});

	it("passes flags in other recipes and the recipe's own clean lines", () => {
		expect(justfileRefusals(recipe('    yarn --cwd frontend/ui run preview'))).toEqual([]);
	});
});

describe("ui's serving scripts", () => {
	it('pass no host or port', () => {
		expect(scriptRefusals(uiManifest)).toEqual([]);
	});

	const manifest = { scripts: { dev: 'vite dev', build: 'vite build', preview: 'vite preview' } };

	it.each([
		['dev', 'vite dev --host'],
		['build', 'vite build --port=1'],
		['preview', 'vite preview --host=0.0.0.0'],
		['preview', 'vite preview --port 80']
	])('refuses %s: %j', (name, script) => {
		const flagged = { scripts: { ...manifest.scripts, [name]: script } };
		expect(scriptRefusals(flagged)).toEqual([`${name}: ${script}`]);
	});

	it('refuses a missing script', () => {
		expect(scriptRefusals({ scripts: { dev: 'vite dev', build: 'vite build' } })).toEqual([
			'preview is missing'
		]);
	});

	it('passes the plain scripts', () => {
		expect(scriptRefusals(manifest)).toEqual([]);
	});
});
