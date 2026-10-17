import { describe, expect, it } from 'vitest';
import {
	BROWSER_MAX_BYTES,
	CUT_MARK,
	PAGE_TYPES,
	codePointCount,
	cutBrowser,
	normalizeDescription,
	parseBrowser,
	parseDescription,
	parsePage,
	parsePageType,
	parsePageTypeField,
	pageTypeOf,
	parseVersion,
	refusedCharacter,
	utf8ByteLength,
	type BugReportField
} from './bug-report-rules';

/** An emoji outside the BMP: one code point, two UTF-16 units, four UTF-8 bytes. */
const EMOJI = '😀';

const RLO = String.fromCodePoint(0x202e);
const CSI = String.fromCodePoint(0x9b);
const TAG_A = String.fromCodePoint(0xe0041);
const ZWJ = String.fromCodePoint(0x200d);

describe('the description', () => {
	it('accepts 4,000 emoji and refuses 4,001, counting code points', () => {
		const atLimit = parseDescription(EMOJI.repeat(4000));
		const overLimit = parseDescription(EMOJI.repeat(4001));
		expect(atLimit.kind).toBe('accepted');
		expect(overLimit).toEqual({
			kind: 'refused',
			refusal: { kind: 'tooLong', field: 'description', limit: 4000, unit: 'codePoints' }
		});
	});

	it('counts CR LF as one character', () => {
		const text = `${'a'.repeat(3998)}\r\nb`;
		const parsed = parseDescription(text);
		expect(parsed).toEqual({ kind: 'accepted', value: `${'a'.repeat(3998)}\nb` });
		expect(codePointCount(normalizeDescription(text))).toBe(4000);
	});

	it('refuses whitespace only as empty', () => {
		expect(parseDescription(' \r\n\t \n ')).toEqual({
			kind: 'refused',
			refusal: { kind: 'empty', field: 'description' }
		});
	});

	it('trims what it keeps', () => {
		expect(parseDescription('  it broke \r\n')).toEqual({ kind: 'accepted', value: 'it broke' });
	});

	it('keeps LF and tab inside', () => {
		expect(parseDescription('one\ttwo\nthree')).toEqual({
			kind: 'accepted',
			value: 'one\ttwo\nthree'
		});
	});

	it.each([
		['U+202E', RLO, 0x202e],
		['U+009B', CSI, 0x9b],
		['a lone CR', 'a\rb', 0x0d],
		['U+2066', String.fromCodePoint(0x2066), 0x2066]
	])('refuses %s, naming the description', (_name, character, codePoint) => {
		expect(parseDescription(`before ${character} after`)).toEqual({
			kind: 'refused',
			refusal: { kind: 'refusedCharacter', field: 'description', codePoint }
		});
	});

	it('lets a tag character and ZWJ through', () => {
		const text = `flag${TAG_A} family👩${ZWJ}👧`;
		expect(parseDescription(text)).toEqual({ kind: 'accepted', value: text });
	});
});

describe('the character rule', () => {
	const strictFields: readonly BugReportField[] = ['page', 'pageType', 'version', 'browser'];

	it.each(strictFields)('refuses LF and tab in %s', (field) => {
		expect(refusedCharacter(field, 'a\nb')).toBe(0x0a);
		expect(refusedCharacter(field, 'a\tb')).toBe(0x09);
	});

	it.each([...strictFields, 'description' as const])(
		'refuses every bidi control and C1 control in %s',
		(field) => {
			for (const codePoint of [0x202a, 0x202e, 0x2066, 0x2069, 0x7f, 0x80, 0x9f, 0x00]) {
				expect(refusedCharacter(field, `x${String.fromCodePoint(codePoint)}`)).toBe(codePoint);
			}
		}
	);

	it.each([...strictFields, 'description' as const])(
		'passes tags, ZWJ and the neighbours of the bidi controls in %s',
		(field) => {
			const neighbours = String.fromCodePoint(0x2029, 0x202f, 0x2065, 0x206a, 0xa0);
			expect(refusedCharacter(field, `${TAG_A}${ZWJ}${neighbours}`)).toBe(undefined);
		}
	);
});

