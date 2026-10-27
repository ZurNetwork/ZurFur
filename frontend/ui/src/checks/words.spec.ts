import { describe, expect, it } from 'vitest';
import { parse } from 'svelte/compiler';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const libRoot = fileURLToPath(new URL('../lib/', import.meta.url));

/** The folders whose components show no word of their own: every word is a prop. */
const wordlessFolders = ['controls', 'icons'] as const;

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
		['punctuation and digits', '<span aria-hidden="true">· — 1/2 …</span><span>{label}</span>'],
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
