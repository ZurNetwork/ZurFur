import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { SAMPLE_SUBSTITUTIONS } from './tokens/copy';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));

/** The folder the stories' fixtures live in, relative to the package. */
const fixturesFolder = 'src/stories/fixtures';

/** The made-up handle and DID the design system's samples were swapped for. */
const reservedSamples: readonly string[] = SAMPLE_SUBSTITUTIONS.map(
	(substitution) => substitution.replacement
);

const reservedDid = reservedSamples.find((sample) => sample.startsWith('did:'));
if (reservedDid === undefined) throw new Error('the reserved samples hold no DID');

/** The only top-level domains a fixture's handle may end in: reserved, so no real account matches. */
const reservedEndings: readonly string[] = ['.test', '.example'];

/** A fixture file, by its path relative to the package. */
interface FixtureFile {
	readonly path: string;
	readonly text: string;
}

export type FixtureRefusalReason =
	'a DID other than the reserved sample' | 'a handle outside .test and .example';

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

/** The value each handle-named field or variable holds, as every string written in it. */
function handleFieldStrings(file: FixtureFile): readonly string[] {
	const source = ts.createSourceFile(file.path, file.text, ts.ScriptTarget.Latest, true);
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

/**
 * Every refusal in one fixture: any `did:` text but the reserved sample, any
 * `@name.domain` whose domain doesn't end in `.test` or `.example`, and the
 * same for a bare domain in a handle-named field or variable.
 */
export function fixtureRefusals(file: FixtureFile): readonly FixtureRefusal[] {
	const refusal = (reason: FixtureRefusalReason, text: string): FixtureRefusal => ({
		path: file.path,
		reason,
		text
	});
	const dids = [...file.text.matchAll(didPattern)].map(([text]) => text);
	const atHandles = [...file.text.matchAll(atHandlePattern)].map(([text]) => text);
	const fieldHandles = handleFieldStrings(file).filter((text) => domainPattern.test(text));
	return [
		...dids
			.filter((did) => did !== reservedDid)
			.map((did) => refusal('a DID other than the reserved sample', did)),
		...[...new Set([...atHandles, ...fieldHandles])]
			.filter((handle) => !hasReservedEnding(handle))
			.map((handle) => refusal('a handle outside .test and .example', handle))
	];
}

/** Every `.ts` file in the fixtures folder, or none while it doesn't exist. */
function realFixtures(): readonly FixtureFile[] {
	const folder = `${uiRoot}${fixturesFolder}`;
	const names = existsSync(folder) ? readdirSync(folder) : [];
	return names
		.filter((name) => name.endsWith('.ts'))
		.map((name) => {
			const filePath = `${fixturesFolder}/${name}`;
			return { path: filePath, text: readFileSync(`${uiRoot}${filePath}`, 'utf8') };
		});
}

function reasonsFor(text: string): readonly FixtureRefusalReason[] {
	return fixtureRefusals({ path: `${fixturesFolder}/people.ts`, text }).map(
		(refusal) => refusal.reason
	);
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

	it('passes every real fixture', () => {
		expect(realFixtures().flatMap(fixtureRefusals)).toEqual([]);
	});
});
