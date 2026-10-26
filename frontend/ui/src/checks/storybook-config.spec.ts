/*
 * Storybook's config, held to exactly what the gallery allows. The config
 * folder holds its four files and nothing Storybook would load by name;
 * `main.ts` holds exactly the allowed keys and values (no remote Storybooks,
 * no static folders, no injected head or body, no opened hosts, nothing
 * reported); the manager's brand title, which Storybook renders as HTML, is
 * one constant, with no brand image or link; and the preview's accessibility
 * rules and sidebar order are the gallery's.
 */
import { describe, expect, it } from 'vitest';
import { lstatSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';
import ts from 'typescript';
import config from '../../.storybook/main.ts';
import { groupNames } from '../gallery/groups.ts';

const configFolder = new URL('../../.storybook/', import.meta.url);

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

// --- The config folder -------------------------------------------------------

/**
 * The four files Storybook's config folder may hold. Storybook also loads
 * `middleware.{js,mjs,cjs}`, `preview-head.html`, `preview-body.html` and
 * `manager-head.html` from it with no key in `main.ts`, so any other entry,
 * a folder or a link included, is refused.
 */
const configFiles: readonly string[] = ['main.ts', 'preview.ts', 'manager.ts', 'vitest.setup.ts'];

type EntryKind = 'file' | 'folder' | 'other';

interface FolderEntry {
	readonly name: string;
	readonly kind: EntryKind;
}

/** What `entry` is in itself, read with lstat so a link is never followed. */
function kindOf(entry: string): EntryKind {
	const stats = lstatSync(entry);
	if (stats.isFile()) return 'file';
	if (stats.isDirectory()) return 'folder';
	return 'other';
}

/** Why the folder holds more, or less, than its four plain files. */
function folderRefusals(entries: readonly FolderEntry[]): readonly string[] {
	const extra = entries
		.filter((entry) => entry.kind !== 'file' || !configFiles.includes(entry.name))
		.map((entry) => `${entry.name} is a ${entry.kind} Storybook would not be told about`);
	const files = entries.filter((entry) => entry.kind === 'file').map((entry) => entry.name);
	const missing = configFiles
		.filter((name) => !files.includes(name))
		.map((name) => `${name} is missing`);
	return [...extra, ...missing];
}

describe("Storybook's config folder", () => {
	const fourFiles = configFiles.map((name): FolderEntry => ({ name, kind: 'file' }));

	it('holds exactly the four files', () => {
		const folder = fileURLToPath(configFolder);
		const entries = readdirSync(folder).map((name): FolderEntry => ({
			name,
			kind: kindOf(path.join(folder, name))
		}));
		expect(folderRefusals(entries)).toEqual([]);
	});

	it.each([
		['middleware.js', 'file'],
		['middleware.mjs', 'file'],
		['middleware.cjs', 'file'],
		['preview-head.html', 'file'],
		['preview-body.html', 'file'],
		['manager-head.html', 'file'],
		['addons', 'folder'],
		['theme.ts', 'other']
	] as const)('refuses %s beside main', (name, kind) => {
		expect(folderRefusals([...fourFiles, { name, kind }])).not.toEqual([]);
	});

	it('refuses a missing manager', () => {
		expect(folderRefusals(fourFiles.filter((entry) => entry.name !== 'manager.ts'))).toEqual([
			'manager.ts is missing'
		]);
	});
});

// --- main.ts -------------------------------------------------------------------

const allowedStories: readonly string[] = [
	'../src/stories/**/*.mdx',
	'../src/stories/**/*.stories.svelte'
];

const allowedAddons: readonly string[] = [
	'@storybook/addon-svelte-csf',
	'@storybook/addon-docs',
	'@storybook/addon-a11y',
	'@storybook/addon-vitest'
];

const allowedCore: Fields = {
	disableTelemetry: true,
	enableCrashReports: false,
	disableWhatsNewNotifications: true,
	disableProjectJson: true
};

const allowedFeatures: Fields = {
	sidebarOnboardingChecklist: false,
	menuOnboardingChecklist: false
};

/**
 * Each key `main.ts` may hold and the check its value must pass. `refs` must
 * be a function that returns `{}`: an object would be merged with the
 * Storybooks Storybook finds in the dependencies on its own, and only a
 * function replaces them. The server-channel hook must be a function; its
 * spec proves what it does.
 */
const mainKeys: Readonly<Record<string, (value: unknown) => boolean>> = {
	framework: (value) => value === '@storybook/sveltekit',
	stories: (value) => isDeepStrictEqual(value, allowedStories),
	addons: (value) => isDeepStrictEqual(value, allowedAddons),
	core: (value) => isDeepStrictEqual(value, allowedCore),
	features: (value) => isDeepStrictEqual(value, allowedFeatures),
	refs: (value) =>
		typeof value === 'function' && isDeepStrictEqual(Reflect.apply(value, undefined, []), {}),
	experimental_serverChannel: (value) => typeof value === 'function'
};

/** Why a main config holds a key, or a value, the gallery doesn't allow. */
function mainRefusals(main: unknown): readonly string[] {
	if (!isFields(main)) return ['the config is not an object'];
	const keys = Reflect.ownKeys(main).map(String);
	const unknown = keys
		.filter((key) => !Object.hasOwn(mainKeys, key))
		.map((key) => `${key} is not allowed`);
	const wrong = Object.entries(mainKeys).flatMap(([key, holds]) => {
		if (!Object.hasOwn(main, key)) return [`${key} is missing`];
		return holds(main[key]) ? [] : [`${key} is not the allowed value`];
	});
	return [...unknown, ...wrong];
}

describe("main.ts's keys and values", () => {
	it('holds exactly the allowed keys and values', () => {
		expect(mainRefusals(config)).toEqual([]);
	});

	const good: Fields = {
		framework: '@storybook/sveltekit',
		stories: [...allowedStories],
		addons: [...allowedAddons],
		core: { ...allowedCore },
		features: { ...allowedFeatures },
		refs: () => ({}),
		experimental_serverChannel: (channel: unknown) => channel
	};

	/** The good config without `key`. */
	const without = (key: string): Fields =>
		Object.fromEntries(Object.entries(good).filter(([name]) => name !== key));

	it('passes a config like the real one', () => {
		expect(mainRefusals(good)).toEqual([]);
	});

	it.each([
		['refs as an object', { ...good, refs: {} }, 'refs is not the allowed value'],
		['refs dropped', without('refs'), 'refs is missing'],
		['refs returning a Storybook', { ...good, refs: () => ({ x: { url: 'https://x' } }) }, 'refs'],
		['staticDirs', { ...good, staticDirs: ['public'] }, 'staticDirs is not allowed'],
		['previewHead', { ...good, previewHead: (head: string) => head }, 'previewHead is not allowed'],
		['managerHead', { ...good, managerHead: (head: string) => head }, 'managerHead is not allowed'],
		['previewBody', { ...good, previewBody: (body: string) => body }, 'previewBody is not allowed'],
		['env', { ...good, env: () => ({}) }, 'env is not allowed'],
		['viteFinal', { ...good, viteFinal: (vite: unknown) => vite }, 'viteFinal is not allowed'],
		['security', { ...good, security: {} }, 'security is not allowed'],
		['core.allowedHosts', { ...good, core: { ...allowedCore, allowedHosts: true } }, 'core'],
		[
			'disableTelemetry dropped',
			{
				...good,
				core: {
					enableCrashReports: false,
					disableWhatsNewNotifications: true,
					disableProjectJson: true
				}
			},
			'core'
		],
		['crash reports on', { ...good, core: { ...allowedCore, enableCrashReports: true } }, 'core'],
		['the checklist back', { ...good, features: { menuOnboardingChecklist: false } }, 'features'],
		['another framework', { ...good, framework: '@storybook/svelte-vite' }, 'framework'],
		[
			'framework options',
			{ ...good, framework: { name: '@storybook/sveltekit', options: {} } },
			'framework'
		],
		['another story glob', { ...good, stories: [...allowedStories, '../src/**/*.mdx'] }, 'stories'],
		['another addon', { ...good, addons: [...allowedAddons, '@storybook/addon-links'] }, 'addons'],
		['no server-channel hook', without('experimental_serverChannel'), 'experimental_serverChannel']
	])('refuses %s', (_label, main, reason) => {
		expect(mainRefusals(main)).toContainEqual(expect.stringContaining(reason));
	});
});

// --- manager.ts and preview.ts, read with TypeScript's parser ------------------

/** Every node under `node`, depth first. */
function descendants(node: ts.Node): readonly ts.Node[] {
	const found: ts.Node[] = [];
	ts.forEachChild(node, (child) => {
		found.push(child, ...descendants(child));
	});
	return found;
}

/** The text a node spells, when it is an identifier, a string or a template part. */
function spelledText(node: ts.Node): string | undefined {
	if (ts.isIdentifier(node) || ts.isPrivateIdentifier(node)) return node.text;
	if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text;
	if (ts.isTemplateHead(node) || ts.isTemplateMiddle(node) || ts.isTemplateTail(node)) {
		return node.text;
	}
	return undefined;
}

function parsed(name: string, text: string): ts.SourceFile {
	return ts.createSourceFile(name, text, ts.ScriptTarget.Latest, true);
}

function lineOf(source: ts.SourceFile, node: ts.Node): string {
	const { line } = source.getLineAndCharacterOfPosition(node.getStart(source));
	return `line ${String(line + 1)}`;
}

/** Why an object literal could hide a key from a scan by name: a spread or a key computed from an expression. */
function hiddenKeyRefusals(source: ts.SourceFile): readonly string[] {
	return descendants(source)
		.filter(ts.isObjectLiteralExpression)
		.flatMap((object) => object.properties)
		.flatMap((property) => {
			if (ts.isSpreadAssignment(property)) return [`a spread at ${lineOf(source, property)}`];
			return ts.isComputedPropertyName(property.name)
				? [`a computed key at ${lineOf(source, property)}`]
				: [];
		});
}

/** The brand title the manager shows. */
const brandTitle = 'Zurfur Component Gallery';

/** Theme keys the manager never sets: a brand picture and a brand link. */
const refusedBrandKeys: ReadonlySet<string> = new Set(['brandImage', 'brandUrl']);

/** The string a `const` declared in `source` under `name` holds, when it is one plain string. */
function constString(source: ts.SourceFile, name: string): string | undefined {
	const declarations = descendants(source)
		.filter(ts.isVariableDeclaration)
		.filter((declaration) => ts.isIdentifier(declaration.name) && declaration.name.text === name);
	const [declaration, ...others] = declarations;
	if (declaration === undefined || others.length > 0) return undefined;
	const list = declaration.parent;
	const isConst = ts.isVariableDeclarationList(list) && (list.flags & ts.NodeFlags.Const) !== 0;
	const value = declaration.initializer;
	if (!isConst || value === undefined) return undefined;
	return ts.isStringLiteral(value) || ts.isNoSubstitutionTemplateLiteral(value)
		? value.text
		: undefined;
}

/** The constant text a `brandTitle` property is given, or undefined when it is not constant. */
function titleValue(source: ts.SourceFile, property: ts.PropertyAssignment): string | undefined {
	const value = property.initializer;
	if (ts.isStringLiteral(value) || ts.isNoSubstitutionTemplateLiteral(value)) return value.text;
	return ts.isIdentifier(value) ? constString(source, value.text) : undefined;
}

/**
 * Why the manager's theme could carry more than the constant title. Storybook
 * renders `brandTitle` as raw HTML, so it is set once, as a `key: value`
 * property, to the constant title (written in place or held in a `const`); any other
 * mention of it, and any mention of `brandImage` or `brandUrl`, is refused.
 */
function managerRefusals(text: string): readonly string[] {
	const source = parsed('manager.ts', text);
	const spelled = descendants(source).flatMap((node) => {
		const spelling = spelledText(node);
		return spelling === undefined ? [] : [{ node, spelling }];
	});
	const brandRefusals = spelled
		.filter(({ spelling }) => refusedBrandKeys.has(spelling))
		.map(({ node, spelling }) => `${spelling} at ${lineOf(source, node)}`);
	const titleMentions = spelled.filter(({ spelling }) => spelling === 'brandTitle');
	const titleRefusals = titleMentions.flatMap(({ node }) => {
		const property = node.parent;
		const isPropertyName = ts.isPropertyAssignment(property) && property.name === node;
		if (!isPropertyName)
			return [`brandTitle used other than as a property at ${lineOf(source, node)}`];
		return titleValue(source, property) === brandTitle
			? []
			: [`brandTitle is not the constant title at ${lineOf(source, node)}`];
	});
	const countRefusals =
		titleMentions.length === 1 ? [] : [`brandTitle is named ${String(titleMentions.length)} times`];
	return [...brandRefusals, ...titleRefusals, ...countRefusals, ...hiddenKeyRefusals(source)];
}

const managerText = readFileSync(new URL('manager.ts', configFolder), 'utf8');

describe("the manager's brand", () => {
	it('is the constant title, with no brand image or link', () => {
		expect(managerRefusals(managerText)).toEqual([]);
	});

	/** A manager file setting the theme to `theme`, with `prelude` before it. */
	const manager = (theme: string, prelude = ''): string =>
		[
			"import { addons } from 'storybook/manager-api';",
			"import { create } from 'storybook/theming/create';",
			prelude,
			`addons.setConfig({ theme: create(${theme}) });`
		].join('\n');

	it.each([
		[
			'the title written in place',
			manager("{ base: 'dark', brandTitle: 'Zurfur Component Gallery' }")
		],
		[
			'the title held in a const',
			manager('{ brandTitle: TITLE }', "const TITLE = 'Zurfur Component Gallery';")
		]
	])('passes %s', (_label, text) => {
		expect(managerRefusals(text)).toEqual([]);
	});

	it.each([
		[
			'a brand link',
			manager("{ brandTitle: 'Zurfur Component Gallery', brandUrl: 'https://x.example' }"),
			'brandUrl'
		],
		[
			'a brand image',
			manager("{ brandTitle: 'Zurfur Component Gallery', brandImage: './logo.svg' }"),
			'brandImage'
		],
		[
			'a title read from the address',
			manager('{ brandTitle: location.hash }'),
			'not the constant title'
		],
		[
			'a title from a template',
			manager('{ brandTitle: `${name}` }', "const name = 'x';"),
			'not the constant title'
		],
		[
			'a title held in a let',
			manager('{ brandTitle: TITLE }', "let TITLE = 'Zurfur Component Gallery';"),
			'not the constant title'
		],
		[
			'another title',
			manager("{ brandTitle: '<img src=x onerror=alert(1)>' }"),
			'not the constant title'
		],
		['no title', manager("{ base: 'dark' }"), 'named 0 times'],
		[
			'a shorthand title',
			manager('{ brandTitle }', "const brandTitle = 'Zurfur Component Gallery';"),
			'other than as a property'
		],
		[
			'a title set later',
			manager(
				"{ brandTitle: 'Zurfur Component Gallery' }",
				'const theme = {};\ntheme.brandTitle = location.hash;'
			),
			'other than as a property'
		],
		[
			'a spread into the theme',
			manager("{ ...extra, brandTitle: 'Zurfur Component Gallery' }"),
			'a spread'
		],
		[
			'a computed key',
			manager("{ [key]: 'x', brandTitle: 'Zurfur Component Gallery' }"),
			'a computed key'
		]
	])('refuses %s', (_label, text, reason) => {
		expect(managerRefusals(text)).toContainEqual(expect.stringContaining(reason));
	});
});

/** A value read from literal syntax only, or `undefined` for anything else. */
function literalValue(node: ts.Expression): unknown {
	if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text;
	if (ts.isNumericLiteral(node)) return Number(node.text);
	if (node.kind === ts.SyntaxKind.TrueKeyword) return true;
	if (node.kind === ts.SyntaxKind.FalseKeyword) return false;
	if (ts.isArrayLiteralExpression(node)) {
		const items = node.elements.map((element) =>
			ts.isSpreadElement(element) ? undefined : literalValue(element)
		);
		return items.includes(undefined) ? undefined : items;
	}
	if (ts.isObjectLiteralExpression(node)) {
		const entries = node.properties.map((property) => {
			if (!ts.isPropertyAssignment(property)) return undefined;
			const key = spelledText(property.name);
			const value = literalValue(property.initializer);
			return key === undefined || value === undefined ? undefined : ([key, value] as const);
		});
		const known = entries.filter((entry) => entry !== undefined);
		return known.length === entries.length ? Object.fromEntries(known) : undefined;
	}
	return undefined;
}

/** The object literal `export default <name>` names, through its `const` declaration. */
function defaultExportObject(source: ts.SourceFile): ts.ObjectLiteralExpression | undefined {
	const exported = source.statements.find(ts.isExportAssignment)?.expression;
	if (exported === undefined) return undefined;
	if (ts.isObjectLiteralExpression(exported)) return exported;
	if (!ts.isIdentifier(exported)) return undefined;
	const declaration = descendants(source)
		.filter(ts.isVariableDeclaration)
		.find((candidate) => ts.isIdentifier(candidate.name) && candidate.name.text === exported.text);
	const value = declaration?.initializer;
	return value !== undefined && ts.isObjectLiteralExpression(value) ? value : undefined;
}

/** The expression a plain `key: value` property of `object` holds; undefined when absent or not plain. */
function propertyValue(object: ts.ObjectLiteralExpression, key: string): ts.Expression | undefined {
	const matches = object.properties.filter(
		(property) => property.name !== undefined && spelledText(property.name) === key
	);
	const [property, ...others] = matches;
	if (property === undefined || others.length > 0 || !ts.isPropertyAssignment(property))
		return undefined;
	return property.initializer;
}

/** The value at a path of plain properties under `object`, read from literal syntax. */
function literalAt(object: ts.ObjectLiteralExpression, keys: readonly string[]): unknown {
	const [key, ...rest] = keys;
	if (key === undefined) return literalValue(object);
	const value = propertyValue(object, key);
	if (value === undefined) return undefined;
	if (rest.length === 0) return literalValue(value);
	return ts.isObjectLiteralExpression(value) ? literalAt(value, rest) : undefined;
}

/** The accessibility rules every story is held to: errors fail, under all five WCAG tag sets. */
const allowedA11y: Fields = {
	test: 'error',
	options: {
		runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] }
	}
};

