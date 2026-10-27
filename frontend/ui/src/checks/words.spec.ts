import { describe, expect, it } from 'vitest';
import { parse } from 'svelte/compiler';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));
const libRoot = fileURLToPath(new URL('../lib/', import.meta.url));

/** The folders whose components show no word of their own: every word is a prop. */
const wordlessFolders = ['controls', 'icons', 'surfaces'] as const;

/** The attributes whose value a person reads or hears. */
const spokenAttributes: ReadonlySet<string> = new Set([
	'aria-label',
	'aria-description',
	'aria-roledescription',
	'aria-valuetext',
	'title',
	'placeholder',
	'alt',
	'label'
]);

/** A word: any letter, in any script. Punctuation, digits and spaces alone are not words. */
const letter = /\p{L}/u;

/** One literal word a component shows. */
interface Finding {
	readonly file: string;
	readonly where: string;
	readonly text: string;
}

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return typeof value === 'object' && value !== null;
}

/** Every string literal and template text inside an expression. */
function literalTexts(expression: unknown): readonly string[] {
	if (Array.isArray(expression)) return expression.flatMap(literalTexts);
	if (!isFields(expression)) return [];
	const own: readonly string[] =
		expression.type === 'Literal' && typeof expression.value === 'string'
			? [expression.value]
			: expression.type === 'TemplateElement' &&
				  isFields(expression.value) &&
				  typeof expression.value.cooked === 'string'
				? [expression.value.cooked]
				: [];
	const children = Object.entries(expression)
		.filter(([key]) => key !== 'type' && key !== 'loc' && key !== 'start' && key !== 'end')
		.flatMap(([, value]) => literalTexts(value));
	return [...own, ...children];
}

/** The literal texts of an attribute's value: its text parts and the strings in its expressions. */
function attributeTexts(value: unknown): readonly string[] {
	if (value === true) return [];
	const parts: readonly unknown[] = Array.isArray(value) ? value : [value];
	return parts.flatMap((part) => {
		if (!isFields(part)) return [];
		if (part.type === 'Text' && typeof part.data === 'string') return [part.data];
		if (part.type === 'ExpressionTag') return literalTexts(part.expression);
		return [];
	});
}

/**
 * Every word a component's markup holds as a literal: non-blank text, a
 * string literal rendered as text, and a literal in a spoken attribute on an
 * element or a child component. Script, style and attributes no one reads
 * (class, data-*, the rest) are not markup words.
 */
function literalWords(file: string, text: string): readonly Finding[] {
	const ast = parse(text, { modern: true });
	const findings: Finding[] = [];
	const visit = (node: unknown, where: string): void => {
		if (Array.isArray(node)) {
			for (const child of node) visit(child, where);
			return;
		}
		if (!isFields(node)) return;
		const name = typeof node.name === 'string' ? node.name : where;
		if (node.type === 'Text' && typeof node.data === 'string' && letter.test(node.data)) {
			findings.push({ file, where: `<${where}>`, text: node.data.trim() });
		}
		if (node.type === 'ExpressionTag') {
			for (const literal of literalTexts(node.expression)) {
				if (letter.test(literal)) findings.push({ file, where: `<${where}> {…}`, text: literal });
			}
		}
		if (node.type === 'Attribute' && typeof node.name === 'string') {
			if (spokenAttributes.has(node.name)) {
				for (const literal of attributeTexts(node.value)) {
					if (letter.test(literal)) {
						findings.push({ file, where: `<${where} ${node.name}>`, text: literal.trim() });
					}
				}
			}
			return;
		}
		for (const [key, value] of Object.entries(node)) {
			if (key === 'expression' || key === 'metadata') continue;
			visit(value, name);
		}
	};
	visit(ast.fragment, 'markup');
	return findings;
}

/** Every `.svelte` file in the wordless folders, relative to src/lib. */
function wordlessComponents(): readonly string[] {
	return wordlessFolders.flatMap((folder) => {
		const root = path.join(libRoot, folder);
		if (!existsSync(root)) return [];
		return readdirSync(root, { recursive: true, encoding: 'utf8' })
			.filter((relative) => relative.endsWith('.svelte'))
			.map((relative) => `${folder}/${relative.split(path.sep).join('/')}`);
	});
}

/** A component whose markup is `markup`, with a `label` and an `onPress` in scope. */
const component = (markup: string): string =>
	`<script lang="ts">\n\tinterface Props {\n\t\treadonly label: string;\n\t\treadonly onPress: () => void;\n\t}\n\tlet { label, onPress }: Props = $props();\n</script>\n\n${markup}\n`;