describe('the page', () => {
	it('accepts an app path of 2,048 bytes and refuses 2,049', () => {
		const atLimit = `/${'a'.repeat(2047)}`;
		expect(parsePage(atLimit)).toEqual({ kind: 'accepted', value: atLimit });
		expect(parsePage(`${atLimit}a`)).toEqual({
			kind: 'refused',
			refusal: { kind: 'tooLong', field: 'page', limit: 2048, unit: 'bytes' }
		});
	});

	it('counts bytes, not characters', () => {
		const multiByte = `/${'é'.repeat(1024)}`;
		expect(utf8ByteLength(multiByte)).toBe(2049);
		expect(parsePage(multiByte).kind).toBe('refused');
	});

	it.each([
		['a query', '/den?x=1', 'queryOrFragment'],
		['a fragment', '/den#top', 'queryOrFragment'],
		['no leading slash', 'den/accounts', 'notAPath']
	])('refuses %s', (_name, text, kind) => {
		expect(parsePage(text)).toEqual({ kind: 'refused', refusal: { kind, field: 'page' } });
	});

	it.each([
		['U+202E', RLO, 0x202e],
		['U+009B', CSI, 0x9b],
		['LF', '\n', 0x0a],
		['tab', '\t', 0x09]
	])('refuses %s, naming the page', (_name, character, codePoint) => {
		expect(parsePage(`/den${character}x`)).toEqual({
			kind: 'refused',
			refusal: { kind: 'refusedCharacter', field: 'page', codePoint }
		});
	});

	it('is not sent when empty after trimming', () => {
		expect(parsePage('  ')).toEqual({ kind: 'notSent' });
	});
});

describe('the page type', () => {
	it.each(PAGE_TYPES)('accepts %s', (pageType) => {
		expect(parsePageType(pageType)).toBe(pageType);
		expect(parsePageTypeField(` ${pageType} `)).toEqual({ kind: 'accepted', value: pageType });
	});

	it.each(['my den', 'Den', 'Other ', 'Settings', ''])('refuses %j', (text) => {
		expect(parsePageType(text)).toBe(undefined);
	});

	it('refuses an unknown page type, naming the field, and does not send an empty one', () => {
		expect(parsePageTypeField('Settings')).toEqual({
			kind: 'refused',
			refusal: { kind: 'unknownPageType', field: 'pageType' }
		});
		expect(parsePageTypeField(' ')).toEqual({ kind: 'notSent' });
	});
});

describe('the version', () => {
	it('accepts 64 bytes and refuses 65', () => {
		expect(parseVersion('1'.repeat(64)).kind).toBe('accepted');
		expect(parseVersion('1'.repeat(65))).toEqual({
			kind: 'refused',
			refusal: { kind: 'tooLong', field: 'version', limit: 64, unit: 'bytes' }
		});
	});

	it('refuses a control character and does not send an empty one', () => {
		expect(parseVersion('0.1\n')).toEqual({ kind: 'accepted', value: '0.1' });
		expect(parseVersion('0.\u00851').kind).toBe('refused');
		expect(parseVersion('')).toEqual({ kind: 'notSent' });
	});
});