/** True when `order` names `groups` in order, compared case-insensitively. */
function isGroupOrder(order: unknown, groups: readonly string[]): boolean {
	if (!Array.isArray(order) || order.length !== groups.length) return false;
	return order.every(
		(name, index) => typeof name === 'string' && name.toLowerCase() === groups[index]?.toLowerCase()
	);
}

/**
 * Why the preview's accessibility rules or sidebar order differ from the
 * gallery's. Both are read from literal syntax in the default export, with no
 * spread or computed key anywhere to hide another value.
 */
function previewRefusals(text: string, groups: readonly string[]): readonly string[] {
	const source = parsed('preview.ts', text);
	const preview = defaultExportObject(source);
	if (preview === undefined) return ['the default export is not an object literal'];
	const a11y = literalAt(preview, ['parameters', 'a11y']);
	const order = literalAt(preview, ['parameters', 'options', 'storySort', 'order']);
	return [
		...(isDeepStrictEqual(a11y, allowedA11y) ? [] : ['parameters.a11y is not the allowed rules']),
		...(isGroupOrder(order, groups) ? [] : ["the story sort is not the gallery's group order"]),
		...hiddenKeyRefusals(source)
	];
}

const previewText = readFileSync(new URL('preview.ts', configFolder), 'utf8');