describe('every word a control shows is a prop', () => {
	it('reads the real components', () => {
		expect(wordlessComponents()).toContain('icons/Icon.svelte');
	});

	it('finds no literal word in the real controls and icons', () => {
		const findings = wordlessComponents().flatMap((relative) =>
			literalWords(relative, readFileSync(path.join(libRoot, relative), 'utf8'))
		);
		expect(findings).toEqual([]);
	});

	it.each([
		[
			'a literal Close in an IconButton',
			'controls/IconButton.svelte',
			'<button onclick={onPress}>Close</button>'
		],
		[
			'aria-label="Open menu" in a Menu',
			'controls/Menu.svelte',
			'<button aria-label="Open menu" onclick={onPress}>{label}</button>'
		],
		[
			'a word beside a prop',
			'controls/Button.svelte',
			'<button onclick={onPress}>{label} now</button>'
		],
		[
			'a word as a string in markup',
			'controls/Button.svelte',
			"<button onclick={onPress}>{'Send'}</button>"
		],
		[
			'a word in a conditional',
			'controls/Button.svelte',
			"<button onclick={onPress}>{label === '' ? 'Untitled' : label}</button>"
		],
		[
			'a word inside a block',
			'controls/Tabs.svelte',
			'{#if label === ""}<p>Nothing here</p>{:else}<p>{label}</p>{/if}'
		],
		[
			'a title',
			'controls/Button.svelte',
			'<button title="More" onclick={onPress}>{label}</button>'
		],
		[
			'a placeholder',
			'controls/TextInput.svelte',
			'<input placeholder="Search" aria-label={label} />'
		],
		['an alt', 'icons/Icon.svelte', '<img src="./a.png" alt="Avatar" />'],
		[
			'aria-description',
			'controls/Button.svelte',
			'<button aria-description="Sends it" onclick={onPress}>{label}</button>'
		],
		[
			'aria-roledescription',
			'controls/Button.svelte',
			'<button aria-roledescription="switch" onclick={onPress}>{label}</button>'
		],
		[
			'aria-valuetext',
			'controls/Button.svelte',
			'<div role="slider" aria-valuenow={1} aria-valuetext="One">{label}</div>'
		],
		[
			'a label prop handed to a child component',
			'controls/Menu.svelte',
			'<script lang="ts">\n\timport Button from \'./Button.svelte\';\n</script>\n\n<Button label="More" />'
		],
		[
			'a templated aria-label',
			'controls/Menu.svelte',
			'<button aria-label="{label} menu" onclick={onPress}>x</button>'
		],
		[
			'a word in an svg title',
			'icons/Icon.svelte',
			'<svg viewBox="0 0 1 1"><title>Star</title></svg>'
		],
		[
			'a word in another script',
			'controls/Button.svelte',
			'<button onclick={onPress}>Отправить</button>'
		]
	])('refuses %s', (_label, file, markup) => {
		const text = markup.startsWith('<script') ? markup : component(markup);
		expect(literalWords(file, text)).not.toEqual([]);
	});

	it.each([
		[
			'words as props',
			'<button aria-label={label} title={label} onclick={onPress}>{label}</button>'
		],
		['punctuation and digits', '<span aria-hidden="true">— 1/2 …</span><span>{label}</span>'],
		['whitespace', '<p>\n\t{label}\n</p>'],
		[
			'attributes no one reads',
			'<button class="zf-btn placed" data-testid="send" type="button" role="switch" aria-checked="true" onclick={onPress}>{label}</button>'
		],
		['an empty alt', '<img src="./a.png" alt="" />'],
		['a word in a style block', '<p>{label}</p>\n<style>\n\tp { font-family: serif; }\n</style>'],
		['a class expression', "<p class={['zf-tip', label === '' ? 'empty' : 'full']}>{label}</p>"]
	])('allows %s', (_label, markup) => {
		expect(literalWords('controls/Button.svelte', component(markup))).toEqual([]);
	});
});

// --- Words no screen shows -------------------------------------------------------

/** One word or mark no screen shows, found in a file. */
interface Refusal {
	readonly file: string;
	readonly line: number;
	readonly found: string;
}

/**
 * The middle dot as written: the character itself, and its escapes in CSS
 * (`\b7`, `\00b7`, up to six hex digits), in script (`\xb7`, `\u00b7`,
 * `\u{b7}`) and in markup (`&middot;`, `&#183;`, `&#xb7;`).
 */
const middleDot =
	/\u00b7|\\0{0,4}b7(?![0-9a-f])|\\x[bB]7|\\u00[bB]7|\\u\{0*[bB]7\}|&middot;|&#0*183;|&#x0*b7;/gi;

/** A phrase, matched as whole words in any letter case, its words split by spaces, hyphens or underscores. */
const phrase = (...words: readonly string[]): RegExp =>
	new RegExp(`(?<![\\p{L}\\p{N}])${words.join('[\\s_-]+')}(?![\\p{L}\\p{N}])`, 'giu');

/** The age-check and ID-check words: Zurfur asks no one to prove their age or who they are. */
const ageCheckPhrases: readonly RegExp[] = [
	phrase('age', 'check'),
	phrase('id', 'check'),
	phrase('verify', 'your', 'age'),
	phrase('age', 'verification')
];

