import { describe, expect, it } from 'vitest';
import { pageToken } from '$lib/types/brand';
import { joinPath, segmentPath } from '../path-builder';
import {
	denApiPathOf,
	denHrefOf,
	denPathFromPathname,
	denPathFromSegments,
	denQueryFromSearch,
	PLAIN_QUERY
} from './den-path';

/** A did:plc key, as an Account's or a Character's segment. */
const DID = 'did:plc:mockalicestudioaaaaaaaa';

/** A commission's key. */
const COMMISSION = '01a0ef9c-5b2e-7c41-9d3a-6f1e2b7c8d90';

/** `count` copies of a plain segment. */
function pieces(count: number): string[] {
	return Array.from({ length: count }, () => 'a');
}

describe('denPathFromPathname: what a browser address asks for', () => {
	it('reads /den as the root', () => {
		expect(denPathFromPathname('/den')).toEqual([]);
	});

	it('splits the rest on / and decodes each piece once', () => {
		expect(denPathFromPathname(`/den/accounts/${DID}`)).toEqual(['accounts', DID]);
		expect(denPathFromPathname('/den/accounts/did%3Aplc%3Aabc')).toEqual([
			'accounts',
			'did:plc:abc'
		]);
	});

	it('decodes exactly once: a double-encoded dot segment stays a literal, harmless segment', () => {
		// %252e%252e decodes once to the literal text "%2e%2e", which is not a
		// dot segment; a second decode (which would make it "..") never happens.
		expect(denPathFromPathname('/den/%252e%252e')).toEqual(['%2e%2e']);
	});

	const refused: readonly [string, string][] = [
		['an address outside /den', '/accounts'],
		['a prefix look-alike', '/dent/x'],
		['a trailing slash (an empty piece)', '/den/'],
		['a doubled slash (an empty piece)', '/den/accounts//x'],
		['a leading doubled slash', '/den//evil.example'],
		['a raw dot segment', '/den/./accounts'],
		['a raw dot-dot segment', '/den/../accounts'],
		['an encoded dot-dot (lowercase)', '/den/%2e%2e/accounts'],
		['an encoded dot-dot (uppercase)', '/den/%2E%2E/accounts'],
		['a half-encoded dot-dot', '/den/.%2e/accounts'],
		['the other half-encoded dot-dot', '/den/%2e./accounts'],
		['an encoded single dot', '/den/%2e'],
		['an encoded slash inside a piece', '/den/a%2Fb'],
		['an encoded backslash', '/den/a%5Cb'],
		['a raw backslash', '/den/a\\b'],
		['an encoded NUL', '/den/a%00b'],
		['an encoded newline', '/den/a%0Ab'],
		['an encoded space', '/den/a%20b'],
		['an encoded question mark', '/den/a%3Fb'],
		['an encoded hash', '/den/a%23b'],
		['a malformed escape', '/den/%zz'],
		['a truncated escape', '/den/%E0%A4%A'],
		['a fullwidth dot-dot look-alike', '/den/%EF%BC%8E%EF%BC%8E'],
		['a two-dot leader look-alike', '/den/%E2%80%A5'],
		['a fullwidth slash look-alike', '/den/a%EF%BC%8Fb'],
		['a right-to-left override', '/den/a%E2%80%AEb'],
		['non-ASCII letters', '/den/%C3%A9t%C3%A9']
	];

	it.each(refused)('refuses %s', (_, pathname) => {
		expect(denPathFromPathname(pathname)).toBeUndefined();
	});

	it('takes 16 pieces and refuses 17', () => {
		expect(denPathFromPathname(`/den/${pieces(16).join('/')}`)).toHaveLength(16);
		expect(denPathFromPathname(`/den/${pieces(17).join('/')}`)).toBeUndefined();
	});

	it('takes a 2048-byte piece and refuses 2049', () => {
		expect(denPathFromPathname(`/den/${'a'.repeat(2048)}`)).toEqual(['a'.repeat(2048)]);
		expect(denPathFromPathname(`/den/${'a'.repeat(2049)}`)).toBeUndefined();
	});
});

