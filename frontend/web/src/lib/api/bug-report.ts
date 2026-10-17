/**
 * What crosses the seam for a bug report: the form's field names, what the
 * report page's load offers, and what its action answers. Plain data; the
 * dialog and the report page both post exactly {@link BUG_REPORT_FIELDS}.
 */

import { bugReference, type BugReference } from '$lib/types/brand';
import type { BugBrowser, BugPage, PageType } from '$lib/types/bug-report-rules';

/**
 * Every field a bug report posts. An item is sent only when its tick box
 * (`send…`) posts {@link TICKED}; an empty value is never sent. The version
 * has a tick box and no value: the server stamps its own build.
 */
export const BUG_REPORT_FIELDS = {
	/** What happened, in the reporter's words. */
	description: 'description',
	sendPage: 'sendPage',
	/** An address on this site, or a path: the server keeps only its path (see {@link ARCHIVED_SUFFIX}). */
	page: 'page',
	sendPageType: 'sendPageType',
	/** One of the closed page types. */
	pageType: 'pageType',
	sendWebVersion: 'sendWebVersion',
	sendBrowser: 'sendBrowser',
	/** The browser's user agent; the server cuts it to fit. */
	browser: 'browser',
	sendWindow: 'sendWindow',
	/** Whole numbers, 1 to 100000. */
	windowWidth: 'windowWidth',
	windowHeight: 'windowHeight',
	sendDisplay: 'sendDisplay',
	/** Each {@link DISPLAY_MATCHED} or {@link DISPLAY_UNMATCHED}. */
	reducedMotion: 'reducedMotion',
	moreContrast: 'moreContrast',
	forcedColors: 'forcedColors',
	sendErrorStatus: 'sendErrorStatus',
	/** The error page's HTTP status, 400 to 599. */
	errorStatus: 'errorStatus'
} as const;

/** One of {@link BUG_REPORT_FIELDS}' names. */
export type BugReportFieldName = (typeof BUG_REPORT_FIELDS)[keyof typeof BUG_REPORT_FIELDS];

/** What a ticked box posts (a checkbox's default value). */
export const TICKED = 'on';

/** A display preference that matched. */
export const DISPLAY_MATCHED = 'true';

/** A display preference that didn't match. */
export const DISPLAY_UNMATCHED = 'false';

/**
 * The words a page shown with archived and deactivated items carries: the
 * server adds them to the page when its address asks for `includeDeleted=true`,
 * and keeps them when a posted page ends with them.
 */
export const ARCHIVED_SUFFIX = ' (showing archived)';

/** The items whose text the reporter can see and change. */
export const BUG_REPORT_TEXT_ITEMS = ['description', 'page', 'browser'] as const;

/** An item whose text can hold a refused character. */
export type BugReportTextItem = (typeof BUG_REPORT_TEXT_ITEMS)[number];

/** Every item of a report. */
export const BUG_REPORT_ITEMS = [
	'description',
	'page',
	'pageType',
	'webVersion',
	'browser',
	'window',
	'display',
	'errorStatus'
] as const;

/** One item of a report. */
export type BugReportItem = (typeof BUG_REPORT_ITEMS)[number];

/** Why the form was refused, naming the item where there is one to name. */
export type BugReportRefusalKind =
	/** The description is empty. */
	| { readonly kind: 'descriptionEmpty' }
	/** The description is over its limit. */
	| { readonly kind: 'descriptionTooLong' }
	/** A text holds a control or bidirectional character. */
	| { readonly kind: 'hiddenCharacter'; readonly item: BugReportTextItem }
	/** "Which page?" holds neither an address on this site nor a path. */
	| { readonly kind: 'pageAddress' }
	/** Any other item that doesn't fit its rule (one the page filled in itself). */
	| { readonly kind: 'itemRefused'; readonly item: BugReportItem };

/** The reporter's own text, carried back on every failure so a page without script keeps it. */
export interface BugReportEcho {
	/** The description as posted. */
	readonly description: string;
	/** The page as posted ("Which page?"), empty when none was. */
	readonly page: string;
}

/** Every way filing a report fails, each with the reporter's text. */
export type BugReportFailure = BugReportEcho &
	(
		| BugReportRefusalKind
		/** The hourly limit is reached. */
		| { readonly kind: 'limit' }
		/** The API failed, didn't answer in time, or refused the report. */
		| { readonly kind: 'couldNotSend' }
	);

/** The report went in. */
export interface BugReportSent {
	readonly kind: 'sent';
	readonly reference: BugReference;
}

/** What the report action answers: sent, or a failure. */
export type BugReportActionData = BugReportSent | BugReportFailure;

/** The page item the report page offers: the page the reporter came from, or a question. */
export type BugReportPageOffer =
	/** The page the reporter came from, read from this site's own address. */
	| { readonly kind: 'known'; readonly page: BugPage }
	/** No page of this site came before: ask "Which page?". */
	| { readonly kind: 'ask' };

/**
 * What kind of stack the web runs on, for the form's line about keeping
 * reports: the dev profile's test stack, mock mode, or neither.
 */
export type TestStack = 'devProfile' | 'mock' | 'none';

/** What the report page's load offers to send. */
export interface BugReportOffer {
	readonly page: BugReportPageOffer;
	/** The page type of the page the reporter came from, when known. */
	readonly pageType: PageType | undefined;
	/** The browser's user agent, cut to fit, when it sent one that fits the rule. */
	readonly browser: BugBrowser | undefined;
}

/** `value[key]`, or `undefined` when `value` isn't an object: reading untyped data one field at a time. */
function field(value: unknown, key: string): unknown {
	return value instanceof Object ? Reflect.get(value, key) : undefined;
}

function isTextItem(value: unknown): value is BugReportTextItem {
	return BUG_REPORT_TEXT_ITEMS.some((item) => item === value);
}

function isItem(value: unknown): value is BugReportItem {
	return BUG_REPORT_ITEMS.some((item) => item === value);
}

/** Whether `value` carries the reporter's text back. */
function hasEcho(value: unknown): boolean {
	return (
		typeof field(value, 'description') === 'string' && typeof field(value, 'page') === 'string'
	);
}

/** Whether `value` is a well-formed {@link BugReportActionData}, checked field by field. */
export function isBugReportActionData(value: unknown): value is BugReportActionData {
	const kind = field(value, 'kind');
	if (typeof kind !== 'string') return false;
	switch (kind) {
		case 'sent': {
			const reference = field(value, 'reference');
			return typeof reference === 'string' && bugReference(reference) !== undefined;
		}
		case 'descriptionEmpty':
		case 'descriptionTooLong':
		case 'pageAddress':
		case 'limit':
		case 'couldNotSend':
			return hasEcho(value);
		case 'hiddenCharacter':
			return hasEcho(value) && isTextItem(field(value, 'item'));
		case 'itemRefused':
			return hasEcho(value) && isItem(field(value, 'item'));
		default:
			return false;
	}
}