/** Every middle dot and age-check phrase in one file's text, with its line. */
export function wordRefusals(file: string, text: string): readonly Refusal[] {
	const lineOf = (offset: number): number => text.slice(0, offset).split('\n').length;
	return [middleDot, ...ageCheckPhrases].flatMap((pattern) =>
		[...text.matchAll(pattern)].map((match) => ({
			file,
			line: lineOf(match.index),
			found: match[0]
		}))
	);
}

/**
 * Every file the words rule reads, relative to the package: all of `src/lib/`
 * and `src/stories/` (the fixtures in `src/stories/fixtures/` among them) and
 * the fixture pictures in `static/fixtures/`. A folder's `NODE.inf.json`
 * describes it to people working here and is never shown, so it is not read.
 */
function wordedFiles(): readonly string[] {
	return ['src/lib', 'src/stories', 'static/fixtures'].flatMap((folder) => {
		const root = path.join(uiRoot, folder);
		if (!existsSync(root)) return [];
		return readdirSync(root, { recursive: true, withFileTypes: true })
			.filter((entry) => entry.isFile() && entry.name !== 'NODE.inf.json')
			.map((entry) =>
				path.relative(uiRoot, path.join(entry.parentPath, entry.name)).split(path.sep).join('/')
			);
	});
}

describe('no middle dot and no age-check words', () => {
	it('reads the library, the stories, their fixtures and the fixture pictures', () => {
		const files = wordedFiles();
		expect(files).toContain('src/lib/controls/Button.svelte');
		expect(files).toContain('src/stories/fixtures/fields.ts');
		expect(files).toContain('static/fixtures/avatar.svg');
		expect(files.filter((file) => file.endsWith('NODE.inf.json'))).toEqual([]);
	});

	it('finds none in the real files', () => {
		const refusals = wordedFiles().flatMap((file) =>
			wordRefusals(file, readFileSync(path.join(uiRoot, file), 'utf8'))
		);
		expect(refusals).toEqual([]);
	});

	it.each([
		[
			'· in a stories fixture',
			'src/stories/fixtures/people.ts',
			"export const line = 'Sketch · WIP';"
		],
		['· in a fixture picture', 'static/fixtures/avatar.svg', '<svg><title>A · B</title></svg>'],
		[
			'\\00b7 in a CSS content',
			'src/lib/styles/surfaces/label.css',
			".x::after { content: '\\00b7'; }"
		],
		[
			'\\b7 in a CSS content',
			'src/lib/controls/Button.svelte',
			"<style>.x::before { content: '\\b7 '; }</style>"
		],
		['\\0000B7 in a CSS content', 'src/lib/x.css', ".x::after { content: '\\0000B7'; }"],
		['\\u00b7 in a script', 'src/lib/x.ts', "export const dot = '\\u00b7';"],
		['\\u{b7} in a script', 'src/lib/x.ts', "export const dot = '\\u{b7}';"],
		['\\xB7 in a script', 'src/lib/x.ts', "export const dot = '\\xB7';"],
		['&middot; in markup', 'src/stories/X.stories.svelte', '<p>a &middot; b</p>'],
		['&#183; in markup', 'src/stories/X.stories.svelte', '<p>a &#183; b</p>'],
		['&#xB7; in markup', 'src/stories/X.stories.svelte', '<p>a &#xB7; b</p>'],
		[
			'"Verify your age" in a fixture',
			'src/stories/fixtures/mature.ts',
			"export const gate = 'Verify your age';"
		],
		[
			'"AGE-CHECK" in a component',
			'src/lib/surfaces/MatureCover.svelte',
			'<p>{label} AGE-CHECK</p>'
		],
		['"age check" in a component', 'src/lib/surfaces/X.svelte', '<p>age check</p>'],
		['"Age verification" in a docs page', 'src/stories/surfaces/X.mdx', '# Age verification'],
		['"ID check" in a story', 'src/stories/X.stories.svelte', '<Story name="ID Check" />'],
		['"id-check" in a fixture picture', 'static/fixtures/avatar.svg', '<svg id="id-check"></svg>'],
		['"verify  your\\nage" across a line', 'src/lib/x.ts', '// verify  your\nage']
	])('refuses %s', (_label, file, text) => {
		expect(wordRefusals(file, text)).not.toEqual([]);
	});

	it.each([
		['the verified mark', "export const seal = 'Verified';"],
		['the package-check icon', "export const icon = 'package-check';"],
		['a package check', '<p>package check</p>'],
		['an age without a check', '<p>Your age stays yours.</p>'],
		['a valid check', '<p>valid check</p>'],
		['a bullet', "export const sep = '•';"],
		['a hex colour ending in b7', '.x { color: #a0b7c1; }'],
		['an escape of another character', ".x::after { content: '\\b70'; }"]
	])('allows %s', (_label, text) => {
		expect(wordRefusals('src/lib/x.ts', text)).toEqual([]);
	});
});
