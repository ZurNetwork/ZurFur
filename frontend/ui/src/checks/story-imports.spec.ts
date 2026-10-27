import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { parse } from 'svelte/compiler';
import path from 'node:path';
import { docTextRules, type DocTextRule } from './scan/network';
import { packageSources, type SourceFile } from './scan/parse';
import * as room from '../stories/controls/room.ts';

/** Why a stories file is refused, and what it named. */
export interface StoryRefusal {
	readonly path: string;
	readonly reason: StoryRefusalReason;
	readonly detail: string;
}

export type StoryRefusalReason =
	| 'does not parse'
	| 'a package stories never import'
	| 'a relative import outside src/stories and src/gallery'
	| 'a relative import with a query or hash'
	| 'a computed import'
	| 'a <style> element'
	| 'a style attribute'
	| 'a style: directive'
	| 'an a11y setting'
	| 'a backgrounds setting'
	| 'a docs height not imported from room'
	| 'a URL in a string'
	| 'an HTML tag in a string'
	| 'a remote link or image in a string';

/** The packages a stories file may import, each exactly as written: `@zurfur/ui` by its public entry only. */
const allowedPackages: ReadonlySet<string> = new Set([
	'@storybook/addon-svelte-csf',
	'storybook/test',
	'@zurfur/ui'
]);

/** The folders a stories file may import relatively: the stories and the token pages' parts. */
const allowedFolders: readonly string[] = ['src/stories/', 'src/gallery/'];

/** The module every docs height comes from, as a package path without its extension. */
const roomModule = 'src/stories/controls/room';

/** The one shape a room may take: a whole number of rem from 1 to 999, so no CSS rides along. */
const roomPattern = /^[1-9]\d{0,2}rem$/;

/** True for a value the room module may export. */
export function isRoom(value: unknown): boolean {
	return typeof value === 'string' && roomPattern.test(value);
}

/** True for a Svelte CSF stories file under src/stories/. */
export function isStoriesFile(filePath: string): boolean {
	return filePath.startsWith('src/stories/') && filePath.endsWith('.stories.svelte');
}

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

/** Every object in a parsed tree, each once. */
function nodesOf(root: unknown): readonly Fields[] {
	const seen = new Set<object>();
	const found: Fields[] = [];
	const visit = (value: unknown): void => {
		if (Array.isArray(value)) {
			for (const item of value) visit(item);
			return;
		}
		if (!isFields(value) || seen.has(value)) return;
		seen.add(value);
		found.push(value);
		for (const child of Object.values(value)) visit(child);
	};
	visit(root);
	return found;
}

/** The text of a string literal, or of a template literal with no expressions. */
function staticTextOf(node: unknown): string | undefined {
	if (!isFields(node)) return undefined;
	if (node.type === 'Literal') return typeof node.value === 'string' ? node.value : undefined;
	if (node.type !== 'TemplateLiteral') return undefined;
	const { expressions, quasis } = node;
	if (!Array.isArray(expressions) || expressions.length > 0 || !Array.isArray(quasis)) {
		return undefined;
	}
	const quasi: unknown = quasis[0];
	const value = isFields(quasi) ? quasi.value : undefined;
	const cooked = isFields(value) ? value.cooked : undefined;
	return typeof cooked === 'string' ? cooked : undefined;
}

/** The name a property key or member property spells: an identifier, or a static string. */
function spelledName(node: unknown, computed: unknown): string | undefined {
	if (computed !== true && isFields(node) && node.type === 'Identifier') {
		return typeof node.name === 'string' ? node.name : undefined;
	}
	return staticTextOf(node);
}

/** True for a key or member named `a11y`: `{ a11y: … }`, `{ 'a11y': … }`, `x.a11y`, `x['a11y']`. */
function namesA11y(node: Fields): boolean {
	if (node.type === 'Property') return spelledName(node.key, node.computed) === 'a11y';
	if (node.type === 'MemberExpression') return spelledName(node.property, node.computed) === 'a11y';
	return false;
}

