import { describe, expect, it } from 'vitest';
import { Either, Schema } from 'effect';
import { claimHandleField, handleField } from './handle';

/**
 * Grammar coverage at the field's own altitude (the `delete-account.spec.ts`
 * precedent) — route specs exercise the wiring, THIS file owns the table:
 * the atproto reference shape and length caps (base tier, sign-in), and the
 * DD 26050561 punycode rejection in every label position (claim tier only —
 * Engineer ruling 2026-08-05: auth-time accepts what claim-time rejects).
 */
const decode = Schema.decodeUnknownEither(handleField('Handle required'));
const decodeClaim = Schema.decodeUnknownEither(claimHandleField('Handle required'));

const LABEL_63 = 'a'.repeat(63);
/** 3×(63+dot) + 61 = 253 — exactly the atproto/backend cap. */
const MAX_LEN_HANDLE = `${LABEL_63}.${LABEL_63}.${LABEL_63}.${'a'.repeat(61)}`;
/** One char past the cap, every label individually legal. */
const OVER_LEN_HANDLE = `${LABEL_63}.${LABEL_63}.${LABEL_63}.${'a'.repeat(62)}`;

describe('handleField grammar', () => {
	it.each([
		['plain', 'alice.bsky.social'],
		['mixed case', 'Alice.Bsky.Social'],
		['all caps (spec: case-insensitive)', 'ALICE.BSKY.SOCIAL'],
		['digit-final label (spec reference regex)', 'a.b2'],
		['inner hyphen', 'foo-bar.example'],
		['63-char label (max)', `${LABEL_63}.com`],
		['253 chars total (max)', MAX_LEN_HANDLE]
	])('accepts %s', (_name, handle) => {
		const result = decode(handle);
		expect(Either.isRight(result)).toBe(true);
	});

	it('trims before validating', () => {
		const result = decode('  padded.example  ');
		expect(Either.isRight(result) && result.right).toBe('padded.example');
	});

	it.each([
		['empty', '', 'Handle required'],
		['whitespace-only (trims to empty)', '   ', 'Handle required'],
		['no dot', 'no-dot', 'This handle is not valid'],
		['leading hyphen in label', '-bad.example', 'This handle is not valid'],
		['trailing hyphen in label', 'bad-.example', 'This handle is not valid'],
		['trailing dot', 'trailing.dot.', 'This handle is not valid'],
		['64-char label', `${'a'.repeat(64)}.com`, 'This handle is not valid'],
		['254 chars total', OVER_LEN_HANDLE, 'This handle is too long']
	])('rejects %s with the authored message', (_name, handle, expected) => {
		const result = decode(handle);
		expect(Either.isLeft(result)).toBe(true);
		if (Either.isLeft(result)) expect(String(result.left)).toContain(expected);
	});

	it('accepts a punycode handle at the base (sign-in) tier — an existing atproto identity', () => {
		expect(Either.isRight(decode('xn--evil.example'))).toBe(true);
	});
});

describe('claimHandleField (the DD 26050561 tier)', () => {
	it.each([
		['leading label', 'xn--evil.example'],
		['middle label', 'good.xn--evil.com'],
		['uppercased', 'XN--evil.example']
	])('rejects punycode in the %s at claim time', (_name, handle) => {
		const result = decodeClaim(handle);
		expect(Either.isLeft(result)).toBe(true);
		if (Either.isLeft(result))
			expect(String(result.left)).toContain('Punycode (xn--) labels are not allowed');
	});

	it('accepts what the base tier accepts otherwise', () => {
		expect(Either.isRight(decodeClaim('alice.bsky.social'))).toBe(true);
	});
});
