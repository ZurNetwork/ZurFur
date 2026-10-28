import { describe, expect, it } from 'vitest';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { parse } from 'svelte/compiler';
import ts from 'typescript';
import { entryPath, exportedComponents, storiedComponent } from './scan/exports';
import { packageSources, type SourceFile } from './scan/parse';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));

/** UI-3a's exported controls other than Button: the only names the pending list may hold. */
const UI_3A_CONTROLS: readonly string[] = [
	'IconButton',
	'Icon',
	'TextInput',
	'Select',
	'Checkbox',
	'Toggle',
	'Tabs',
	'PillSwitch',
	'Menu',
	'DateField'
];

/**
 * The exported controls that have no stories yet. It only shrinks: each name
 * leaves once its stories land, and no name joins it.
 */
const PENDING: readonly string[] = [];

export type CoverageRefusalReason =
	| 'an exported component without stories'
	| 'an exported component without its contract spec'
	| 'a pending component that has stories'
	| "a pending name outside UI-3a's controls";

export interface CoverageRefusal {
	readonly reason: CoverageRefusalReason;
	readonly name: string;
}

export interface CoverageInput {
	/** The public entry's text. */
	readonly entry: string;
	/** Every stories file under src/stories/. */
	readonly stories: readonly SourceFile[];
	/** True when a file exists, by its path relative to the package. */
	readonly hasFile: (relative: string) => boolean;
	readonly pending: readonly string[];
}

/**
 * Every refusal of the coverage rule: each exported component has a stories
 * file naming it and its `Name.svelte.spec.ts` beside it; the pending list
 * exempts from stories only, holds only UI-3a's controls, and never a
 * component that has stories.
 */
export function coverageRefusals(input: CoverageInput): readonly CoverageRefusal[] {
	const storied = new Set(input.stories.flatMap((file) => storiedComponent(file) ?? []));
	const exported = exportedComponents(input.entry);
	const refusal = (reason: CoverageRefusalReason, name: string): CoverageRefusal => ({
		reason,
		name
	});
	return [
		...exported
			.filter(({ name }) => !storied.has(name) && !input.pending.includes(name))
			.map(({ name }) => refusal('an exported component without stories', name)),
		...exported
			.filter(({ file }) => !input.hasFile(`${file}.spec.ts`))
			.map(({ name }) => refusal('an exported component without its contract spec', name)),
		...input.pending
			.filter((name) => storied.has(name))
			.map((name) => refusal('a pending component that has stories', name)),
		...input.pending
			.filter((name) => !UI_3A_CONTROLS.includes(name))
			.map((name) => refusal("a pending name outside UI-3a's controls", name))
	];
}

/** A stories file at `src/stories/controls/<fileName>`, naming `component` from `from`. */
function storiesOf(fileName: string, component: string, from = '@zurfur/ui'): SourceFile {
	const text = [
		'<script module lang="ts">',
		"\timport { defineMeta } from '@storybook/addon-svelte-csf';",
		`\timport { ${component} } from '${from}';`,
		'',
		`\tconst { Story } = defineMeta({ title: 'Controls/${component}', component: ${component} });`,
		'</script>',
		'',
		'<Story name="Plain" />',
		''
	].join('\n');
	return { path: `src/stories/controls/${fileName}`, text };
}

/** A public entry exporting each name from `./controls/<Name>.svelte`, beside a plain export. */
function entryOf(...names: readonly string[]): string {
	const lines = names.map(
		(name) => `export { default as ${name} } from './controls/${name}.svelte';`
	);
	return ["export { parseSkin, type Skin } from './styles/environment.ts';", ...lines].join('\n');
}

const everyFileExists = (): boolean => true;

