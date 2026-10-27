import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { parse } from 'svelte/compiler';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { SAMPLE_SUBSTITUTIONS } from './tokens/copy';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));

/** The folder the stories live in, relative to the package. */
const storiesFolder = 'src/stories';

/** The folder the stories' fixtures live in, relative to the package. */
const fixturesFolder = `${storiesFolder}/fixtures`;

/** The folders read whole, every file whatever its extension: the stories, and the gallery parts they render. */
const readFolders: readonly string[] = [storiesFolder, 'src/gallery'];

/** The made-up handle and DID the design system's samples were swapped for. */
const reservedSamples: readonly string[] = SAMPLE_SUBSTITUTIONS.map(
	(substitution) => substitution.replacement
);

const reservedDid = reservedSamples.find((sample) => sample.startsWith('did:'));
if (reservedDid === undefined) throw new Error('the reserved samples hold no DID');

/** The only top-level domains a fixture's handle may end in: reserved, so no real account matches. */
const reservedEndings: readonly string[] = ['.test', '.example'];

/** A fixture or stories file, by its path relative to the package. */
interface FixtureFile {
	readonly path: string;
	readonly text: string;
}

export type FixtureRefusalReason =
	| 'a DID other than the reserved sample'
	| 'a handle outside .test and .example'
	| 'a stories file that does not parse';

export interface FixtureRefusal {
	readonly path: string;
	readonly reason: FixtureRefusalReason;
	readonly text: string;
}

/**
 * Every `did:` that starts a word and isn't followed by a space (a `did:` key
 * or type annotation is), with what follows it up to a space, a quote or
 * punctuation that ends a value.
 */
