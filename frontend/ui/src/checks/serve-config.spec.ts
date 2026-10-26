import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import ts from 'typescript';
import config from '../../vite.config.ts';
import { galleryPort } from '../../gallery-port.ts';

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

/** The values of `extends` that inherit the root config, and with it its loopback `server`. */
const inheritsRoot: readonly unknown[] = ['./vite.config.ts', true];

/** True when a project runs its tests in a browser, served by a Vite server of its own. */
function runsInBrowser(project: Fields): boolean {
	const test = isFields(project.test) ? project.test : {};
	const browser = isFields(test.browser) ? test.browser : {};
	return browser.enabled !== undefined && browser.enabled !== false;
}

/**
 * Why one Vitest project could serve beyond loopback. A browser project's
 * server takes the root `server` through `extends`, and Vitest lays the
 * project's own `server` and `browser.api` over it, so neither may name a
 * host or open the server to other origins. A project not written inline
 * (a path or a function) can't be read, so it is refused.
 */
function projectRefusals(project: unknown): readonly string[] {
	if (!isFields(project)) return ['a project not written inline'];
	if (!runsInBrowser(project)) return [];
	const refusals: string[] = [];
	if (!inheritsRoot.includes(project.extends)) refusals.push('it does not extend the root config');
	const server = isFields(project.server) ? project.server : {};
	for (const key of ['host', ...openingKeys]) {
		if (Object.hasOwn(server, key)) refusals.push(`it sets server.${key}`);
	}
	const test = isFields(project.test) ? project.test : {};
	const browser = isFields(test.browser) ? test.browser : {};
	const api = isFields(browser.api) ? browser.api : {};
	for (const key of ['host', ...openingKeys]) {
		if (Object.hasOwn(api, key)) refusals.push(`it sets browser.api.${key}`);
	}
	return refusals;
}

/** Keys that let a Vitest API server, the browser's or the UI's, write files or run tests on the host. */
const apiPowerKeys = ['allowWrite', 'allowExec'] as const;

/**
 * Why a `test` block lets an API server write or execute: `test.api` or
 * `test.browser.api` setting either key, whatever its value, in any project
 * or the root config.
 */
function apiPowerRefusals(block: unknown): readonly string[] {
	const test = isFields(block) && isFields(block.test) ? block.test : {};
	const browser = isFields(test.browser) ? test.browser : {};
	const apis = [
		['api', test.api],
		['browser.api', browser.api]
	] as const;
	return apis.flatMap(([where, api]) =>
		isFields(api)
			? apiPowerKeys
					.filter((key) => Object.hasOwn(api, key))
					.map((key) => `it sets ${where}.${key}`)
			: []
	);
}

function vitestProjects(viteConfig: unknown): readonly unknown[] {
	const test = isFields(viteConfig) ? viteConfig.test : undefined;
	const projects = isFields(test) ? test.projects : undefined;
	return Array.isArray(projects) ? projects : [];
}