describe('story coverage', () => {
	it('reads the components the public entry exports, and nothing else it exports', () => {
		const entry = [
			entryOf('Button', 'Toggle'),
			"export { default as Icon } from './icons/Icon.svelte';",
			"export { ICON_NAMES, isIconName, type IconName } from './icons/icons.ts';",
			"export type { default as Typed } from './controls/Typed.svelte';",
			"export { default } from './controls/Unnamed.svelte';",
			"export * from './controls/Starred.svelte';"
		].join('\n');
		expect(exportedComponents(entry)).toEqual([
			{ name: 'Button', file: 'src/lib/controls/Button.svelte' },
			{ name: 'Toggle', file: 'src/lib/controls/Toggle.svelte' },
			{ name: 'Icon', file: 'src/lib/icons/Icon.svelte' }
		]);
	});

	it.each([
		['a plain import', storiesOf('Button.stories.svelte', 'Button'), 'Button'],
		[
			'an aliased import',
			{
				path: 'src/stories/controls/Button.stories.svelte',
				text: "<script module lang=\"ts\">\n\timport { defineMeta } from '@storybook/addon-svelte-csf';\n\timport { Button as Shown } from '@zurfur/ui';\n\n\tconst { Story } = defineMeta({ component: Shown });\n</script>\n"
			},
			'Button'
		],
		[
			'a component imported by path',
			storiesOf('Button.stories.svelte', 'Button', '../../lib/controls/Button.svelte'),
			undefined
		],
		[
			'no defineMeta',
			{
				path: 'src/stories/controls/X.stories.svelte',
				text: '<script module lang="ts">\n</script>\n'
			},
			undefined
		],
		[
			'a file that does not parse',
			{ path: 'src/stories/controls/X.stories.svelte', text: '<script module lang="ts">' },
			undefined
		]
	] as const)('reads the component of a stories file with %s', (_label, file, expected) => {
		expect(storiedComponent(file)).toBe(expected);
	});

	it('passes a component with its stories and its contract spec', () => {
		const refusals = coverageRefusals({
			entry: entryOf('Button'),
			stories: [storiesOf('Button.stories.svelte', 'Button')],
			hasFile: everyFileExists,
			pending: PENDING
		});
		expect(refusals).toEqual([]);
	});

	it('passes a pending component without stories', () => {
		const refusals = coverageRefusals({
			entry: entryOf('Toggle'),
			stories: [],
			hasFile: everyFileExists,
			pending: ['Toggle']
		});
		expect(refusals).toEqual([]);
	});

	it.each([
		[
			'a new export without stories',
			{ entry: entryOf('Banner'), stories: [], pending: [] },
			[{ reason: 'an exported component without stories', name: 'Banner' }]
		],
		[
			'a pending name that has stories',
			{
				entry: entryOf('Toggle'),
				stories: [storiesOf('Toggle.stories.svelte', 'Toggle')],
				pending: ['Toggle']
			},
			[{ reason: 'a pending component that has stories', name: 'Toggle' }]
		],
		[
			'Banner in the pending list',
			{ entry: entryOf(), stories: [], pending: ['Banner'] },
			[{ reason: "a pending name outside UI-3a's controls", name: 'Banner' }]
		],
		[
			'Button in the pending list',
			{ entry: entryOf('Button'), stories: [], pending: ['Button'] },
			[{ reason: "a pending name outside UI-3a's controls", name: 'Button' }]
		],
		[
			'an export whose stories file names another component',
			{
				entry: entryOf('Button', 'Toggle'),
				stories: [storiesOf('Button.stories.svelte', 'Toggle')],
				pending: []
			},
			[{ reason: 'an exported component without stories', name: 'Button' }]
		],
		[
			'an export whose stories import it by path',
			{
				entry: entryOf('Button'),
				stories: [storiesOf('Button.stories.svelte', 'Button', '../../lib/controls/Button.svelte')],
				pending: []
			},
			[{ reason: 'an exported component without stories', name: 'Button' }]
		]
	] as const)('refuses %s', (_label, input, expected) => {
		expect(coverageRefusals({ ...input, hasFile: everyFileExists })).toEqual(expected);
	});

	it('refuses an export without its contract spec, pending or not', () => {
		const refusals = coverageRefusals({
			entry: entryOf('Button', 'Toggle'),
			stories: [storiesOf('Button.stories.svelte', 'Button')],
			hasFile: (relative) => relative === 'src/lib/controls/Button.svelte',
			pending: ['Toggle']
		});
		expect(refusals).toEqual([
			{ reason: 'an exported component without its contract spec', name: 'Button' },
			{ reason: 'an exported component without its contract spec', name: 'Toggle' }
		]);
	});

	it("pins the pending list to UI-3a's controls, each once", () => {
		expect(PENDING.filter((name) => !UI_3A_CONTROLS.includes(name))).toEqual([]);
		expect(new Set(PENDING).size).toBe(PENDING.length);
	});

	it('passes the real public entry and stories', () => {
		const stories = packageSources().filter(
			(source) => source.path.startsWith('src/stories/') && source.path.endsWith('.stories.svelte')
		);
		const refusals = coverageRefusals({
			entry: readFileSync(`${uiRoot}${entryPath}`, 'utf8'),
			stories,
			hasFile: (relative) => existsSync(`${uiRoot}${relative}`),
			pending: PENDING
		});
		expect(refusals).toEqual([]);
	});
});