/** True for a key or member named `backgrounds`, which Storybook writes into the page as raw CSS. */
function namesBackgrounds(node: Fields): boolean {
	if (node.type === 'Property') return spelledName(node.key, node.computed) === 'backgrounds';
	if (node.type === 'MemberExpression') {
		return spelledName(node.property, node.computed) === 'backgrounds';
	}
	return false;
}

/** The keys Storybook pastes into a docs page's CSS as a story's height. */
const heightKeys: ReadonlySet<string> = new Set(['height', 'iframeHeight']);

/** The local names a file imports by name from the room module. */
function roomImports(filePath: string, nodes: readonly Fields[]): ReadonlySet<string> {
	const names = nodes.flatMap((node): readonly string[] => {
		if (node.type !== 'ImportDeclaration' || !isFields(node.source)) return [];
		const source = node.source.value;
		if (typeof source !== 'string' || !source.startsWith('.')) return [];
		const target = path.posix.join(path.posix.dirname(filePath), source).replace(/\.ts$/, '');
		if (target !== roomModule || !Array.isArray(node.specifiers)) return [];
		return node.specifiers.flatMap((specifier: unknown): readonly string[] => {
			if (!isFields(specifier) || specifier.type !== 'ImportSpecifier') return [];
			const local = isFields(specifier.local) ? specifier.local.name : undefined;
			return typeof local === 'string' ? [local] : [];
		});
	});
	return new Set(names);
}

/**
 * True for a height Storybook would paste into CSS that isn't a name imported
 * from the room module: a `height` or `iframeHeight` key holding anything
 * else, or an assignment to a member of either name.
 */
function isRawHeight(node: Fields, rooms: ReadonlySet<string>): boolean {
	if (node.type === 'Property') {
		const key = spelledName(node.key, node.computed);
		if (key === undefined || !heightKeys.has(key)) return false;
		const value = isFields(node.value) ? node.value : undefined;
		const name = value?.type === 'Identifier' ? value.name : undefined;
		return typeof name !== 'string' || !rooms.has(name);
	}
	if (node.type !== 'AssignmentExpression' || !isFields(node.left)) return false;
	const { left } = node;
	if (left.type !== 'MemberExpression') return false;
	const member = spelledName(left.property, left.computed);
	return member !== undefined && heightKeys.has(member);
}

/** A string literal's value, or a template literal's cooked and raw text with its expressions left out. */
function stringReadings(node: Fields): readonly string[] {
	if (node.type === 'Literal') return typeof node.value === 'string' ? [node.value] : [];
	if (node.type !== 'TemplateLiteral' || !Array.isArray(node.quasis)) return [];
	const values = node.quasis.flatMap((quasi: unknown): readonly Fields[] => {
		const value = isFields(quasi) ? quasi.value : undefined;
		return isFields(value) ? [value] : [];
	});
	const joined = (part: 'cooked' | 'raw'): string =>
		values.map((value) => (typeof value[part] === 'string' ? value[part] : '')).join('');
	return [joined('cooked'), joined('raw')];
}

/** Why a string breaks the docs-text rule, in the words a refusal uses. */
function docTextReason(rule: DocTextRule): StoryRefusalReason {
	switch (rule) {
		case 'doc-url':
			return 'a URL in a string';
		case 'doc-tag':
			return 'an HTML tag in a string';
		case 'doc-destination':
			return 'a remote link or image in a string';
	}
}

/** The text of each script block of a parsed `.svelte` file. */
function scriptTexts(text: string, root: Fields): readonly string[] {
	return [root.module, root.instance].flatMap((script) => {
		const content = isFields(script) ? script.content : undefined;
		if (!isFields(content)) return [];
		const { start, end } = content;
		return typeof start === 'number' && typeof end === 'number' ? [text.slice(start, end)] : [];
	});
}

