import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

/** A place in a JSON document: object keys and array indexes, outermost first. */
export type JsonPath = readonly (string | number)[];

/** One value of the design system's token file that the committed copy writes differently. */
export interface SampleSubstitution {
	readonly path: JsonPath;
	readonly replacement: string;
}

/**
 * The only two places where the committed `tokens.json` differs from the design
 * system's file: a handle and an identifier shown as type samples, replaced by
 * made-up ones of the same shape. Paths and replacements only; the originals
 * are read from the source file at run time and are never written down.
 */
export const SAMPLE_SUBSTITUTIONS: readonly SampleSubstitution[] = [
	{ path: ['type', 'groups', 1, 'styles', 0, 'sample'], replacement: '@artist.test' },
	{
		path: ['type', 'groups', 1, 'styles', 1, 'sample'],
		replacement: 'did:plc:abcdefghijklmnopqrstuvwx'
	}
];

/**
 * The sha256 of the design system's raw `tokens.json` with the text at each
 * listed path replaced by `"<sample>"`. A new design-system revision changes
 * this constant deliberately; the unmasked file is never hashed.
 */
export const SOURCE_MASKED_SHA256 =
	'68f19fa28a0755a1e872a6e45bafb2085886d74f07074b2b176847cc1c5868e9';

/** What stands in for a sample text when the file is hashed. */
export const MASK = '<sample>';

/** The committed copy, resolved from this file. */
const COMMITTED_PATH = fileURLToPath(new URL('../../lib/styles/tokens.json', import.meta.url));

/** A JSON string literal to find in the raw text, and the literal to put in its place. */
interface Swap {
	readonly needle: string;
	readonly insert: string;
}

/** The index of the only occurrence of `needle` in `text`; undefined when there are none or several. */
function locateOnce(text: string, needle: string): number | undefined {
	const first = text.indexOf(needle);
	if (first === -1 || text.includes(needle, first + needle.length)) return undefined;
	return first;
}

/**
 * Replaces the single occurrence of each swap's needle, leaving every other
 * byte alone. Undefined when a needle is not there exactly once or two needles
 * overlap.
 */
function applySwaps(text: string, swaps: readonly Swap[]): string | undefined {
	const located = swaps.map((swap) => ({ swap, index: locateOnce(text, swap.needle) }));
	const sorted = located
		.flatMap(({ swap, index }) => (index === undefined ? [] : [{ swap, index }]))
		.toSorted((left, right) => left.index - right.index);
	if (sorted.length !== swaps.length) return undefined;
	let result = '';
	let cursor = 0;
	for (const { swap, index } of sorted) {
		if (index < cursor) return undefined;
		result += text.slice(cursor, index) + swap.insert;
		cursor = index + swap.needle.length;
	}
	return result + text.slice(cursor);
}

/**
 * The sha256 (hex) of `text` with each of the given JSON string literals,
 * which must each occur exactly once, replaced by `"<sample>"`; undefined when
 * one does not.
 */
export function maskedSha256(text: string, literals: readonly string[]): string | undefined {
	const insert = JSON.stringify(MASK);
	const masked = applySwaps(
		text,
		literals.map((needle) => ({ needle, insert }))
	);
	return masked === undefined ? undefined : createHash('sha256').update(masked).digest('hex');
}

/** The value at a path of parsed JSON, or undefined when the path leads nowhere. */
export function valueAt(root: unknown, steps: JsonPath): unknown {
	let current: unknown = root;
	for (const step of steps) {
		if (!(current instanceof Object)) return undefined;
		current = Reflect.get(current, step);
	}
	return current;
}

/** A path as a plain label, such as `type.groups[1].styles[0].sample`. */
function labelOf(steps: JsonPath): string {
	return steps
		.map((step, index) =>
			typeof step === 'number' ? `[${String(step)}]` : `${index === 0 ? '' : '.'}${step}`
		)
		.join('');
}