// --- Title Case ------------------------------------------------------------------

/** The short words that stay in lower case inside a name; the first word never does. */
const SHORT_WORDS: ReadonlySet<string> = new Set([
	'a',
	'an',
	'and',
	'of',
	'the',
	'to',
	'in',
	'on',
	'for',
	'with',
	'by'
]);

/** A word that starts with a capital letter. */
const capitalised = /^[\p{Lu}\p{Lt}]/u;

/** A word that starts with a letter; one that starts with a digit or a symbol passes as written. */
const startsWithLetter = /^\p{L}/u;

/**
 * The words of `name` that break Title Case: each word starts with a capital,
 * but for the short words after the first, and a word that starts with a digit
 * or a symbol passes. An empty name breaks it too.
 */
export function titleCaseBreaks(name: string): readonly string[] {
	const words = name.split(/\s+/u).filter((word) => word !== '');
	if (words.length === 0) return ['an empty name'];
	return words.filter((word, index) => {
		if (!startsWithLetter.test(word) || capitalised.test(word)) return false;
		return index === 0 || !SHORT_WORDS.has(word);
	});
}

/** One name the Title Case rule refuses, and where. */
export interface TitleRefusal {
	readonly file: string;
	readonly name: string;
	readonly why: string;
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
		for (const [key, child] of Object.entries(value)) {
			if (key !== 'parent') visit(child);
		}
	};
	visit(root);
	return found;
}

/** A string literal's text, or a template literal's with no expressions; undefined for anything else. */
function staticText(node: unknown): string | undefined {
	if (!isFields(node)) return undefined;
	if (node.type === 'Literal') return typeof node.value === 'string' ? node.value : undefined;
	if (node.type !== 'TemplateLiteral') return undefined;
	const expressions: readonly unknown[] = Array.isArray(node.expressions) ? node.expressions : [];
	const quasis: readonly unknown[] = Array.isArray(node.quasis) ? node.quasis : [];
	const [only] = quasis;
	if (expressions.length > 0 || quasis.length !== 1 || !isFields(only)) return undefined;
	return isFields(only.value) && typeof only.value.cooked === 'string'
		? only.value.cooked
		: undefined;
}

/** A static attribute value's text: plain text, or one expression holding a string. */
function attributeText(value: unknown): string | undefined {
	const parts: readonly unknown[] = Array.isArray(value) ? value : [value];
	const [only] = parts;
	if (parts.length !== 1 || !isFields(only)) return undefined;
	if (only.type === 'Text') return typeof only.data === 'string' ? only.data : undefined;
	return only.type === 'ExpressionTag' ? staticText(only.expression) : undefined;
}

/** The key a property is written with, when it is an identifier or a string. */
function keyName(node: unknown): string | undefined {
	if (!isFields(node)) return undefined;
	if (node.type === 'Identifier') return typeof node.name === 'string' ? node.name : undefined;
	return staticText(node);
}

/**
 * Every Title Case refusal in a stories file: its `defineMeta` title, each
 * `/`-separated segment, and every `<Story name>`. A title or a name the
 * check can't read as one string is refused, and so is a file with no
 * title, since Storybook would make one from the file's path.
 */