/** Why one import specifier is refused, or undefined when it is allowed. */
function specifierRefusal(filePath: string, specifier: string): StoryRefusalReason | undefined {
	if (!specifier.startsWith('.')) {
		return allowedPackages.has(specifier) ? undefined : 'a package stories never import';
	}
	if (/[?#]/.test(specifier)) return 'a relative import with a query or hash';
	const target = path.posix.join(path.posix.dirname(filePath), specifier);
	const inside = allowedFolders.some((folder) => target.startsWith(folder));
	return inside ? undefined : 'a relative import outside src/stories and src/gallery';
}

function parseRoot(text: string): Fields | string {
	try {
		const root: unknown = parse(text, { modern: true });
		return isFields(root) ? root : 'the parser gave no tree';
	} catch (error) {
		return error instanceof Error ? error.message : 'the parser gave up';
	}
}

/**
 * Every refusal in one stories file: an import other than Svelte CSF,
 * `storybook/test`, `@zurfur/ui` and relative files under src/stories/ and
 * src/gallery/; any `<style>`, `style=` or `style:`; any `a11y` or
 * `backgrounds` key; a `height` or `iframeHeight` that isn't a name imported
 * from the room module; and any
 * string or template text that breaks the lib doc-comment rule, since docs
 * descriptions render as Markdown with raw HTML allowed.
 */
export function storyFileRefusals(file: SourceFile): readonly StoryRefusal[] {
	const refusal = (reason: StoryRefusalReason, detail: string): StoryRefusal => ({
		path: file.path,
		reason,
		detail
	});
	const root = parseRoot(file.text);
	if (typeof root === 'string') return [refusal('does not parse', root)];
	const nodes = nodesOf(root);

	const scriptSpecifiers = scriptTexts(file.text, root).flatMap((script) =>
		ts.preProcessFile(script, true, true).importedFiles.map((imported) => imported.fileName)
	);
	const dynamicSources = nodes.flatMap((node) =>
		node.type === 'ImportExpression' ? [staticTextOf(node.source)] : []
	);
	const computedImports = dynamicSources.filter((source) => source === undefined);
	const specifiers = new Set([
		...scriptSpecifiers,
		...dynamicSources.filter((source) => source !== undefined)
	]);
	const importRefusals = [...specifiers].flatMap((specifier) => {
		const reason = specifierRefusal(file.path, specifier);
		return reason === undefined ? [] : [refusal(reason, specifier)];
	});

	const styleRefusals = nodes.flatMap((node): readonly StoryRefusal[] => {
		const name = typeof node.name === 'string' ? node.name : '';
		if (node.type === 'StyleSheet') return [refusal('a <style> element', '<style>')];
		if (node.type === 'RegularElement' && name.toLowerCase() === 'style') {
			return [refusal('a <style> element', `<${name}>`)];
		}
		if (node.type === 'Attribute' && name.toLowerCase() === 'style') {
			return [refusal('a style attribute', name)];
		}
		if (node.type === 'StyleDirective') return [refusal('a style: directive', `style:${name}`)];
		return [];
	});

	const a11yRefusals = nodes.filter(namesA11y).map(() => refusal('an a11y setting', 'a11y'));
	const backgroundsRefusals = nodes
		.filter(namesBackgrounds)
		.map(() => refusal('a backgrounds setting', 'backgrounds'));
	const rooms = roomImports(file.path, nodes);
	const heightRefusals = nodes
		.filter((node) => isRawHeight(node, rooms))
		.map(() => refusal('a docs height not imported from room', 'height'));

	const stringRefusals = nodes.flatMap((node) => {
		const readings = stringReadings(node);
		const rules = new Set(readings.flatMap(docTextRules));
		const excerpt = (readings[0] ?? '').replace(/\s+/g, ' ').slice(0, 80);
		return [...rules].map((rule) => refusal(docTextReason(rule), excerpt));
	});

	return [
		...importRefusals,
		...computedImports.map(() => refusal('a computed import', 'import(…)')),
		...styleRefusals,
		...a11yRefusals,
		...backgroundsRefusals,
		...heightRefusals,
		...stringRefusals
	];
}

const storyPath = 'src/stories/controls/Button.stories.svelte';

/** A stories file with `imports` in its module script, `meta` in defineMeta and `body` after it. */
function storiesFile(imports: string, meta: string, body: string): string {
	return [
		'<script module lang="ts">',
		"\timport { defineMeta } from '@storybook/addon-svelte-csf';",
		imports,
		'',
		'\tconst { Story } = defineMeta({',
		"\t\ttitle: 'Controls/Button',",
		`\t\tcomponent: Button${meta}`,
		'\t});',
		'</script>',
		'',
		body,
		''
	].join('\n');
}

const goodImports = [
	"\timport { expect, fn } from 'storybook/test';",
	"\timport { Button } from '@zurfur/ui';",
	"\timport { people } from '../fixtures/people.ts';",
	"\timport GalleryState from '../../gallery/GalleryState.svelte';"
].join('\n');

const goodMeta =
	",\n\t\ttags: ['autodocs'],\n\t\targs: { onPress: fn() },\n\t\tparameters: { layout: 'centered' }";

const goodBody =
	'<Story name="Primary" args={{ variant: \'primary\' }} play={async ({ canvas }) => {\n\tawait expect(canvas.getByRole(\'button\')).toBeVisible();\n}} />\n\n<Story name="Busy">\n\t<GalleryState>{people.length}</GalleryState>\n</Story>';

function reasonsFor(text: string): readonly StoryRefusalReason[] {
	return storyFileRefusals({ path: storyPath, text }).map((refusal) => refusal.reason);
}

describe('story imports', () => {
	it('passes a good stories file', () => {
		expect(reasonsFor(storiesFile(goodImports, goodMeta, goodBody))).toEqual([]);
	});

	it.each([
		[
			'a relative path into src/lib',
			"\timport Button from '../../lib/controls/Button.svelte';",
			'a relative import outside src/stories and src/gallery'
		],
		[
			'a relative path past src',
			"\timport { Button } from '../../../package.json';",
			'a relative import outside src/stories and src/gallery'
		],
		[
			'a relative path into the checks',
			"\timport { Button } from '../../checks/x.ts';",
			'a relative import outside src/stories and src/gallery'
		],
		[
			'a relative path with a query',
			"\timport { Button } from '../fixtures/x.ts?raw';",
			'a relative import with a query or hash'
		],
		['bits-ui', "\timport { Button } from 'bits-ui';", 'a package stories never import'],
		['react', "\timport Button from 'react';", 'a package stories never import'],
		['@zurfur/ui/styles', "\timport '@zurfur/ui/styles';", 'a package stories never import'],
		[
			'a deep path into @zurfur/ui',
			"\timport Button from '@zurfur/ui/src/lib/controls/Button.svelte';",
			'a package stories never import'
		],
		[
			'another Storybook package',
			"\timport { Button } from '@storybook/svelte';",
			'a package stories never import'
		],
		['storybook itself', "\timport { Button } from 'storybook';", 'a package stories never import'],
		[
			'a re-export',
			"\texport { Button } from 'bits-ui';\n\timport { Button } from '@zurfur/ui';",
			'a package stories never import'
		],
		[
			'a type-only import',
			"\timport type { Meta } from '@storybook/svelte';\n\timport { Button } from '@zurfur/ui';",
			'a package stories never import'
		],
		[
			'a dynamic import in the script',
			"\timport { Button } from '@zurfur/ui';\n\tconst heavy = import('three');",
			'a package stories never import'
		],
		[
			'a computed dynamic import',
			"\timport { Button } from '@zurfur/ui';\n\tconst name = 'three';\n\tconst heavy = import(name);",
			'a computed import'
		]
	] as const)('refuses %s', (_label, imports, reason) => {
		expect(reasonsFor(storiesFile(imports, '', '<Story name="Plain" />'))).toEqual([reason]);
	});

	it('refuses an import in a markup expression', () => {
		const body = "{#await import('react') then m}{m}{/await}";
		const text = storiesFile("\timport { Button } from '@zurfur/ui';", '', body);
		expect(reasonsFor(text)).toEqual(['a package stories never import']);
	});

	it('refuses an import in the instance script', () => {
		const text = `${storiesFile("\timport { Button } from '@zurfur/ui';", '', '<Story name="Plain" />')}\n<script lang="ts">\n\timport 'react';\n</script>\n`;
		expect(reasonsFor(text)).toEqual(['a package stories never import']);
	});

	const imports = "\timport { Button } from '@zurfur/ui';";

	it.each([
		[
			'<style>',
			'<Story name="Plain" />\n\n<style>\n\tp { color: red; }\n</style>',
			'a <style> element'
		],
		['an empty <style>', '<Story name="Plain" />\n\n<style></style>', 'a <style> element'],
		[
			'a <style> inside the markup',
			'<Story name="Plain">\n\t<div><style>p { color: red; }</style></div>\n</Story>',
			'a <style> element'
		],
		[
			'a <style> in the head',
			'<svelte:head><style>p { color: red; }</style></svelte:head>\n\n<Story name="Plain" />',
			'a <style> element'
		],
		[
			'style="…"',
			'<Story name="Plain">\n\t<p style="color: red">x</p>\n</Story>',
			'a style attribute'
		],
		[
			'style={…}',
			'<Story name="Plain">\n\t<p style={\'color: red\'}>x</p>\n</Story>',
			'a style attribute'
		],
		[
			'STYLE="…"',
			'<Story name="Plain">\n\t<p STYLE="color: red">x</p>\n</Story>',
			'a style attribute'
		],
		[
			'a style prop on a component',
			'<Story name="Plain">\n\t<Button style="color: red">x</Button>\n</Story>',
			'a style attribute'
		],
		[
			'style:color',
			'<Story name="Plain">\n\t<p style:color="red">x</p>\n</Story>',
			'a style: directive'
		],
		[
			"parameters: { a11y: { test: 'off' } } on a story",
			'<Story name="Plain" parameters={{ a11y: { test: \'off\' } }} />',
			'an a11y setting'
		],
		[
			"a quoted 'a11y' key",
			"<Story name=\"Plain\" parameters={{ 'a11y': { test: 'todo' } }} />",
			'an a11y setting'
		]
	] as const)('refuses %s', (_label, body, reason) => {
		expect(reasonsFor(storiesFile(imports, '', body))).toEqual([reason]);
	});

	it.each([
		[
			"parameters: { a11y: { test: 'off' } } in defineMeta",
			",\n\t\tparameters: { a11y: { test: 'off' } }"
		],
		['a computed a11y key', ",\n\t\tparameters: { ['a11y']: { test: 'off' } }"],
		['a member named a11y', ',\n\t\ttags: [String(Object.keys({}).a11y)]']
	] as const)('refuses %s', (_label, meta) => {
		expect(reasonsFor(storiesFile(imports, meta, '<Story name="Plain" />'))).toEqual([
			'an a11y setting'
		]);
	});

	/** A defineMeta tail whose docs description is `source`, written as script source. */
	const describedAs = (source: string): string =>
		`,\n\t\tparameters: { docs: { description: { component: ${source} } } }`;

	it.each([
		['a Markdown image', "'![x](https://example.org/a.png)'", 'a URL in a string'],
		['an <img>', '\'<img src="https://example.org/b.png">\'', 'a URL in a string'],
		[
			'a remote stylesheet',
			'\'<link rel="stylesheet" href="https://example.org/c.css">\'',
			'a URL in a string'
		],
		['an <iframe>', '\'<iframe src="https://example.org/">\'', 'a URL in a string'],
		['an <object>', '\'<object data="https://example.org/d">\'', 'a URL in a string'],
		['a <video>', '\'<video src="https://example.org/e.mp4">\'', 'a URL in a string'],
		[
			'an <svg><image>',
			'\'<svg><image href="https://example.org/f.png"/></svg>\'',
			'a URL in a string'
		]
	] as const)('refuses %s in a docs description', (_label, source, reason) => {
		const reasons = reasonsFor(storiesFile(imports, describedAs(source), '<Story name="Plain" />'));
		expect(reasons).toContain(reason);
	});

	it.each([
		['an <img> with a local source', '\'<img src="./a.png">\'', ['an HTML tag in a string']],
		[
			'a backslash host in a Markdown link',
			"'[x](/\\\\example.org/g)'",
			['a remote link or image in a string']
		],
		[
			'a backslash host in a Markdown image',
			"'![x](\\\\\\\\example.org/g.png)'",
			['a remote link or image in a string']
		],
		[
			'a backslash host in a reference definition',
			"'![x][r]\\n\\n[r]: /\\\\example.org/g.png'",
			['a remote link or image in a string']
		],
		['a tag in a template literal', '`A <img src="./a.png"> here`', ['an HTML tag in a string']],
		[
			'a tag split across a template expression',
			'`<${\'\'}img src="./a.png">`',
			['an HTML tag in a string']
		],
		['a tag behind a unicode escape', '\'\\u003cimg src="./a.png">\'', ['an HTML tag in a string']],
		[
			'a URL in a String.raw template',
			'String.raw`[x](/\\example.org/g)`',
			['a remote link or image in a string']
		]
	] as const)('refuses %s in a docs description', (_label, source, reasons) => {
		expect(reasonsFor(storiesFile(imports, describedAs(source), '<Story name="Plain" />'))).toEqual(
			reasons
		);
	});

	it('refuses every remote load in one docs description', () => {
		const payload = [
			'![x](https://example.org/a.png)',
			'<img src="https://example.org/b.png">',
			'<link rel="stylesheet" href="https://example.org/c.css">',
			'<iframe src="https://example.org/">',
			'<object data="https://example.org/d">',
			'<video src="https://example.org/e.mp4">',
			'<svg><image href="https://example.org/f.png"/></svg>',
			'[x](/\\\\example.org/g)'
		].join(' ');
		const reasons = reasonsFor(
			storiesFile(imports, describedAs(`'${payload}'`), '<Story name="Plain" />')
		);
		expect(reasons).toEqual([
			'a URL in a string',
			'an HTML tag in a string',
			'a remote link or image in a string'
		]);
	});

	it.each([
		[
			"a story's parameters in the markup",
			storiesFile(
				imports,
				'',
				'<Story name="Plain" parameters={{ docs: { description: { story: \'<img src="./a.png">\' } } }} />'
			)
		],
		[
			'the instance script',
			`${storiesFile(imports, '', '<Story name="Plain" />')}\n<script lang="ts">\n\tconst words = '<img src="./a.png">';\n</script>\n`
		],
		[
			'a markup expression',
			storiesFile(imports, '', '<Story name="Plain">{\'<img src="./a.png">\'}</Story>')
		]
	] as const)('refuses a tag in a string in %s', (_label, text) => {
		expect(reasonsFor(text)).toEqual(['an HTML tag in a string']);
	});

	it.each([
		['a keyboard map', "'Tab moves to the button; Enter and Space press it.'"],
		['a tag in a code span', "'Renders a `<button>`; `Esc` closes it.'"],
		['a local Markdown link', "'See [the paper](./Paper.mdx) and a < b.'"],
		['a template with an expression', '`${1} of ${2} pages`']
	] as const)('passes %s in a docs description', (_label, source) => {
		expect(reasonsFor(storiesFile(imports, describedAs(source), '<Story name="Plain" />'))).toEqual(
			[]
		);
	});

	it('refuses a file that does not parse', () => {
		expect(reasonsFor('<script module lang="ts">\n\timport {')).toEqual(['does not parse']);
	});

	it.each([
		['src/stories/controls/Button.stories.svelte', true],
		['src/stories/tokens/deep/Colour.stories.svelte', true],
		['src/stories/frame/StoryFrame.svelte', false],
		['src/stories/fixtures/people.ts', false],
		['src/lib/Button.stories.svelte', false]
	] as const)('reads %s as a stories file: %s', (filePath, expected) => {
		expect(isStoriesFile(filePath)).toBe(expected);
	});

	it.each([
		[
			'a raw docs story height in defineMeta',
			",\n\t\tparameters: { docs: { story: { height: '1px} *{background:red} x{' } } }"
		],
		[
			'a raw iframeHeight in defineMeta',
			",\n\t\tparameters: { docs: { story: { iframeHeight: '300px' } } }"
		],
		['a quoted height key', ",\n\t\tparameters: { docs: { story: { 'height': '16rem' } } }"],
		['a height from a local name', ',\n\t\tparameters: { docs: { story: { height: ROOM } } }'],
		['a height from a template', ',\n\t\tparameters: { docs: { story: { height: `${16}rem` } } }']
	] as const)('refuses %s', (_label, meta) => {
		const localRoom = "\timport { Button } from '@zurfur/ui';\n\tconst ROOM = '16rem';";
		expect(reasonsFor(storiesFile(localRoom, meta, '<Story name="Plain" />'))).toEqual([
			'a docs height not imported from room'
		]);
	});

	it.each([
		[
			"a raw height in a story's parameters",
			'<Story name="Plain" parameters={{ docs: { story: { height: \'200px\' } } }} />'
		],
		[
			"a raw iframeHeight in a story's parameters",
			'<Story name="Plain" parameters={{ docs: { story: { iframeHeight: \'200px\' } } }} />'
		],
		[
			'a height assigned in a play function',
			'<Story name="Plain" play={async ({ parameters }) => { parameters.docs.story.height = \'2px\'; }} />'
		],
		[
			'a member of a room namespace import',
			'<Story name="Plain" parameters={{ docs: { story: { height: rooms.FLOATING_ROOM } } }} />'
		]
	] as const)('refuses %s', (_label, body) => {
		const withNamespace =
			"\timport { Button } from '@zurfur/ui';\n\timport * as rooms from './room.ts';";
		expect(reasonsFor(storiesFile(withNamespace, '', body))).toEqual([
			'a docs height not imported from room'
		]);
	});

	it.each([
		[
			'the meta',
			',\n\t\tparameters: { docs: { story: { height: FLOATING_ROOM } } }',
			'<Story name="Plain" />'
		],
		[
			"a story's parameters",
			'',
			'<Story name="Plain" parameters={{ docs: { story: { iframeHeight: FLOATING_ROOM } } }} />'
		]
	] as const)('passes a height imported from room in %s', (_label, meta, body) => {
		const fromRoom =
			"\timport { Button } from '@zurfur/ui';\n\timport { FLOATING_ROOM } from './room.ts';";
		expect(reasonsFor(storiesFile(fromRoom, meta, body))).toEqual([]);
	});

	it.each([
		[
			'in defineMeta',
			",\n\t\tparameters: { backgrounds: { default: 'x' } }",
			'<Story name="Plain" />'
		],
		[
			'a quoted key',
			",\n\t\tparameters: { 'backgrounds': { disable: true } }",
			'<Story name="Plain" />'
		],
		[
			"on a story's parameters",
			'',
			"<Story name=\"Plain\" parameters={{ backgrounds: { values: [{ name: 'x', value: 'red' }] } }} />"
		],
		[
			'a member named backgrounds',
			',\n\t\ttags: [String(Object.keys({}).backgrounds)]',
			'<Story name="Plain" />'
		]
	] as const)('refuses a backgrounds setting %s', (_label, meta, body) => {
		expect(reasonsFor(storiesFile(imports, meta, body))).toEqual(['a backgrounds setting']);
	});

	it('holds every room to a whole number of rem', () => {
		const rooms = Object.entries(room);
		expect(rooms.map(([name]) => name)).toContain('FLOATING_ROOM');
		expect(rooms.filter(([, value]) => !isRoom(value))).toEqual([]);
	});

	it.each([
		['1px} *{background:url(\\2f\\2f evil.example/x)} x{'],
		['16rem;'],
		['16px'],
		['0rem'],
		['1000rem'],
		['16.5rem'],
		[' 16rem'],
		['16REM'],
		[16]
	] as const)('refuses the room %s', (value) => {
		expect(isRoom(value)).toBe(false);
	});

	it.each(['1rem', '16rem', '999rem'])('passes the room %s', (value) => {
		expect(isRoom(value)).toBe(true);
	});

	it('passes every real stories file', () => {
		const storyFiles = packageSources().filter((source) => isStoriesFile(source.path));
		expect(storyFiles.flatMap(storyFileRefusals)).toEqual([]);
	});
});
