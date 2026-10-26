/*
 * The docs pages: MDX can carry code, so every `.mdx` file under src/stories/
 * is held to a line grammar of words and story references. A page imports
 * Storybook's doc blocks and its own stories file from the same folder, names
 * that file in one `<Meta of={…} />`, and otherwise holds blank lines,
 * Markdown headings, plain prose and `<Story of={…} />` lines. Anything else
 * (another import or JSX, `{` outside `of={…}`, `export`, an image, a link,
 * raw HTML, a URL) is refused.
 */
import { describe, expect, it } from 'vitest';
import { lstatSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';

/** The stories folder, from the spec's own path, as the forbidden-files check finds src/. */
function storiesRoot(): string {
	const { testPath } = expect.getState();
	if (testPath === undefined) throw new Error('no test path outside a test');
	return path.resolve(path.dirname(testPath), '..', 'stories');
}

/** The one import of Storybook's doc blocks a page may carry, exactly as written. */
const blocksImport = "import { Meta, Story } from '@storybook/addon-docs/blocks';";

/** An identifier a page may bind or name: a capital, then letters and digits. */
const name = '[A-Z][A-Za-z0-9]*';

const storiesImport = new RegExp(
	`^import \\* as (${name}) from '\\./(${name})\\.stories\\.svelte';$`
);
const metaLine = new RegExp(`^<Meta of=\\{(${name})\\} />$`);
const storyLine = new RegExp(`^<Story of=\\{(${name})\\.(${name})\\} />$`);

/**
 * A heading or a line of prose: letters (any script), digits, spaces and a
 * little punctuation. No character that starts markup, an expression, a link,
 * an image, code, an entity or a path is in the set.
 */
const heading = /^#{1,6} [\p{L}\p{N} ,.;:'’?!-]+$/u;
const prose = /^[\p{L}\p{N}][\p{L}\p{N} ,.;:'’?!-]*$/u;

/** A line MDX reads as JavaScript (an import or export), whatever follows. */
const esmStart = /^(?:import|export)\b/i;

/** Text no line may hold, in any letter case, even inside words the grammar allows. */
const refusedText = ['http', '//', 'data:', 'javascript:', 'www.'] as const;

/** One line the grammar reads, as the rule it matched. */
type PageLine =
	| { readonly kind: 'blank' }
	| { readonly kind: 'blocks' }
	| { readonly kind: 'stories'; readonly binding: string }
	| { readonly kind: 'meta'; readonly of: string }
	| { readonly kind: 'story'; readonly of: string }
	| { readonly kind: 'words' };

function readLine(line: string): PageLine | undefined {
	if (line === '') return { kind: 'blank' };
	if (line === blocksImport) return { kind: 'blocks' };
	const imported = storiesImport.exec(line);
	if (imported !== null) {
		const [, binding, file] = imported;
		return binding !== undefined && binding === file ? { kind: 'stories', binding } : undefined;
	}
	const meta = metaLine.exec(line)?.[1];
	if (meta !== undefined) return { kind: 'meta', of: meta };
	const story = storyLine.exec(line)?.[1];
	if (story !== undefined) return { kind: 'story', of: story };
	if (esmStart.test(line)) return undefined;
	if (heading.test(line) || prose.test(line)) return { kind: 'words' };
	return undefined;
}

/** Why one MDX page's text falls outside the grammar; empty when it holds. */
function docsPageRefusals(text: string): readonly string[] {
	const lines = text.split('\n');
	const read = lines.map(readLine);
	const lineRefusals = lines.flatMap((line, index) => {
		const at = `line ${String(index + 1)}`;
		const lower = line.toLowerCase();
		const found = refusedText.filter((refused) => lower.includes(refused));
		const textRefusals = found.map((refused) => `${at} holds ${JSON.stringify(refused)}`);
		return read[index] === undefined
			? [`${at} is outside the grammar`, ...textRefusals]
			: textRefusals;
	});
	const known = read.filter((line) => line !== undefined);
	const count = (kind: PageLine['kind']): number =>
		known.filter((line) => line.kind === kind).length;
	const bindings = known.flatMap((line) => (line.kind === 'stories' ? [line.binding] : []));
	const named = known.flatMap((line) =>
		line.kind === 'meta' || line.kind === 'story' ? [line.of] : []
	);
	const shapeRefusals = [
		...(count('blocks') === 1 ? [] : ['the doc blocks are not imported exactly once']),
		...(count('stories') === 1 ? [] : ['a stories file is not imported exactly once']),
		...(count('meta') === 1 ? [] : ['the page has no single <Meta of={…} />']),
		...named
			.filter((binding) => !bindings.includes(binding))
			.map((binding) => `${binding} is not the imported stories file`)
	];
	return [...lineRefusals, ...shapeRefusals];
}

/** Every `.mdx` file under `folder`, `/`-separated and relative to it, read without following links. */
function mdxFiles(folder: string, relative = ''): readonly string[] {
	return readdirSync(path.join(folder, relative)).flatMap((entry) => {
		const entryPath = relative === '' ? entry : `${relative}/${entry}`;
		const stats = lstatSync(path.join(folder, entryPath));
		if (stats.isDirectory()) return mdxFiles(folder, entryPath);
		return stats.isFile() && entry.endsWith('.mdx') ? [entryPath] : [];
	});
}

const goodPage = [
	blocksImport,
	"import * as Colour from './Colour.stories.svelte';",
	'',
	'<Meta of={Colour} />',
	'',
	'# Colour',
	'',
	'<Story of={Colour.Page} />',
	''
].join('\n');

/** The good page with `line` added after its title. */
const planted = (line: string): string => goodPage.replace('# Colour\n', `# Colour\n${line}\n`);

describe('the docs pages', () => {
	it('holds every real page to the grammar', () => {
		const root = storiesRoot();
		const refusals = mdxFiles(root).flatMap((file) =>
			docsPageRefusals(readFileSync(path.join(root, file), 'utf8')).map(
				(refusal) => `${file}: ${refusal}`
			)
		);
		expect(refusals).toEqual([]);
	});

	it('passes the page the gallery writes', () => {
		expect(docsPageRefusals(goodPage)).toEqual([]);
	});

	it('passes further headings and plain prose', () => {
		const page = planted("## Paper Light\n\nThe skin's text, on paper; read it twice!");
		expect(docsPageRefusals(page)).toEqual([]);
	});

	it.each([
		['a remote image tag', '<img src="https://example.org/x.png" />'],
		['an export', 'export const x = 1'],
		['another import', "import x from 'react'"],
		['an import read as prose', 'import words'],
		['an export read as prose', 'Export words'],
		['an expression', '{1+1}'],
		['a Markdown image', '![](a.png)'],
		['a Markdown link', '[x](https://example.org)'],
		['a script', '<script>alert(1)</script>'],
		['raw HTML', '<div>x</div>'],
		['JSX', '<Canvas />'],
		['a heading holding an expression', '# Colour {x}'],
		['a heading holding a tag', '# Colour <b>x</b>'],
		['prose holding a URL', 'see https colon example'],
		['prose holding a protocol-relative path', 'see example.org//x'],
		['prose holding a data: URL', 'Data:text'],
		['prose holding javascript:', 'run javascript:alert'],
		['a web address', 'see www.example.org'],
		['code', '`x`'],
		['a reference link', '[x]: /y'],
		['an autolink', '<https://example.org>'],
		['an entity', 'a &lt; b'],
		['a Story of another file', '<Story of={Other.Page} />'],
		['a second Meta', '<Meta of={Colour} />'],
		['a Meta with more props', '<Meta of={Colour} title="x" />'],
		['a carriage return', 'words\r']
	])('refuses %s', (_label, line) => {
		expect(docsPageRefusals(planted(line))).not.toEqual([]);
	});

	it.each([
		['an import from lib', goodPage.replace("'./Colour.stories.svelte'", "'../lib/Colour.svelte'")],
		[
			'a stories file from another folder',
			goodPage.replace("'./Colour.stories.svelte'", "'../tokens/Colour.stories.svelte'")
		],
		[
			'a stories file under another name',
			goodPage.replace("'./Colour.stories.svelte'", "'./Paper.stories.svelte'")
		],
		[
			'the blocks from elsewhere',
			goodPage.replace("'@storybook/addon-docs/blocks'", "'@storybook/blocks'")
		],
		['more doc blocks', goodPage.replace('{ Meta, Story }', '{ Meta, Story, Markdown }')],
		['no Meta', goodPage.replace('<Meta of={Colour} />', '')],
		[
			'no stories import',
			goodPage.replace("import * as Colour from './Colour.stories.svelte';", '')
		]
	])('refuses %s', (_label, page) => {
		expect(docsPageRefusals(page)).not.toEqual([]);
	});
});
