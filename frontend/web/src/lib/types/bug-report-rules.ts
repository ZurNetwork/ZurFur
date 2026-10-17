/**
 * The rules a bug report's fields follow, stated once for both sides of the
 * seam: the browser form checks with them and the server checks again with
 * the same functions. Plain TypeScript, no Effect, so the client can import it.
 *
 * Every field is parsed, never merely validated: a parser returns the field's
 * brand or the reason it was refused, naming the field. The description is
 * required; every other text is optional, and an optional text that is empty
 * after trimming is "not sent", never an empty value.
 */

import type { Brand } from './brand';

/** The most code points a description may hold, after line ends are normalized and it is trimmed. */
export const DESCRIPTION_MAX_CODE_POINTS = 4000;

/** The most UTF-8 bytes the page address may hold. */
export const PAGE_MAX_BYTES = 2048;

/** The most UTF-8 bytes the page type may hold. */
export const PAGE_TYPE_MAX_BYTES = 64;

/** The most UTF-8 bytes the app version may hold. */
export const VERSION_MAX_BYTES = 64;

/** The most UTF-8 bytes the browser text may hold, its cut mark included. */
export const BROWSER_MAX_BYTES = 512;

/** The suffix that shows the browser text was cut; its bytes count inside {@link BROWSER_MAX_BYTES}. */
export const CUT_MARK = '…';

/** A field of a bug report. */
export type BugReportField = 'description' | 'page' | 'pageType' | 'version' | 'browser';

/** The kinds of page a report can come from: a closed list. */
export const PAGE_TYPES = [
	'My Den',
	'Accounts',
	'An Account',
	'Report a bug',
	'Error page',
	'Other'
] as const;

/** One of {@link PAGE_TYPES}. */
export type PageType = (typeof PAGE_TYPES)[number];

/** A description that passed {@link parseDescription}: normalized, trimmed, 1 to 4,000 code points. */
export type BugDescription = Brand<string, 'BugDescription'>;

/** A page address that passed {@link parsePage}: an app path with no query or fragment. */
export type BugPage = Brand<string, 'BugPage'>;

/** An app version that passed {@link parseVersion}. */
export type BugVersion = Brand<string, 'BugVersion'>;

/** Browser text that passed {@link parseBrowser}: trimmed, cut to fit, free of control characters. */
export type BugBrowser = Brand<string, 'BugBrowser'>;

/** How a length is measured. */
export type LengthUnit = 'codePoints' | 'bytes';

/** Why a field was refused; every reason names its field. */
export type BugReportRefusal =
	| { readonly kind: 'empty'; readonly field: 'description' }
	| {
			readonly kind: 'tooLong';
			readonly field: BugReportField;
			readonly limit: number;
			readonly unit: LengthUnit;
	  }
	| {
			readonly kind: 'refusedCharacter';
			readonly field: BugReportField;
			readonly codePoint: number;
	  }
	| { readonly kind: 'notAPath'; readonly field: 'page' }
	| { readonly kind: 'queryOrFragment'; readonly field: 'page' }
	| { readonly kind: 'unknownPageType'; readonly field: 'pageType' };

/** A required field's outcome: its parsed value, or why it was refused. */
export type FieldParse<T> =
	| { readonly kind: 'accepted'; readonly value: T }
	| { readonly kind: 'refused'; readonly refusal: BugReportRefusal };

/** An optional field's outcome: parsed, refused, or empty and so not sent. */
export type OptionalFieldParse<T> = FieldParse<T> | { readonly kind: 'notSent' };

const LINE_FEED = 0x0a;
const TAB = 0x09;

/** CR LF becomes LF, then the text is trimmed. */
export function normalizeDescription(text: string): string {
	return text.replaceAll('\r\n', '\n').trim();
}

/** The number of Unicode code points in `text`, never UTF-16 units. */
export function codePointCount(text: string): number {
	return Array.from(text).length;
}

/** The UTF-8 byte length of one code point; a lone surrogate counts as the 3 bytes of U+FFFD. */
function utf8Width(codePoint: number): number {
	if (codePoint < 0x80) return 1;
	if (codePoint < 0x800) return 2;
	if (codePoint < 0x10000) return 3;
	return 4;
}

/** The number of bytes `text` takes in UTF-8. */
export function utf8ByteLength(text: string): number {
	let bytes = 0;
	for (const character of text) bytes += utf8Width(character.codePointAt(0) ?? 0);
	return bytes;
}

/** A C0 or C1 control character (general category Cc). */
function isControl(codePoint: number): boolean {
	return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
}

/** One of the nine bidirectional embedding, override and isolate controls. */
function isBidiControl(codePoint: number): boolean {
	return (
		(codePoint >= 0x202a && codePoint <= 0x202e) || (codePoint >= 0x2066 && codePoint <= 0x2069)
	);
}

/** Whether `field` refuses `codePoint`: the description keeps LF and tab; no field keeps any other control. */
function refuses(field: BugReportField, codePoint: number): boolean {
	if (isBidiControl(codePoint)) return true;
	if (!isControl(codePoint)) return false;
	return !(field === 'description' && (codePoint === LINE_FEED || codePoint === TAB));
}

/** The first code point in `text` that `field` refuses, if any. */
export function refusedCharacter(field: BugReportField, text: string): number | undefined {
	for (const character of text) {
		const codePoint = character.codePointAt(0) ?? 0;
		if (refuses(field, codePoint)) return codePoint;
	}
	return undefined;
}

