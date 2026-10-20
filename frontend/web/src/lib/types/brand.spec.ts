import { describe, expect, it } from 'vitest';
import {
	accountId,
	bugReference,
	did,
	firstPartyOrigin,
	handle,
	handleFromTrusted,
	pathSegment,
	pageToken
} from './brand';
import { HANDLE_MAX_LEN } from './handle-format';

describe('handle (the validated mint)', () => {
	it('mints a well-shaped handle, trimmed', () => {
		expect(handle('  alice.zurfur.app  ')).toBe('alice.zurfur.app');
	});

	it('rejects empty and whitespace-only input', () => {
		expect(handle('')).toBeUndefined();
		expect(handle('   ')).toBeUndefined();
	});

	it('rejects input over the atproto length cap', () => {
		const overlong = `${'a'.repeat(HANDLE_MAX_LEN)}.com`;
		expect(handle(overlong)).toBeUndefined();
	});

	it('rejects a shape that is not an atproto handle (no dot)', () => {
		expect(handle('no-dot')).toBeUndefined();
	});

	it('rejects punycode (xn--) labels — claim-time rule set (DD 26050561)', () => {
		expect(handle('xn--sneaky.example')).toBeUndefined();
	});
});

describe('the trusted nominal casts', () => {
	it('pass their input through unchanged (brand only, no runtime work)', () => {
		expect(accountId('0198c5f2-aaaa-7bbb-8ccc-ddddeeee0001')).toBe(
			'0198c5f2-aaaa-7bbb-8ccc-ddddeeee0001'
		);
		expect(did('did:plc:alice')).toBe('did:plc:alice');
		expect(handleFromTrusted('alice.zurfur.app')).toBe('alice.zurfur.app');
	});
});

describe('pathSegment (validated mint)', () => {
	it.each([
		'accounts',
		'did:plc:abc234',
		'01a0ef9c-5b2e-7c41-9d3a-6f1e2b7c8d90',
		'a.b_c~d%2e',
		'...'
	])('accepts %s', (piece) => {
		expect(pathSegment(piece)).toBe(piece);
	});

	it.each(['', '.', '..', 'a/b', 'a\\b', 'a b', 'a?b', 'a#b', 'été', '‥', 'a'.repeat(2049)])(
		'refuses %j',
		(piece) => {
			expect(pathSegment(piece)).toBeUndefined();
		}
	);
});

describe('pageToken (validated mint)', () => {
	it('accepts visible ASCII up to 512 characters', () => {
		expect(pageToken('p1.25')).toBe('p1.25');
		expect(pageToken('a'.repeat(512))).toHaveLength(512);
	});

	it.each(['', 'a b', 'a\u0000b', 'é', 'a'.repeat(513)])('refuses %j', (raw) => {
		expect(pageToken(raw)).toBeUndefined();
	});
});

describe('firstPartyOrigin (validated mint)', () => {
	it.each(['http://127.0.0.1:8080', 'http://[::1]:8080', 'https://zurfur.app'])(
		'accepts %s as itself',
		(raw) => {
			expect(firstPartyOrigin(raw)).toBe(raw);
		}
	);

	it('normalises the scheme case and drops the bare trailing slash', () => {
		expect(firstPartyOrigin('HTTP://127.0.0.1:8080/')).toBe('http://127.0.0.1:8080');
	});

	it.each([
		'http://127.0.0.1:8080/app',
		'http://127.0.0.1:8080?x=1',
		'http://127.0.0.1:8080?',
		'http://127.0.0.1:8080/?',
		'http://127.0.0.1:8080#top',
		'http://127.0.0.1:8080#',
		'http://user:pass@127.0.0.1:8080',
		'http://user@127.0.0.1:8080',
		'ftp://127.0.0.1:8080',
		'file:///etc/passwd',
		'ws://127.0.0.1:8080',
		'wss://zurfur.app',
		'not a url',
		'',
		'127.0.0.1:8080'
	])('refuses %j without throwing', (raw) => {
		expect(firstPartyOrigin(raw)).toBeUndefined();
	});
});

describe('bugReference', () => {
	it.each(['3f9a1c2e', 'A', 'x'.repeat(64)])('accepts %j as itself', (raw) => {
		expect(bugReference(raw)).toBe(raw);
	});

	it.each([
		'',
		'x'.repeat(65),
		'has space',
		'tab\there',
		'émoji',
		`${String.fromCodePoint(0x202e)}3f9a`
	])('refuses %j', (raw) => {
		expect(bugReference(raw)).toBeUndefined();
	});
});