describe("the browser tests' servers", () => {
	const projects = vitestProjects(config);

	it('finds a browser project to check', () => {
		const browserProjects = projects.filter(
			(project) => isFields(project) && runsInBrowser(project)
		);
		expect(browserProjects).not.toEqual([]);
	});

	it('serves every browser project on loopback, with no host of its own', () => {
		expect(projects.flatMap(projectRefusals)).toEqual([]);
	});

	it('lets no API server write files or run tests', () => {
		expect([config, ...projects].flatMap(apiPowerRefusals)).toEqual([]);
	});

	it.each([
		['allowWrite: true in a browser project', { test: { browser: { api: { allowWrite: true } } } }],
		['allowExec in a browser project', { test: { browser: { api: { allowExec: false } } } }],
		['allowWrite on the UI API', { test: { api: { allowWrite: true } } }],
		['allowExec on the UI API', { test: { api: { port: 1, allowExec: true } } }]
	])('refuses %s', (_label, project) => {
		expect(apiPowerRefusals(project)).not.toEqual([]);
	});

	it('passes an API on a port', () => {
		expect(apiPowerRefusals({ test: { api: 51204, browser: { api: { port: 1 } } } })).toEqual([]);
	});

	const browserProject = {
		extends: './vite.config.ts',
		server: { strictPort: false },
		test: { name: 'x', browser: { enabled: true } }
	};

	it.each([
		['a server host', { ...browserProject, server: { host: '0.0.0.0' } }, 'server.host'],
		['a loopback server host', { ...browserProject, server: { host: loopback } }, 'server.host'],
		['allowedHosts', { ...browserProject, server: { allowedHosts: true } }, 'server.allowedHosts'],
		[
			'a browser api host',
			{ ...browserProject, test: { browser: { enabled: true, api: { host: '0.0.0.0' } } } },
			'browser.api.host'
		],
		['no extends', { ...browserProject, extends: undefined }, 'does not extend'],
		['another config', { ...browserProject, extends: './other.config.ts' }, 'does not extend'],
		['a project by path', 'packages/*', 'not written inline']
	])('refuses %s', (_label, project, reason) => {
		expect(projectRefusals(project)).toContainEqual(expect.stringContaining(reason));
	});

	it.each([
		['a browser project like the real one', browserProject],
		[
			'a browser api on a port',
			{ ...browserProject, test: { browser: { enabled: true, api: 63315 } } }
		],
		['a node project with its own host', { server: { host: '0.0.0.0' }, test: { name: 'n' } }],
		['a disabled browser', { server: { host: '0.0.0.0' }, test: { browser: { enabled: false } } }]
	])('passes %s', (_label, project) => {
		expect(projectRefusals(project)).toEqual([]);
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

/**
 * The variables the guarded scripts pass through `env -i`, each with its exact
 * value; nothing else from the caller's environment reaches Storybook or
 * Vitest. The proxies point at a closed loopback port, in both letter cases
 * (undici reads the lower-case ones first), so a Node request to another host
 * fails; loopback stays direct; telemetry is off and its address closed too.
 */
const guardVariables: ReadonlyMap<string, string> = new Map([
	['PATH', '"$PATH"'],
	['HOME', '"$HOME"'],
	['ZURFUR_GALLERY_PORT', '"${ZURFUR_GALLERY_PORT-}"'],
	['NODE_USE_ENV_PROXY', '1'],
	['HTTP_PROXY', 'http://127.0.0.1:9'],
	['HTTPS_PROXY', 'http://127.0.0.1:9'],
	['http_proxy', 'http://127.0.0.1:9'],
	['https_proxy', 'http://127.0.0.1:9'],
	['NO_PROXY', '127.0.0.1,localhost,::1'],
	['no_proxy', '127.0.0.1,localhost,::1'],
	['STORYBOOK_DISABLE_TELEMETRY', '1'],
	['STORYBOOK_TELEMETRY_URL', 'http://127.0.0.1:9']
]);

/** The port rule's one spelling: the gallery's port, printed by `gallery-port.ts`. */
const portArgument = '"$(node gallery-port.ts)"';

/**
 * Each guarded script's program, its required arguments (a flag, or a flag
 * and its value) and the arguments it may also carry. Any other argument is
 * refused, so no flag that opens or reports can slip in beside the pinned ones.
 */
interface GuardedCommand {
	readonly program: readonly string[];
	readonly required: readonly (readonly string[])[];
	readonly optional: readonly (readonly string[])[];
}

const guardedScripts: Readonly<Record<string, GuardedCommand>> = {
	storybook: {
		program: ['storybook', 'dev'],
		required: [
			['-c', '.storybook'],
			['--host', '127.0.0.1'],
			['--port', portArgument],
			['--exact-port'],
			['--no-open'],
			['--no-version-updates'],
			['--disable-telemetry']
		],
		optional: [['--ci']]
	},
	'build-storybook': {
		program: ['storybook', 'build'],
		required: [['-c', '.storybook'], ['-o', 'storybook-static'], ['--disable-telemetry']],
		optional: [['--quiet']]
	},
	test: { program: ['vitest'], required: [['--run']], optional: [] },
	'test:unit': { program: ['vitest'], required: [], optional: [] }
};

/**
 * Splits a script into words at spaces, keeping double-quoted text (quotes
 * included) inside its word; undefined when a quote is left open.
 */
function scriptWords(script: string): readonly string[] | undefined {
	const words: string[] = [];
	let word = '';
	let quoted = false;
	for (const character of script) {
		if (character === '"') quoted = !quoted;
		if (character === ' ' && !quoted) {
			if (word !== '') words.push(word);
			word = '';
		} else {
			word += character;
		}
	}
	if (quoted) return undefined;
	if (word !== '') words.push(word);
	return words;
}

/**
 * Characters that would end the guarded command or start another, outside
 * the port rule: a second command after it runs with the caller's whole
 * environment.
 */
const commandBreaks = /[;&|`\n<>]|\$\(/;

const assignment = /^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/;

/** Why one script is not `env -i`, exactly the guard's variables, then its pinned command. */
function guardedScriptRefusals(script: string, command: GuardedCommand): readonly string[] {
	if (commandBreaks.test(script.replace(portArgument, ''))) {
		return ['it holds a character that ends or starts a command'];
	}
	const words = scriptWords(script);
	if (words === undefined) return ['it leaves a quote open'];
	const [env, clear, ...rest] = words;
	if (env !== 'env' || clear !== '-i') return ['it does not start with env -i'];
	const firstCommandWord = rest.findIndex((word) => !assignment.test(word));
	const assignments = firstCommandWord === -1 ? rest : rest.slice(0, firstCommandWord);
	const commandWords = firstCommandWord === -1 ? [] : rest.slice(firstCommandWord);
	const passed = assignments.map((word) => assignment.exec(word)?.slice(1) ?? []);
	const names = passed.map(([name]) => name ?? '');
	const variableRefusals = [
		...passed.flatMap(([name = '', value]) => {
			const expected = guardVariables.get(name);
			if (expected === undefined) return [`it passes ${name}`];
			return value === expected ? [] : [`it passes ${name}=${String(value)}`];
		}),
		...[...guardVariables.keys()]
			.filter((name) => !names.includes(name))
			.map((name) => `it does not pass ${name}`),
		...names
			.filter((name, index) => names.indexOf(name) !== index)
			.map((name) => `it passes ${name} twice`)
	];
	return [...variableRefusals, ...commandRefusals(commandWords, command)];
}

/** Why a command line is not the pinned program with its required arguments and nothing unknown. */
function commandRefusals(words: readonly string[], command: GuardedCommand): readonly string[] {
	const program = words.slice(0, command.program.length);
	if (program.join(' ') !== command.program.join(' ')) {
		return [`it runs ${program.join(' ') || 'nothing'}, not ${command.program.join(' ')}`];
	}
	const known = [...command.required, ...command.optional];
	const refusals: string[] = [];
	const seen: string[] = [];
	let at = command.program.length;
	while (at < words.length) {
		const flag = words[at] ?? '';
		const shape = known.find((candidate) => candidate[0] === flag);
		if (shape === undefined) {
			refusals.push(`it passes ${flag}`);
			at += 1;
			continue;
		}
		const given = words.slice(at, at + shape.length);
		if (given.join(' ') !== shape.join(' ')) refusals.push(`it passes ${given.join(' ')}`);
		if (seen.includes(flag)) refusals.push(`it passes ${flag} twice`);
		seen.push(flag);
		at += shape.length;
	}
	const missing = command.required
		.filter(([flag]) => flag !== undefined && !seen.includes(flag))
		.map((shape) => `it does not pass ${shape.join(' ')}`);
	return [...refusals, ...missing];
}

/** Words that start Storybook or Vitest: only a guarded script may name them. */
const guardedPrograms = /\b(?:storybook|vitest)\b/;

/** Why the ui scripts start Storybook or Vitest other than through the four guarded scripts. */
function guardedManifestRefusals(manifest: unknown): readonly string[] {
	const scripts = isFields(manifest) ? manifest.scripts : undefined;
	if (!isFields(scripts)) return ['scripts are missing'];
	const pinned = Object.entries(guardedScripts).flatMap(([name, command]) => {
		const script = scripts[name];
		if (typeof script !== 'string') return [`${name} is missing`];
		return guardedScriptRefusals(script, command).map((refusal) => `${name}: ${refusal}`);
	});
	const unguarded = Object.entries(scripts)
		.filter(([name]) => !Object.hasOwn(guardedScripts, name))
		.filter(([, script]) => typeof script !== 'string' || guardedPrograms.test(script))
		.map(([name]) => `${name} starts Storybook or Vitest outside the guard`);
	return [...pinned, ...unguarded];
}

describe("ui's guarded scripts", () => {
	it('run Storybook and Vitest only under env -i and the guard, with the pinned flags', () => {
		expect(guardedManifestRefusals(uiManifest)).toEqual([]);
	});

	const guard = [...guardVariables].map(([name, value]) => `${name}=${value}`).join(' ');
	const storybook = `env -i ${guard} storybook dev -c .storybook --host 127.0.0.1 --port ${portArgument} --exact-port --no-open --ci --no-version-updates --disable-telemetry`;
	const scripts = {
		dev: 'vite dev',
		storybook,
		'build-storybook': `env -i ${guard} storybook build -c .storybook -o storybook-static --disable-telemetry --quiet`,
		test: `env -i ${guard} vitest --run`,
		'test:unit': `env -i ${guard} vitest`
	};

	/** The manifest with one script changed. */
	const changed = (name: keyof typeof scripts, script: string): Fields => ({
		scripts: { ...scripts, [name]: script }
	});

	it('passes scripts like the real ones', () => {
		expect(guardedManifestRefusals({ scripts })).toEqual([]);
	});

	it.each([
		[
			'--host 0.0.0.0',
			changed('storybook', storybook.replace('--host 127.0.0.1', '--host 0.0.0.0'))
		],
		['--exact-port dropped', changed('storybook', storybook.replace(' --exact-port', ''))],
		[
			'--no-version-updates dropped',
			changed('storybook', storybook.replace(' --no-version-updates', ''))
		],
		[
			'--disable-telemetry dropped',
			changed('storybook', storybook.replace(' --disable-telemetry', ''))
		],
		['--no-open dropped', changed('storybook', storybook.replace(' --no-open', ''))],
		['a second host', changed('storybook', `${storybook} --host 0.0.0.0`)],
		['an unknown flag', changed('storybook', `${storybook} --https`)],
		['another port rule', changed('storybook', storybook.replace(portArgument, '6006'))],
		['another config folder', changed('storybook', storybook.replace('-c .storybook', '-c ../sb'))],
		['the guard dropped from test', changed('test', 'vitest --run')],
		['env -i dropped', changed('test', `${guard} vitest --run`)],
		['env without -i', changed('test', `env ${guard} vitest --run`)],
		[
			'a lower-case proxy dropped',
			changed('test', scripts.test.replace(' https_proxy=http://127.0.0.1:9', ''))
		],
		[
			'no_proxy dropped',
			changed('test:unit', scripts['test:unit'].replace(' no_proxy=127.0.0.1,localhost,::1', ''))
		],
		[
			'a proxy pointed elsewhere',
			changed(
				'test',
				scripts.test.replace(
					'HTTPS_PROXY=http://127.0.0.1:9',
					'HTTPS_PROXY=http://proxy.example:8080'
				)
			)
		],
		[
			'telemetry back on',
			changed(
				'build-storybook',
				scripts['build-storybook'].replace(
					'STORYBOOK_DISABLE_TELEMETRY=1',
					'STORYBOOK_DISABLE_TELEMETRY=0'
				)
			)
		],
		[
			'an extra variable',
			changed('test', scripts.test.replace('env -i ', 'env -i DATABASE_URL="$DATABASE_URL" '))
		],
		[
			'a variable passed twice',
			changed('test', scripts.test.replace('vitest', 'HTTPS_PROXY= vitest'))
		],
		['a second command after the guarded one', changed('test', `${scripts.test} && vitest --run`)],
		['a command chained with a semicolon', changed('test', `${scripts.test}; vitest`)],
		['another substitution', changed('test', scripts.test.replace('"$PATH"', '"$(cat ~/.env)"'))],
		[
			'--disable-telemetry dropped from the build',
			changed('build-storybook', scripts['build-storybook'].replace(' --disable-telemetry', ''))
		],
		['another program', changed('test', scripts.test.replace('vitest --run', 'node x.js'))],
		['an open quote', changed('test', `${scripts.test} "`)],
		['a missing script', { scripts: { ...scripts, 'test:unit': undefined } }],
		[
			'an unguarded Storybook script',
			{ scripts: { ...scripts, sb: 'storybook dev --host 0.0.0.0' } }
		],
		['an unguarded Vitest script', { scripts: { ...scripts, coverage: 'vitest --coverage' } }]
	])('refuses %s', (_label, manifest) => {
		expect(guardedManifestRefusals(manifest)).not.toEqual([]);
	});
});