function refused<T>(refusal: BugReportRefusal): FieldParse<T> {
	return { kind: 'refused', refusal };
}

/** The refusal for the first refused character of `text` in `field`, if any. */
function characterRefusal(field: BugReportField, text: string): BugReportRefusal | undefined {
	const codePoint = refusedCharacter(field, text);
	return codePoint === undefined ? undefined : { kind: 'refusedCharacter', field, codePoint };
}

/** The description: CR LF to LF, trimmed, 1 to 4,000 code points, LF and tab its only controls. */
export function parseDescription(text: string): FieldParse<BugDescription> {
	const normalized = normalizeDescription(text);
	if (normalized.length === 0) return refused({ kind: 'empty', field: 'description' });
	const badCharacter = characterRefusal('description', normalized);
	if (badCharacter !== undefined) return refused(badCharacter);
	if (codePointCount(normalized) > DESCRIPTION_MAX_CODE_POINTS) {
		return refused({
			kind: 'tooLong',
			field: 'description',
			limit: DESCRIPTION_MAX_CODE_POINTS,
			unit: 'codePoints'
		});
	}
	return { kind: 'accepted', value: normalized as BugDescription };
}

/** The page address: trimmed, at most 2,048 bytes, starting with `/`, with no `?` or `#` and no control. */
export function parsePage(text: string): OptionalFieldParse<BugPage> {
	const trimmed = text.trim();
	if (trimmed.length === 0) return { kind: 'notSent' };
	const badCharacter = characterRefusal('page', trimmed);
	if (badCharacter !== undefined) return refused(badCharacter);
	if (utf8ByteLength(trimmed) > PAGE_MAX_BYTES) {
		return refused({ kind: 'tooLong', field: 'page', limit: PAGE_MAX_BYTES, unit: 'bytes' });
	}
	if (!trimmed.startsWith('/')) return refused({ kind: 'notAPath', field: 'page' });
	if (trimmed.includes('?') || trimmed.includes('#')) {
		return refused({ kind: 'queryOrFragment', field: 'page' });
	}
	return { kind: 'accepted', value: trimmed as BugPage };
}

/** `text` as one of {@link PAGE_TYPES}, exactly, or `undefined`. */
export function parsePageType(text: string): PageType | undefined {
	return PAGE_TYPES.find((pageType) => pageType === text);
}

/**
 * The page type of an app path, by its first piece: `/den` and below is
 * "My Den", `/accounts` "Accounts", an address below it "An Account",
 * `/report-a-bug` "Report a bug", anything else "Other". "Error page" is
 * never read from a path.
 */
export function pageTypeOf(pathname: string): PageType {
	const [section, ...rest] = pathname.split('/').filter((piece) => piece.length > 0);
	if (section === 'den') return 'My Den';
	if (section === 'accounts') return rest.length === 0 ? 'Accounts' : 'An Account';
	if (section === 'report-a-bug' && rest.length === 0) return 'Report a bug';
	return 'Other';
}

/** The page type field: trimmed, then one of {@link PAGE_TYPES} (each well inside 64 bytes). */
export function parsePageTypeField(text: string): OptionalFieldParse<PageType> {
	const trimmed = text.trim();
	if (trimmed.length === 0) return { kind: 'notSent' };
	const pageType = parsePageType(trimmed);
	if (pageType === undefined) return refused({ kind: 'unknownPageType', field: 'pageType' });
	return { kind: 'accepted', value: pageType };
}

/** The app version: trimmed, at most 64 bytes, no control. */
export function parseVersion(text: string): OptionalFieldParse<BugVersion> {
	const trimmed = text.trim();
	if (trimmed.length === 0) return { kind: 'notSent' };
	const badCharacter = characterRefusal('version', trimmed);
	if (badCharacter !== undefined) return refused(badCharacter);
	if (utf8ByteLength(trimmed) > VERSION_MAX_BYTES) {
		return refused({ kind: 'tooLong', field: 'version', limit: VERSION_MAX_BYTES, unit: 'bytes' });
	}
	return { kind: 'accepted', value: trimmed as BugVersion };
}

/**
 * `text` cut to fit 512 UTF-8 bytes: unchanged when it fits; otherwise the
 * longest whole-code-point prefix that leaves room for {@link CUT_MARK},
 * then the mark. Lone surrogates become U+FFFD first, as UTF-8 would send
 * them. Cutting a cut text changes nothing.
 */
export function cutBrowser(text: string): string {
	const wellFormed = text.toWellFormed();
	if (utf8ByteLength(wellFormed) <= BROWSER_MAX_BYTES) return wellFormed;
	const room = BROWSER_MAX_BYTES - utf8ByteLength(CUT_MARK);
	let bytes = 0;
	let kept = '';
	for (const character of wellFormed) {
		const width = utf8Width(character.codePointAt(0) ?? 0);
		if (bytes + width > room) break;
		bytes += width;
		kept += character;
	}
	return `${kept}${CUT_MARK}`;
}

/** The browser text: trimmed, cut to 512 bytes (never refused for its length), no control. */
export function parseBrowser(text: string): OptionalFieldParse<BugBrowser> {
	const trimmed = text.trim();
	if (trimmed.length === 0) return { kind: 'notSent' };
	const cut = cutBrowser(trimmed);
	const badCharacter = characterRefusal('browser', cut);
	if (badCharacter !== undefined) return refused(badCharacter);
	return { kind: 'accepted', value: cut as BugBrowser };
}