const didPattern = /\bdid:(?!\s)[^\s'"`()[\]{},;<>]*/gi;

/** An `@` followed directly by a dotted name: `@someone.bsky.social`, and an email's domain. */
const atHandlePattern = /@[a-z0-9-]+(?:\.[a-z0-9-]+)+/gi;

/** A bare dotted name, as a handle field holds it, with or without a leading `@`. */
const domainPattern = /^@?[a-z0-9-]+(?:\.[a-z0-9-]+)+$/i;

/** A field or a variable whose name says it holds a handle. */
const handleNamePattern = /handle/i;

function hasReservedEnding(handle: string): boolean {
	const lower = handle.toLowerCase();
	return reservedEndings.some((ending) => lower.endsWith(ending));
}

/** Every string literal, and every no-substitution template, inside `node`. */
function stringsInside(node: ts.Node): readonly string[] {
	if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return [node.text];
	const found: string[] = [];
	node.forEachChild((child) => {
		found.push(...stringsInside(child));
	});
	return found;
}

/** The name of a property or a variable, when it is written plainly or as a string. */
function plainName(name: ts.Node): string | undefined {
	if (ts.isIdentifier(name) || ts.isStringLiteral(name)) return name.text;
	if (ts.isComputedPropertyName(name) && ts.isStringLiteral(name.expression)) {
		return name.expression.text;
	}
	return undefined;
}

/** The value each handle-named field or variable in a `.ts` or `.json` file holds, as every string written in it. */
function scriptHandleStrings(file: FixtureFile): readonly string[] {
	const source = file.path.endsWith('.json')
		? ts.parseJsonText(file.path, file.text)
		: ts.createSourceFile(file.path, file.text, ts.ScriptTarget.Latest, true);
	const found: string[] = [];
	const visit = (node: ts.Node): void => {
		const isNamedValue = ts.isPropertyAssignment(node) || ts.isVariableDeclaration(node);
		const name = isNamedValue ? plainName(node.name) : undefined;
		const initializer = isNamedValue ? node.initializer : undefined;
		if (name !== undefined && handleNamePattern.test(name) && initializer !== undefined) {
			found.push(...stringsInside(initializer));
		}
		node.forEachChild(visit);
	};
	visit(source);
	return found;
}

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

/** Every object in a parsed Svelte tree, each once. */
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

/** A string literal's value, a no-expression template's text, or a markup text's data. */
function staticText(node: Fields): string | undefined {
	if (node.type === 'Literal') return typeof node.value === 'string' ? node.value : undefined;
	if (node.type === 'Text') return typeof node.data === 'string' ? node.data.trim() : undefined;
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

/** The name a property key, a variable or an attribute spells: an identifier or a static string. */
function svelteName(node: Fields): string | undefined {
	if (node.type === 'Attribute') return typeof node.name === 'string' ? node.name : undefined;
	const key =
		node.type === 'Property' ? node.key : node.type === 'VariableDeclarator' ? node.id : undefined;
	if (!isFields(key) || node.computed === true) return undefined;
	if (key.type === 'Identifier') return typeof key.name === 'string' ? key.name : undefined;
	return staticText(key);
}

/** What a property, a variable or an attribute holds. */
function svelteValue(node: Fields): unknown {
	if (node.type === 'Property' || node.type === 'Attribute') return node.value;
	return node.type === 'VariableDeclarator' ? node.init : undefined;
}

/**
 * The value each handle-named field, variable or attribute in a `.svelte`
 * file holds, in its scripts and its markup, as every string written in it;
 * or the parser's message when the file does not parse.
 */
function svelteHandleStrings(file: FixtureFile): readonly string[] | string {
	let root: unknown;
	try {
		root = parse(file.text, { modern: true });
	} catch (error) {
		return error instanceof Error ? error.message : 'the parser gave up';
	}
	return nodesOf(root)
		.filter((node) => {
			const name = svelteName(node);
			return name !== undefined && handleNamePattern.test(name);
		})
		.flatMap((node) => nodesOf(svelteValue(node)))
		.flatMap((node) => {
			const text = staticText(node);
			return text === undefined ? [] : [text];
		});
}

/**
 * The strings in a file's handle-named fields: read from a `.svelte`, `.ts` or
 * `.json` file's tree; any other file (an `.mdx` page) is read as text alone.
 */
function handleStringsOf(file: FixtureFile): readonly string[] | string {
	if (file.path.endsWith('.svelte')) return svelteHandleStrings(file);
	if (file.path.endsWith('.ts') || file.path.endsWith('.json')) return scriptHandleStrings(file);
	return [];
}

/**
 * Every refusal in one fixture or stories file: any `did:` text but the
 * reserved sample, any `@name.domain` whose domain doesn't end in `.test` or
 * `.example`, anywhere in the text, and the same for a bare domain in a
 * handle-named field or variable (or, in a stories file, attribute).
 */
export function fixtureRefusals(file: FixtureFile): readonly FixtureRefusal[] {
	const refusal = (reason: FixtureRefusalReason, text: string): FixtureRefusal => ({
		path: file.path,
		reason,
		text
	});
	const handleStrings = handleStringsOf(file);
	if (typeof handleStrings === 'string') {
		return [refusal('a stories file that does not parse', handleStrings)];
	}
	const dids = [...file.text.matchAll(didPattern)].map(([text]) => text);
	const atHandles = [...file.text.matchAll(atHandlePattern)].map(([text]) => text);
	const fieldHandles = handleStrings.filter((text) => domainPattern.test(text));
	return [
		...dids
			.filter((did) => did !== reservedDid)
			.map((did) => refusal('a DID other than the reserved sample', did)),
		...[...new Set([...atHandles, ...fieldHandles])]
			.filter((handle) => !hasReservedEnding(handle))
			.map((handle) => refusal('a handle outside .test and .example', handle))
	];
}

/** Every file under the stories and gallery folders, whatever its extension, sorted by path. */
function realFixtures(): readonly FixtureFile[] {
	return readFolders
		.flatMap((relative) => {
			const folder = `${uiRoot}${relative}`;
			const names = existsSync(folder)
				? readdirSync(folder, { recursive: true, encoding: 'utf8' })
				: [];
			return names.map((name) => `${relative}/${name}`);
		})
		.filter((filePath) => statSync(`${uiRoot}${filePath}`).isFile())
		.sort()
		.map((filePath) => ({ path: filePath, text: readFileSync(`${uiRoot}${filePath}`, 'utf8') }));
}

function reasonsFor(text: string): readonly FixtureRefusalReason[] {
	return fixtureRefusals({ path: `${fixturesFolder}/people.ts`, text }).map(
		(refusal) => refusal.reason
	);
}

function storyReasonsFor(text: string): readonly FixtureRefusalReason[] {
	return fixtureRefusals({ path: `${storiesFolder}/controls/Avatar.stories.svelte`, text }).map(
		(refusal) => refusal.reason
	);
}

/** A stories file with `script` in its module script and `markup` after it. */
function storiesFile(script: string, markup: string): string {
	return [
		'<script module lang="ts">',
		"\timport { defineMeta } from '@storybook/addon-svelte-csf';",
		"\tconst { Story } = defineMeta({ title: 'Controls/Avatar' });",
		`\t${script}`,
		'</script>',
		'',
		markup
	].join('\n');
}

describe('story fixtures', () => {
	it('reads the reserved samples from their one constant', () => {
		expect(reservedSamples).toContain(reservedDid);
		expect(reservedDid).toMatch(/^did:plc:[a-z2-7]{24}$/);
	});

	it.each(reservedSamples)('passes the reserved sample %s', (sample) => {
		expect(reasonsFor(`export const sample = '${sample}';`)).toEqual([]);
	});

	it.each([
		['@artist.test', "export const artist = { name: 'Ash', handle: '@artist.test' };"],
		['@studio.example', "export const credit = 'Art by @studio.example';"],
		['a bare handle in .test', "export const artist = { handle: 'artist.test' };"],
		['a handle variable in .example', "export const artistHandle = 'artist.example';"],
		['the reserved DID', `export const artist = { did: '${reservedDid}' };`],
		['a handle field holding a name, not a domain', "export const artist = { handle: 'Ash' };"],
		['a scoped package name', "export const from = '@zurfur/ui';"],
		['plain words', "export const words = 'Send request';"],
		['a did field and its type', 'export interface Person {\n\treadonly did: string;\n}'],
		['a word ending in did', "export const words = 'candid: yes';"]
	] as const)('passes %s', (_label, text) => {
		expect(reasonsFor(text)).toEqual([]);
	});

	it.each([
		['another did:plc value', "export const did = 'did:plc:zzzzzzzzzzzzzzzzzzzzzzzz';"],
		['the reserved DID with more after it', `export const did = '${reservedDid}a';`],
		['the reserved DID with a fragment', `export const did = '${reservedDid}#atproto';`],
		['a did:web', "export const did = 'did:web:zurfur.app';"],
		['a DID in upper case', `export const did = '${reservedDid.toUpperCase()}';`],
		['a DID built in a template', 'const id = 1;\nexport const did = `did:plc:${String(id)}`;'],
		['a DID in a comment', '// did:plc:zzzzzzzzzzzzzzzzzzzzzzzz\nexport const x = 1;'],
		['a bare did: prefix', "export const prefix = 'did:';"]
	] as const)('refuses %s', (_label, text) => {
		expect(reasonsFor(text)).toEqual(['a DID other than the reserved sample']);
	});

	it.each([
		['@someone.bsky.social', "export const artist = { handle: '@someone.bsky.social' };"],
		['@someone.bsky.social in words', "export const credit = 'Art by @someone.bsky.social';"],
		['an email address', "export const contact = 'someone@zurfur.app';"],
		['a .test that is not the last label', "export const credit = '@artist.test.com';"],
		['a lookalike ending', "export const credit = '@artist.testing';"],
		['a bare domain in a handle field', "export const artist = { handle: 'someone.bsky.social' };"],
		['a bare domain in a quoted handle key', "export const artist = { 'handle': 'zurfur.app' };"],
		['a bare domain in a handle variable', "export const ownerHandle = 'someone.bsky.social';"],
		[
			'a bare domain cast in a handle field',
			"export const artist = { handle: 'someone.bsky.social' as string };"
		]
	] as const)('refuses %s', (_label, text) => {
		expect(reasonsFor(text)).toEqual(['a handle outside .test and .example']);
	});

	it.each([
		[
			'@artist.test in the markup',
			storiesFile('', '<Story name="Signed">Art by @artist.test</Story>')
		],
		[
			'@artist.test in a script',
			storiesFile("const artist = { handle: '@artist.test' };", '<Story name="Signed" />')
		],
		[
			'the reserved DID in an arg',
			storiesFile('', `<Story name="Owner" args={{ did: '${reservedDid}' }} />`)
		],
		[
			'a bare handle in .example in an attribute',
			storiesFile('', '<Story name="Owner" handle="studio.example" />')
		],
		['a handle attribute holding a name', storiesFile('', '<Story name="Owner" handle="Ash" />')],
		[
			'a real token page',
			readFileSync(`${uiRoot}${storiesFolder}/tokens/Colour.stories.svelte`, 'utf8')
		]
	] as const)('passes a stories file with %s', (_label, text) => {
		expect(storyReasonsFor(text)).toEqual([]);
	});

	it.each([
		[
			'another did:plc value in the markup',
			storiesFile('', '<Story name="Owner">did:plc:zzzzzzzzzzzzzzzzzzzzzzzz</Story>')
		],
		[
			'another did:plc value in a script',
			storiesFile("const did = 'did:plc:zzzzzzzzzzzzzzzzzzzzzzzz';", '<Story name="Owner" />')
		],
		[
			'another did:plc value in an arg',
			storiesFile('', '<Story name="Owner" args={{ did: "did:plc:zzzzzzzzzzzzzzzzzzzzzzzz" }} />')
		]
	] as const)('refuses a stories file with %s', (_label, text) => {
		expect(storyReasonsFor(text)).toEqual(['a DID other than the reserved sample']);
	});

	it.each([
		[
			'@someone.bsky.social in the markup',
			storiesFile('', '<Story name="Signed">Art by @someone.bsky.social</Story>')
		],
		[
			'@someone.bsky.social in a script',
			storiesFile("const credit = 'Art by @someone.bsky.social';", '<Story name="Signed" />')
		],
		[
			'a bare domain in a handle attribute',
			storiesFile('', '<Story name="Owner" handle="someone.bsky.social" />')
		],
		[
			'a bare domain in a handle arg',
			storiesFile('', '<Story name="Owner" args={{ handle: \'someone.bsky.social\' }} />')
		],
		[
			'a bare domain in a handle variable',
			storiesFile("const ownerHandle = 'someone.bsky.social';", '<Story name="Owner" />')
		]
	] as const)('refuses a stories file with %s', (_label, text) => {
		expect(storyReasonsFor(text)).toEqual(['a handle outside .test and .example']);
	});

	it('refuses a stories file that does not parse', () => {
		expect(storyReasonsFor('<script module lang="ts">\n\tconst {')).toEqual([
			'a stories file that does not parse'
		]);
	});

	it('reads every file under the stories and the gallery, whatever its extension', () => {
		const paths = realFixtures().map((file) => file.path);
		expect(paths).toEqual(
			expect.arrayContaining([
				`${storiesFolder}/tokens/Colour.stories.svelte`,
				`${storiesFolder}/tokens/Colour.mdx`,
				`${storiesFolder}/controls/ToggleHolder.svelte`,
				`${storiesFolder}/controls/args.ts`,
				`${storiesFolder}/controls/room.ts`,
				`${storiesFolder}/NODE.inf.json`,
				`${storiesFolder}/frame/StoryFrame.svelte`,
				'src/gallery/GalleryPage.svelte'
			])
		);
	});

	it.each([
		[
			'a real DID in a holder',
			`${storiesFolder}/controls/ToggleHolder.svelte`,
			'<script lang="ts">\n\tconst owner = \'did:plc:zzzzzzzzzzzzzzzzzzzzzzzz\';\n</script>\n\n<p>{owner}</p>\n',
			'a DID other than the reserved sample'
		],
		[
			'a real handle in a holder',
			`${storiesFolder}/controls/ToggleHolder.svelte`,
			'<p>Art by @someone.bsky.social</p>\n',
			'a handle outside .test and .example'
		],
		[
			'a real DID in room.ts',
			`${storiesFolder}/controls/room.ts`,
			"export const owner = 'did:web:zurfur.app';",
			'a DID other than the reserved sample'
		],
		[
			'a real handle in args.ts',
			`${storiesFolder}/controls/args.ts`,
			"export const ownerHandle = 'someone.bsky.social';",
			'a handle outside .test and .example'
		],
		[
			'a real DID in a docs page',
			`${storiesFolder}/tokens/Colour.mdx`,
			'# Colour\n\nMade by did:plc:zzzzzzzzzzzzzzzzzzzzzzzz.\n',
			'a DID other than the reserved sample'
		],
		[
			'a real handle in a docs page',
			`${storiesFolder}/tokens/Colour.mdx`,
			'# Colour\n\nMade by @someone.bsky.social.\n',
			'a handle outside .test and .example'
		],
		[
			'a real handle in a node file',
			`${storiesFolder}/NODE.inf.json`,
			'{ "short": "x", "owner": { "handle": "someone.bsky.social" } }',
			'a handle outside .test and .example'
		],
		[
			'a real DID in a gallery part',
			'src/gallery/GalleryPage.svelte',
			'<p>did:plc:zzzzzzzzzzzzzzzzzzzzzzzz</p>\n',
			'a DID other than the reserved sample'
		]
	] as const)('refuses %s', (_label, filePath, text, reason) => {
		const read = realFixtures().map((file) => file.path);
		expect(read).toContain(filePath);
		expect(fixtureRefusals({ path: filePath, text }).map((refusal) => refusal.reason)).toEqual([
			reason
		]);
	});

	it('passes every real file under the stories and the gallery', () => {
		expect(realFixtures().flatMap(fixtureRefusals)).toEqual([]);
	});
});