describe('the browser cut', () => {
	const markBytes = utf8ByteLength(CUT_MARK);

	it('returns text of 512 bytes or fewer unchanged', () => {
		const exact = 'a'.repeat(BROWSER_MAX_BYTES);
		expect(cutBrowser(exact)).toBe(exact);
		expect(cutBrowser('Mozilla/5.0')).toBe('Mozilla/5.0');
		expect(cutBrowser('')).toBe('');
	});

	it('fits the mark inside 512 bytes', () => {
		const cut = cutBrowser('a'.repeat(BROWSER_MAX_BYTES + 1));
		expect(utf8ByteLength(cut)).toBeLessThanOrEqual(BROWSER_MAX_BYTES);
		expect(cut).toBe(`${'a'.repeat(BROWSER_MAX_BYTES - markBytes)}${CUT_MARK}`);
	});

	// A 4-byte character placed so it straddles the room left for the mark, at each of its offsets.
	it.each([0, 1, 2, 3, 4])(
		'cuts at a whole code point when a 4-byte character starts %i bytes before the room ends',
		(before) => {
			const room = BROWSER_MAX_BYTES - markBytes;
			const lead = 'a'.repeat(room - before);
			const input = `${lead}${EMOJI}${'b'.repeat(40)}`;
			const cut = cutBrowser(input);
			const expectedKept = before >= 4 ? `${lead}${EMOJI}` : lead;

			expect(utf8ByteLength(cut)).toBeLessThanOrEqual(BROWSER_MAX_BYTES);
			expect(cut.isWellFormed()).toBe(true);
			expect(cut).toBe(`${expectedKept}${CUT_MARK}`);
			expect(cutBrowser(cut)).toBe(cut);
		}
	);

	it('is idempotent on long multi-byte text', () => {
		const input = `${EMOJI}é${'ж'.repeat(300)}${EMOJI.repeat(50)}`;
		const cut = cutBrowser(input);
		expect(utf8ByteLength(cut)).toBeLessThanOrEqual(BROWSER_MAX_BYTES);
		expect(cut.isWellFormed()).toBe(true);
		expect(cutBrowser(cut)).toBe(cut);
		expect(input.startsWith(cut.slice(0, -CUT_MARK.length))).toBe(true);
	});

	it('turns a lone surrogate into U+FFFD so the result is well formed', () => {
		const loneHighSurrogate = String.fromCharCode(0xd83d);
		const cut = cutBrowser(`a${loneHighSurrogate}b`);
		expect(cut).toBe(`a${String.fromCodePoint(0xfffd)}b`);
		expect(cut.isWellFormed()).toBe(true);
	});
});

describe('the browser field', () => {
	it('is cut, never refused for its length', () => {
		const parsed = parseBrowser(`  ${'x'.repeat(5000)}  `);
		expect(parsed.kind).toBe('accepted');
		if (parsed.kind !== 'accepted') return;
		expect(utf8ByteLength(parsed.value)).toBeLessThanOrEqual(BROWSER_MAX_BYTES);
		expect(parsed.value.endsWith(CUT_MARK)).toBe(true);
	});

	it.each([
		['U+202E', RLO, 0x202e],
		['U+009B', CSI, 0x9b],
		['LF', '\n', 0x0a],
		['tab', '\t', 0x09]
	])('refuses %s, naming the browser', (_name, character, codePoint) => {
		expect(parseBrowser(`Mozilla${character}5.0`)).toEqual({
			kind: 'refused',
			refusal: { kind: 'refusedCharacter', field: 'browser', codePoint }
		});
	});

	it('is not sent when empty after trimming', () => {
		expect(parseBrowser(' \t ')).toEqual({ kind: 'notSent' });
	});
});

describe('the page type of a path', () => {
	it.each([
		['/den', 'My Den'],
		['/den/', 'My Den'],
		['/den/commissions/x', 'My Den'],
		['/accounts', 'Accounts'],
		['/accounts/', 'Accounts'],
		['/accounts/0192f0c4-aaaa', 'An Account'],
		['/accounts/0192f0c4-aaaa/more', 'An Account'],
		['/report-a-bug', 'Report a bug'],
		['/report-a-bug/x', 'Other'],
		['/', 'Other'],
		['/login', 'Other'],
		['/denx', 'Other'],
		['/accountsx', 'Other'],
		['', 'Other']
	] as const)('reads %j as %j', (pathname, expected) => {
		expect(pageTypeOf(pathname)).toBe(expected);
	});

	it('never answers the error page', () => {
		expect(pageTypeOf('/error')).toBe('Other');
	});
});