describe("the preview's rules", () => {
	it("holds the accessibility rules and the gallery's group order", () => {
		expect(previewRefusals(previewText, groupNames)).toEqual([]);
	});

	const groups = ['Tokens and paper', 'Controls'];
	const a11y =
		"a11y: { test: 'error', options: { runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] } } }";
	const sort = "options: { storySort: { order: ['Tokens and Paper', 'Controls'] } }";
	const preview = (parameters: string, prelude = ''): string =>
		`${prelude}\nconst preview = { decorators: [], parameters: { ${parameters} } };\nexport default preview;\n`;

	it('passes a preview like the real one', () => {
		expect(previewRefusals(preview(`${a11y}, ${sort}`), groups)).toEqual([]);
	});

	it.each([
		['a11y warning only', preview(`${a11y.replace("'error'", "'todo'")}, ${sort}`), 'a11y'],
		['a tag set dropped', preview(`${a11y.replace(", 'wcag22aa'", '')}, ${sort}`), 'a11y'],
		[
			'a rule switched off',
			preview(
				`${a11y.replace(' } } }', " }, rules: { 'color-contrast': { enabled: false } } } }")}, ${sort}`
			),
			'a11y'
		],
		['no a11y', preview(sort), 'a11y'],
		['a11y from a variable', preview(`a11y: rules, ${sort}`, 'const rules = {};'), 'a11y'],
		[
			'groups out of order',
			preview(
				`${a11y}, ${sort.replace("'Tokens and Paper', 'Controls'", "'Controls', 'Tokens and Paper'")}`
			),
			'story sort'
		],
		['a group missing', preview(`${a11y}, ${sort.replace(", 'Controls'", '')}`), 'story sort'],
		['a spread of parameters', preview(`${a11y}, ${sort}, ...more`), 'a spread'],
		['a second a11y', preview(`${a11y}, ${sort}, ${a11y}`), 'a11y']
	])('refuses %s', (_label, text, reason) => {
		expect(previewRefusals(text, groups)).toContainEqual(expect.stringContaining(reason));
	});
});