export function storyTitleRefusals(file: SourceFile): readonly TitleRefusal[] {
	const refuse = (name: string, why: string): TitleRefusal => ({ file: file.path, name, why });
	let root: unknown;
	try {
		root = parse(file.text, { modern: true });
	} catch {
		return [refuse('', 'a stories file that does not parse')];
	}
	const nodes = nodesOf(root);
	const metas = nodes.filter(
		(node) =>
			node.type === 'CallExpression' &&
			isFields(node.callee) &&
			node.callee.type === 'Identifier' &&
			node.callee.name === 'defineMeta'
	);
	const titles = metas.flatMap((meta) => {
		const callArguments: readonly unknown[] = Array.isArray(meta.arguments) ? meta.arguments : [];
		const [argument] = callArguments;
		const properties: readonly unknown[] =
			isFields(argument) && Array.isArray(argument.properties) ? argument.properties : [];
		return properties.filter(
			(property) =>
				isFields(property) &&
				property.type === 'Property' &&
				property.computed !== true &&
				keyName(property.key) === 'title'
		);
	});
	const titleRefusals =
		titles.length === 0
			? [refuse('', 'no defineMeta title')]
			: titles.flatMap((property) => {
					const title = isFields(property) ? staticText(property.value) : undefined;
					if (title === undefined) return [refuse('', 'a title that is not one string')];
					return title
						.split('/')
						.flatMap((segment) =>
							titleCaseBreaks(segment).map((word) => refuse(title, `"${segment}": ${word}`))
						);
				});
	const stories = nodes.filter((node) => node.type === 'Component' && node.name === 'Story');
	const nameRefusals = stories.flatMap((story) => {
		const attributes: readonly unknown[] = Array.isArray(story.attributes) ? story.attributes : [];
		return attributes
			.filter((attribute) => isFields(attribute) && attribute.name === 'name')
			.flatMap((attribute) => {
				const name = isFields(attribute) ? attributeText(attribute.value) : undefined;
				if (name === undefined) return [refuse('', 'a story name that is not one string')];
				return titleCaseBreaks(name).map((word) => refuse(name, word));
			});
	});
	return [...titleRefusals, ...nameRefusals];
}

/** Every fixture `title`, a string under a key named `title`, that breaks Title Case. */
export function fixtureTitleRefusals(file: SourceFile): readonly TitleRefusal[] {
	const source = ts.createSourceFile(file.path, file.text, ts.ScriptTarget.Latest, true);
	const found: TitleRefusal[] = [];
	const visit = (node: ts.Node): void => {
		if (ts.isPropertyAssignment(node)) {
			const key = node.name;
			const isTitle = (ts.isIdentifier(key) || ts.isStringLiteral(key)) && key.text === 'title';
			const value = node.initializer;
			const isText = ts.isStringLiteral(value) || ts.isNoSubstitutionTemplateLiteral(value);
			if (isTitle && isText) {
				for (const word of titleCaseBreaks(value.text)) {
					found.push({ file: file.path, name: value.text, why: word });
				}
			}
		}
		ts.forEachChild(node, visit);
	};
	visit(source);
	return found;
}

/**
 * Fixture titles held back from the rule, each by file and text. It only
 * shrinks. "Commission title" labels a field rather than titling anything:
 * it waits on whether a field's label counts as a title.
 */
const TITLE_CASE_PENDING: readonly Omit<TitleRefusal, 'why'>[] = [
	{ file: 'src/stories/fixtures/fields.ts', name: 'Commission title' }
];

/** True when a refusal is held back by the pending list. */
const isPending = (refusal: TitleRefusal): boolean =>
	TITLE_CASE_PENDING.some(
		(pending) => pending.file === refusal.file && pending.name === refusal.name
	);

/** Every stories file under src/stories/. */
const storiesFiles = (): readonly SourceFile[] =>
	packageSources().filter(
		(source) => source.path.startsWith('src/stories/') && source.path.endsWith('.stories.svelte')
	);

/** Every fixture module in src/stories/fixtures/. */
function fixtureFiles(): readonly SourceFile[] {
	const folder = 'src/stories/fixtures';
	const root = `${uiRoot}${folder}`;
	if (!existsSync(root)) return [];
	return readdirSync(root)
		.filter((name) => name.endsWith('.ts'))
		.sort()
		.map((name) => {
			const path = `${folder}/${name}`;
			return { path, text: readFileSync(`${uiRoot}${path}`, 'utf8') };
		});
}

/** A stories file with `title` and the `<Story>` lines given. */
function storiesWith(title: string, ...stories: readonly string[]): SourceFile {
	const text = [
		'<script module lang="ts">',
		"\timport { defineMeta } from '@storybook/addon-svelte-csf';",
		"\timport { Button } from '@zurfur/ui';",
		'',
		`\tconst { Story } = defineMeta({ ${title}component: Button });`,
		'</script>',
		'',
		...stories,
		''
	].join('\n');
	return { path: 'src/stories/controls/Button.stories.svelte', text };
}