describe('denQueryFromSearch: only allowlisted parameters survive', () => {
	it('reads includeDeleted=true and a well-formed page token', () => {
		const query = denQueryFromSearch(new URLSearchParams('includeDeleted=true&pageToken=p1.25'));
		expect(query).toEqual({ includeDeleted: true, pageToken: 'p1.25' });
	});

	it('drops unknown parameters, other spellings and other values', () => {
		const search = new URLSearchParams(
			'include-deleted=true&include_deleted=true&includeDeleted=TRUE&evil=1&page_token=x'
		);
		expect(denQueryFromSearch(search)).toEqual(PLAIN_QUERY);
	});

	it('drops a repeated flag or a repeated token rather than guess', () => {
		const search = new URLSearchParams(
			'includeDeleted=true&includeDeleted=true&pageToken=a&pageToken=b'
		);
		expect(denQueryFromSearch(search)).toEqual(PLAIN_QUERY);
	});

	it('drops a page token with spaces, control characters, or over 512 characters', () => {
		const spaced = denQueryFromSearch(new URLSearchParams({ pageToken: 'a b' }));
		const control = denQueryFromSearch(new URLSearchParams({ pageToken: 'a\u0000b' }));
		const long = denQueryFromSearch(new URLSearchParams({ pageToken: 'a'.repeat(513) }));
		expect([spaced.pageToken, control.pageToken, long.pageToken]).toEqual([
			undefined,
			undefined,
			undefined
		]);
	});
});

describe('denHrefOf: the only way a Den link is built', () => {
	it('builds an absolute, encoded link with only allowlisted parameters, in a fixed order', () => {
		const path = denPathFromSegments(['commissions', COMMISSION]);
		const token = pageToken('p1.25');
		if (path === undefined || token === undefined) throw new Error('fixture refused');

		expect(denHrefOf(path, PLAIN_QUERY)).toBe(`/den/commissions/${COMMISSION}`);
		expect(denHrefOf(path, { includeDeleted: true, pageToken: token })).toBe(
			`/den/commissions/${COMMISSION}?includeDeleted=true&pageToken=p1.25`
		);
	});

	it('keeps a DID readable and escapes only %', () => {
		const path = denPathFromSegments(['accounts', 'did:web:a%3Ab']);
		if (path === undefined) throw new Error('fixture refused');

		expect(denHrefOf(path, PLAIN_QUERY)).toBe('/den/accounts/did:web:a%253Ab');
	});

	it('round-trips: a built link parses back to the same path', () => {
		const path = denPathFromSegments(['accounts', 'did:web:a%3Ab', '%2e%2e']);
		if (path === undefined) throw new Error('fixture refused');
		const href = denHrefOf(path, PLAIN_QUERY);
		if (href === undefined) throw new Error('link refused');

		expect(denPathFromPathname(new URL(href, 'http://x.invalid').pathname)).toEqual(path);
	});

	it('builds the root as /den', () => {
		expect(denHrefOf([], PLAIN_QUERY)).toBe('/den');
		expect(denHrefOf([], { includeDeleted: true, pageToken: undefined })).toBe(
			'/den?includeDeleted=true'
		);
	});
});

describe('denPathFromSegments: pieces the backend sent are checked too', () => {
	it.each([
		['an empty piece', ['']],
		['a dot', ['.']],
		['a dot-dot', ['..']],
		['a slash', ['a/b']],
		['a backslash', ['a\\b']],
		['a query', ['a?b']],
		['a fragment', ['a#b']],
		['non-ASCII', ['été']],
		['17 pieces', pieces(17)]
	])('refuses %s', (_, segments) => {
		expect(denPathFromSegments(segments)).toBeUndefined();
	});
});

