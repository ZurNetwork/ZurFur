import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { parse } from 'svelte/compiler';
import path from 'node:path';
import { packageSources, type SourceFile } from './scan/parse';

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
	| 'an a11y setting';

/** The packages a stories file may import, each exactly as written: `@zurfur/ui` by its public entry only. */
const allowedPackages: ReadonlySet<string> = new Set([
	'@storybook/addon-svelte-csf',
	'storybook/test',
	'@zurfur/ui'
]);

/** The folders a stories file may import relatively: the stories and the token pages' parts. */
const allowedFolders: readonly string[] = ['src/stories/', 'src/gallery/'];

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
 * src/gallery/; any `<style>`, `style=` or `style:`; and any `a11y` key.
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

	return [
		...importRefusals,
		...computedImports.map(() => refusal('a computed import', 'import(…)')),
		...styleRefusals,
		...a11yRefusals
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

	it('passes every real stories file', () => {
		const storyFiles = packageSources().filter((source) => isStoriesFile(source.path));
		expect(storyFiles.flatMap(storyFileRefusals)).toEqual([]);
	});
});
