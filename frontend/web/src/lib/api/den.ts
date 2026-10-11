/**
 * The web's own shape for My Den: plain data the den page's load hands the
 * pane and the tree. The server builds every link in it (`DenHref`); no path
 * segment ever reaches a component. Unknown values from the backend arrive
 * here already folded to `'unknown'`, and an unknown access already as a card.
 */

import type { Problem } from './problem';
import type { Trail } from './trail';
import type { DenHref, DenName, DenType } from '$lib/types/brand';

/** A node's own visibility level, as only someone who can open it sees it. */
export type DenLevel = 'private' | 'listed' | 'public' | 'unknown';

/** What a node is. A mount shows its target's kind. */
export type DenKind = 'directory' | 'file' | 'symlink' | 'unknown';

/** Why a soft-deleted node is marked: an archived commission or a deactivated Account. */
export type DenRemoved = 'archived' | 'deactivated' | 'unknown';

/** An entry the viewer can open. */
export interface OpenEntry {
	readonly view: 'open';
	readonly href: DenHref;
	readonly name: DenName;
	readonly type: DenType;
	readonly kind: DenKind;
	readonly mounted: boolean;
	readonly ownLevel: DenLevel;
	readonly contentNotShown: boolean;
	readonly removed: DenRemoved | undefined;
}

/** An entry the viewer may know exists but can't open: no link, no kind, no level. */
export interface CardEntry {
	readonly view: 'card';
	readonly name: DenName;
	readonly type: DenType;
	readonly mounted: boolean;
}

/** One row of a listing. */
export type DenEntry = OpenEntry | CardEntry;

/** One folder above the open node. */
export interface DenCrumb {
	readonly href: DenHref;
	readonly name: DenName;
}

/** What an open node holds. */
export type DenBody =
	| {
			readonly body: 'listing';
			readonly entries: readonly DenEntry[];
			readonly more: DenHref | undefined;
	  }
	| { readonly body: 'file' }
	| { readonly body: 'link' }
	/** A content case this build doesn't know: shown as "content not shown yet". */
	| { readonly body: 'notShown' };

/** The node the address lands on, with what goes with it. */
export type DenPage =
	| {
			readonly view: 'open';
			readonly node: OpenEntry;
			readonly crumbs: readonly DenCrumb[];
			readonly body: DenBody;
	  }
	| { readonly view: 'card'; readonly node: CardEntry; readonly crumbs: readonly DenCrumb[] };

/** How a Den read came back. */
export type DenOutcome =
	| {
			readonly outcome: 'page';
			readonly page: DenPage;
			/** This address with "Show archived and deactivated" on, and off. */
			readonly flagLinks: { readonly on: DenHref; readonly off: DenHref };
	  }
	/** Every not-found, whatever its cause, looks the same. */
	| { readonly outcome: 'notFound' }
	/** This build has no live Den behind it yet. */
	| { readonly outcome: 'notConnected' }
	| { readonly outcome: 'problem'; readonly problem: Problem };

/** One folder's listing, as the tree keeps it. */
export interface DenListing {
	readonly folder: DenHref;
	readonly entries: readonly DenEntry[];
	readonly more: DenHref | undefined;
}

/** Everything the den page's load hands the pane and the frame. */
export interface DenPageData {
	readonly kind: 'denPage';
	readonly outcome: DenOutcome;
	/** My Den's root, with the current flag. */
	readonly rootHref: DenHref;
	/** Whether "Show archived and deactivated" is on for this address. */
	readonly includeDeleted: boolean;
	/** Whether this answer is a later page of a folder rather than its first. */
	readonly continued: boolean;
	/** The listings of the folders above the node, read on a full page load. */
	readonly ancestors: readonly DenListing[];
	readonly title: string;
	readonly trail: Trail;
}

/** `value[key]`, or `undefined` when `value` isn't an object: reading untyped data one field at a time. */
function field(value: unknown, key: string): unknown {
	return value instanceof Object ? Reflect.get(value, key) : undefined;
}

/** Whether `value` is one of `allowed`. */
function oneOf(value: unknown, allowed: readonly unknown[]): boolean {
	return allowed.includes(value);
}

/** An origin no request ever reaches, for parsing a link on its own. */
const LINK_PARSE_BASE = 'http://den-link.invalid';

/**
 * Whether `value` is a Den link: `/den`, `/den/…` or `/den?…`, with no `//`
 * and no `\`, whose path and query the URL parser reads back unchanged — so
 * a dot segment (raw or encoded) that resolution would move out of the Den
 * never passes.
 */
