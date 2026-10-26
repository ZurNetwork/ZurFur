import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { maskedSha256, SAMPLE_SUBSTITUTIONS, SOURCE_MASKED_SHA256, valueAt } from './copy';

const committed = readFileSync(new URL('../../lib/styles/tokens.json', import.meta.url), 'utf8');
const parsed: unknown = JSON.parse(committed);
const replacementLiterals = SAMPLE_SUBSTITUTIONS.map(({ replacement }) =>
	JSON.stringify(replacement)
);

describe('the committed tokens.json', () => {
	it('lists exactly the two substituted samples', () => {
		expect(SAMPLE_SUBSTITUTIONS.map(({ path }) => path.join('/'))).toEqual([
			'type/groups/1/styles/0/sample',
			'type/groups/1/styles/1/sample'
		]);
	});

	it.each(SAMPLE_SUBSTITUTIONS.map((substitution) => [substitution.path.join('.'), substitution]))(
		'holds the replacement at %s',
		(_label, substitution) => {
			expect(valueAt(parsed, substitution.path)).toBe(substitution.replacement);
		}
	);

	it.each(replacementLiterals.map((literal) => [literal]))(
		'holds %s exactly once in its raw text',
		(literal) => {
			expect(committed.split(literal)).toHaveLength(2);
		}
	);

	it('shows a reserved .test handle and a made-up did:plc of the right shape', () => {
		const [handle, did] = SAMPLE_SUBSTITUTIONS.map(({ replacement }) => replacement);
		expect(handle).toMatch(/^@[a-z]+\.test$/);
		expect(did).toMatch(/^did:plc:[a-z2-7]{24}$/);
	});

	it('hashes, masked at the listed paths, to the pinned design system revision', () => {
		expect(maskedSha256(committed, replacementLiterals)).toBe(SOURCE_MASKED_SHA256);
	});

	it('fails the hash when any byte outside the listed paths changes', () => {
		const edited = committed.replace('"#151417"', '"#151418"');
		expect(edited).not.toBe(committed);
		expect(maskedSha256(edited, replacementLiterals)).not.toBe(SOURCE_MASKED_SHA256);
	});

	it('fails the hash when a token is added', () => {
		const edited = committed.replace('"version": 1,', '"version": 1, "extra": 1,');
		expect(edited).not.toBe(committed);
		expect(maskedSha256(edited, replacementLiterals)).not.toBe(SOURCE_MASKED_SHA256);
	});

	it('does not hash a file that lacks a listed value', () => {
		expect(
			maskedSha256(committed.replace(replacementLiterals[0] ?? '', '"x"'), replacementLiterals)
		).toBeUndefined();
	});
});