export type SubstituteOutcome =
	| {
			readonly kind: 'substituted';
			/** The source text with only the listed values changed. */
			readonly text: string;
			/** The sha256 (hex) of the source text with each listed value masked. */
			readonly maskedSha256: string;
	  }
	| { readonly kind: 'refused'; readonly reason: string };

/**
 * Finds the original at each listed path of the raw source, checks its JSON
 * string literal occurs exactly once in the raw text, and swaps that one
 * occurrence for the replacement's literal. Refusals name the path and never
 * quote the source.
 */
export function substituteSamples(
	raw: string,
	substitutions: readonly SampleSubstitution[] = SAMPLE_SUBSTITUTIONS
): SubstituteOutcome {
	let parsed: unknown;
	try {
		parsed = JSON.parse(raw);
	} catch {
		return { kind: 'refused', reason: 'the source is not valid JSON' };
	}
	const swaps: Swap[] = [];
	for (const { path: steps, replacement } of substitutions) {
		const original = valueAt(parsed, steps);
		if (typeof original !== 'string') {
			return { kind: 'refused', reason: `the source holds no text at ${labelOf(steps)}` };
		}
		const needle = JSON.stringify(original);
		if (locateOnce(raw, needle) === undefined) {
			return {
				kind: 'refused',
				reason: `the text at ${labelOf(steps)} does not occur exactly once in the source`
			};
		}
		swaps.push({ needle, insert: JSON.stringify(replacement) });
	}
	const text = applySwaps(raw, swaps);
	const hash = maskedSha256(
		raw,
		swaps.map((swap) => swap.needle)
	);
	if (text === undefined || hash === undefined) {
		return { kind: 'refused', reason: 'the listed values overlap in the source' };
	}
	return { kind: 'substituted', text, maskedSha256: hash };
}

/** The exit codes of the command line. */
const EXIT_DIFFERENT = 1;
const EXIT_UNREADABLE = 2;

const USAGE =
	'usage: node src/checks/tokens/copy.ts [--check] <path to the design system tokens.json>';

function readBytes(file: string): Buffer | undefined {
	try {
		return readFileSync(file);
	} catch {
		return undefined;
	}
}

/** Runs the command line; returns the exit code. */
function main(args: readonly string[]): number {
	const isCheck = args.includes('--check');
	const operands = args.filter((arg) => arg !== '--check');
	const [sourcePath] = operands;
	if (sourcePath === undefined || operands.length !== 1) {
		console.error(USAGE);
		return EXIT_UNREADABLE;
	}
	const source = readBytes(sourcePath);
	if (source === undefined) {
		console.error('cannot read the source file');
		return EXIT_UNREADABLE;
	}
	const outcome = substituteSamples(source.toString('utf8'));
	if (outcome.kind === 'refused') {
		console.error(`refused: ${outcome.reason}`);
		return EXIT_DIFFERENT;
	}
	if (outcome.maskedSha256 !== SOURCE_MASKED_SHA256) {
		console.error(
			`refused: the source differs from the design system revision this package was built from ` +
				`(masked sha256 ${outcome.maskedSha256}, expected ${SOURCE_MASKED_SHA256}); ` +
				'a new revision needs SOURCE_MASKED_SHA256 changed deliberately'
		);
		return EXIT_DIFFERENT;
	}
	if (!isCheck) {
		writeFileSync(COMMITTED_PATH, outcome.text);
		console.log(`wrote ${path.relative(process.cwd(), COMMITTED_PATH)}`);
		return 0;
	}
	const committed = readBytes(COMMITTED_PATH);
	if (committed === undefined) {
		console.error('cannot read the committed tokens.json');
		return EXIT_UNREADABLE;
	}
	if (!committed.equals(Buffer.from(outcome.text, 'utf8'))) {
		console.error('the committed tokens.json differs from the source with the two samples swapped');
		return EXIT_DIFFERENT;
	}
	console.log('the committed tokens.json matches the source');
	return 0;
}

const entry = process.argv[1];
if (entry !== undefined && path.resolve(entry) === fileURLToPath(import.meta.url)) {
	process.exitCode = main(process.argv.slice(2));
}