describe('denApiPathOf: the API request path', () => {
	it('reads the root at /den and a node at /den/<segments>', () => {
		const path = denPathFromSegments(['accounts', DID]);
		if (path === undefined) throw new Error('fixture refused');

		expect(denApiPathOf([], PLAIN_QUERY)).toBe('/den');
		expect(denApiPathOf(path, { includeDeleted: true, pageToken: undefined })).toBe(
			`/den/accounts/${DID}?includeDeleted=true`
		);
	});
});

describe('joinPath: the parse-equality check', () => {
	it('builds what the URL parser reads back unchanged', () => {
		const path = segmentPath(['accounts', 'abc']);
		if (path === undefined) throw new Error('fixture refused');

		expect(joinPath('/accounts', path)).toBe('/accounts/accounts/abc');
	});

	it('refuses a prefix the parser would rewrite', () => {
		const path = segmentPath(['a']);
		if (path === undefined) throw new Error('fixture refused');

		expect(joinPath('/x/../den', path)).toBeUndefined();
		expect(joinPath('/den?x', path)).toBeUndefined();
	});
});

describe('the path-handling review’s further cases', () => {
	it.each([
		['the review’s own traversal, verbatim', '/den/..%2Faccounts'],
		['lowercase escaped slashes', '/den/..%2f..%2fme'],
		['an uppercase half-encoded dot-dot', '/den/%2E./x'],
		['dot-dot-slash in one piece', '/den/%2e%2e%2f'],
		['an overlong UTF-8 dot pair', '/den/%C0%AE%C0%AE'],
		['an overlong UTF-8 slash', '/den/a%C0%AFb'],
		['a lone percent', '/den/%'],
		['the servlet-style dot-dot-semicolon', '/den/..;/accounts'],
		['a semicolon', '/den/a;b'],
		['a tab', '/den/a%09b'],
		['a carriage return', '/den/a%0Db'],
		['DEL', '/den/a%7Fb'],
		['a plus', '/den/a+b'],
		['an at sign', '/den/a@b'],
		['an apostrophe', "/den/a'b"],
		['a division slash', '/den/a%E2%88%95b'],
		['a one-dot leader pair', '/den/%E2%80%A4%E2%80%A4'],
		['a small full stop pair', '/den/%EF%B9%92%EF%B9%92'],
		['an escaped slash after the prefix', '/den%2Fx'],
		['an uppercase prefix', '/DEN/x']
	])('refuses %s', (_, pathname) => {
		expect(denPathFromPathname(pathname)).toBeUndefined();
	});

	it('accepts "..." as a literal segment: only . and .. are special', () => {
		expect(denPathFromPathname('/den/...')).toEqual(['...']);
	});

	it.each([
		['%2e%2e', '/den/accounts/%252e%252e'],
		['.%2e', '/den/accounts/.%252e'],
		['%2F', '/den/accounts/%252F'],
		['%', '/den/accounts/%25']
	])(
		'keeps the literal %s escaped on the API path, so fetch can’t resolve it',
		(literal, expected) => {
			const path = denPathFromSegments(['accounts', literal]);
			if (path === undefined) throw new Error('fixture refused');
			const apiPath = denApiPathOf(path, PLAIN_QUERY);

			expect(apiPath).toBe(expected);
			expect(new URL(`/api/v1${apiPath ?? ''}`, 'http://x.invalid').pathname).toBe(
				`/api/v1${expected}`
			);
		}
	);

	it('carries a page token holding & = # + % encoded, never setting another parameter', () => {
		const token = pageToken('a&includeDeleted=true#x+%25=');
		const path = denPathFromSegments(['commissions']);
		if (token === undefined || path === undefined) throw new Error('fixture refused');
		const href = denHrefOf(path, { includeDeleted: false, pageToken: token });
		const parsed = new URL(href ?? '', 'http://x.invalid');

		expect(parsed.searchParams.get('includeDeleted')).toBeNull();
		expect(parsed.searchParams.get('pageToken')).toBe('a&includeDeleted=true#x+%25=');
		expect(parsed.hash).toBe('');
	});
});