export function isDenLink(value: unknown): boolean {
	if (typeof value !== 'string') return false;
	if (value.includes('//') || value.includes('\\')) return false;
	if (!(value === '/den' || value.startsWith('/den/') || value.startsWith('/den?'))) return false;
	const parsed = new URL(value, LINK_PARSE_BASE);
	return parsed.origin === LINK_PARSE_BASE && `${parsed.pathname}${parsed.search}` === value;
}

/** A Den link, or absent. */
function isOptionalDenLink(value: unknown): boolean {
	return value === undefined || isDenLink(value);
}

/** Whether `value` is a listing row, every link in it a Den link. */
function isDenEntry(value: unknown): boolean {
	const names =
		typeof field(value, 'name') === 'string' && typeof field(value, 'type') === 'string';
	const mounted = typeof field(value, 'mounted') === 'boolean';
	switch (field(value, 'view')) {
		case 'card':
			return names && mounted;
		case 'open':
			return (
				names &&
				mounted &&
				isDenLink(field(value, 'href')) &&
				oneOf(field(value, 'kind'), ['directory', 'file', 'symlink', 'unknown']) &&
				oneOf(field(value, 'ownLevel'), ['private', 'listed', 'public', 'unknown']) &&
				typeof field(value, 'contentNotShown') === 'boolean' &&
				oneOf(field(value, 'removed'), [undefined, 'archived', 'deactivated', 'unknown'])
			);
		default:
			return false;
	}
}

/** Whether `value` is an array whose every item passes `check`. */
function everyItem(value: unknown, check: (item: unknown) => boolean): boolean {
	return Array.isArray(value) && value.every(check);
}

/** Whether `value` is a page's body. */
function isDenBody(value: unknown): boolean {
	switch (field(value, 'body')) {
		case 'listing':
			return (
				everyItem(field(value, 'entries'), isDenEntry) && isOptionalDenLink(field(value, 'more'))
			);
		case 'file':
		case 'link':
		case 'notShown':
			return true;
		default:
			return false;
	}
}

/** Whether `value` is a page: its node, its crumbs and (open) its body. */
function isDenPage(value: unknown): boolean {
	const crumbs = everyItem(
		field(value, 'crumbs'),
		(crumb) => isDenLink(field(crumb, 'href')) && typeof field(crumb, 'name') === 'string'
	);
	const node = field(value, 'node');
	switch (field(value, 'view')) {
		case 'card':
			return crumbs && field(node, 'view') === 'card' && isDenEntry(node);
		case 'open':
			return (
				crumbs &&
				field(node, 'view') === 'open' &&
				isDenEntry(node) &&
				isDenBody(field(value, 'body'))
			);
		default:
			return false;
	}
}

/** Whether `value` is a Den outcome. */
function isDenOutcome(value: unknown): boolean {
	switch (field(value, 'outcome')) {
		case 'page': {
			const flagLinks = field(value, 'flagLinks');
			return (
				isDenPage(field(value, 'page')) &&
				isDenLink(field(flagLinks, 'on')) &&
				isDenLink(field(flagLinks, 'off'))
			);
		}
		case 'notFound':
		case 'notConnected':
			return true;
		case 'problem':
			return field(value, 'problem') instanceof Object;
		default:
			return false;
	}
}

/**
 * Whether `value` is a {@link DenPageData}, as a preload hands it back
 * untyped. It checks every field it vouches for, and that every link in it
 * is shaped like a Den link, so nothing else can pass as one.
 */
export function isDenPageData(value: unknown): value is DenPageData {
	return (
		field(value, 'kind') === 'denPage' &&
		isDenLink(field(value, 'rootHref')) &&
		typeof field(value, 'includeDeleted') === 'boolean' &&
		typeof field(value, 'continued') === 'boolean' &&
		typeof field(value, 'title') === 'string' &&
		everyItem(
			field(value, 'ancestors'),
			(listing) =>
				isDenLink(field(listing, 'folder')) &&
				everyItem(field(listing, 'entries'), isDenEntry) &&
				isOptionalDenLink(field(listing, 'more'))
		) &&
		everyItem(field(value, 'trail'), (step) => {
			const href = field(step, 'href');
			return href === undefined || href === '/accounts' || isDenLink(href);
		}) &&
		isDenOutcome(field(value, 'outcome'))
	);
}

/** The entries of a listing in display order: folders first, then everything else, each in the server's order. */
export function foldersFirst(entries: readonly DenEntry[]): readonly DenEntry[] {
	const isFolder = (entry: DenEntry) => entry.view === 'open' && entry.kind === 'directory';
	return [...entries.filter(isFolder), ...entries.filter((entry) => !isFolder(entry))];
}