describe('Title Case', () => {
	it.each([
		'Controls/Button',
		'Tokens and Paper',
		'Surfaces and Feedback',
		'With an Icon',
		'Steps From a Day',
		"I'm 18+",
		'18+ Only',
		'Disabled Does Nothing',
		'A Story in a Frame',
		'(Draft) Card',
		'Écran'
	])('passes %s', (name) => {
		expect(name.split('/').flatMap((segment) => titleCaseBreaks(segment))).toEqual([]);
	});

	it.each([
		['suggestive blurred', ['suggestive', 'blurred']],
		['surfaces and feedback', ['surfaces', 'feedback']],
		['and Then', ['and']],
		['With A Lowercase word', ['word']],
		['écran', ['écran']],
		['', ['an empty name']]
	])('refuses %j', (name, breaks) => {
		expect(titleCaseBreaks(name)).toEqual(breaks);
	});

	it('passes a stories file in Title Case', () => {
		const file = storiesWith(
			"title: 'Surfaces and Feedback/Mature Cover', ",
			'<Story name="Rated Adult" />'
		);
		expect(storyTitleRefusals(file)).toEqual([]);
	});

	it.each([
		[
			'the story name "suggestive blurred"',
			storiesWith(
				"title: 'Surfaces and Feedback/Mature Cover', ",
				'<Story name="suggestive blurred" />'
			),
			['suggestive', 'blurred']
		],
		[
			'a title segment "surfaces and feedback"',
			storiesWith("title: 'surfaces and feedback/Mature Cover', ", '<Story name="Plain" />'),
			['"surfaces and feedback": surfaces', '"surfaces and feedback": feedback']
		],
		[
			'a story name in braces',
			storiesWith("title: 'Controls/Button', ", "<Story name={'plain'} />"),
			['plain']
		],
		[
			'a story name read from a variable',
			storiesWith("title: 'Controls/Button', ", '<Story name={NAME} />'),
			['a story name that is not one string']
		],
		[
			'a title read from a variable',
			storiesWith('title: TITLE, ', '<Story name="Plain" />'),
			['a title that is not one string']
		],
		['no title', storiesWith('', '<Story name="Plain" />'), ['no defineMeta title']],
		[
			'a file that does not parse',
			{ path: 'src/stories/controls/X.stories.svelte', text: '<script module lang="ts">' },
			['a stories file that does not parse']
		]
	])('refuses %s', (_label, file, whys) => {
		expect(storyTitleRefusals(file).map((refusal) => refusal.why)).toEqual(whys);
	});

	it.each([
		["title: 'rated adult'", ['rated', 'adult']],
		["'title': `graphic`", ['graphic']],
		["rating: { title: 'Rated Suggestive', show: 'Show art' }", []],
		["label: 'Show art'", []],
		["titleExample: 'a dragon asleep'", []]
	])('reads the fixture %s', (fields, breaks) => {
		const file = {
			path: 'src/stories/fixtures/x.ts',
			text: `export const WORDS = { ${fields} } as const;`
		};
		expect(fixtureTitleRefusals(file).map((refusal) => refusal.why)).toEqual(breaks);
	});

	it('passes the real stories files', () => {
		const files = storiesFiles();
		expect(files.length).toBeGreaterThan(0);
		expect(files.flatMap(storyTitleRefusals)).toEqual([]);
	});

	it('passes the real fixtures, the pending titles aside', () => {
		const files = fixtureFiles();
		expect(files.map((file) => file.path)).toContain('src/stories/fixtures/fields.ts');
		const refusals = files.flatMap(fixtureTitleRefusals);
		expect(refusals.filter((refusal) => !isPending(refusal))).toEqual([]);
	});

	it('holds back only titles that still break the rule', () => {
		const refused = fixtureFiles().flatMap(fixtureTitleRefusals);
		const stale = TITLE_CASE_PENDING.filter(
			(pending) =>
				!refused.some((refusal) => refusal.file === pending.file && refusal.name === pending.name)
		);
		expect(stale).toEqual([]);
	});
});
