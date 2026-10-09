/**
 * The den page's program over the {@link import('./api/zurfur-api').ZurfurApi}
 * port: read the node an address asks for (and, on a full page load, the
 * folders above it), then map the read to the web's own Den view, building
 * every link through the Den path builder. Pages run it through
 * {@link import('./runtime').runApi}.
 */

import { Effect } from 'effect';
import type {
	DenBody,
	DenCrumb,
	DenEntry,
	DenKind,
	DenLevel,
	DenListing,
	DenOutcome,
	DenPage,
	DenPageData,
	DenRemoved,
	OpenEntry
} from '$lib/api/den';
import { ProblemKind } from '$lib/api/problem';
import { HttpStatus } from '$lib/api/http-status';
import type { Trail, TrailStep } from '$lib/api/trail';
import { denName, denType, pageToken, type DenHref } from '$lib/types/brand';
import { ContractViolation, type NetworkFailure } from './api/errors';
import type { DenQuery, DenRead, DenReadContent, DenReadNode } from './api/den-read';
import { ZurfurApi } from './api/zurfur-api';
import type { SegmentPath } from './path-builder';
import {
	denHrefOf,
	denPathFromPathname,
	denPathFromSegments,
	denQueryFromSearch,
	PLAIN_QUERY
} from './den/den-path';

/** The template a Den contract error names, never the real path. */
export const DEN_ROUTE_TEMPLATE = '/den/{path}';

/** How the den page's load should answer. */
export type DenPageResult =
	| { readonly result: 'page'; readonly data: DenPageData }
	/** A stale page token: load the folder again from its first page. */
	| { readonly result: 'reload'; readonly location: DenHref }
	/** The session ended: go and sign in again. */
	| { readonly result: 'signedOut' };

/** A contract error for the Den read, naming only the route template. */
function denViolation(detail: string): ContractViolation {
	return new ContractViolation({ path: DEN_ROUTE_TEMPLATE, status: HttpStatus.Ok, detail });
}

/** `value` if it is a known level, else `'unknown'`, which shows no level. */
function toLevel(value: string): DenLevel {
	if (value === 'private' || value === 'listed' || value === 'public') return value;
	return 'unknown';
}

/** `value` if it is a known kind, else `'unknown'`, which gets a generic marker. */
function toKind(value: string): DenKind {
	if (value === 'directory' || value === 'file' || value === 'symlink') return value;
	return 'unknown';
}

/** The soft-delete word, folded to `'unknown'` when this build doesn't know it. */
function toRemoved(value: string | undefined): DenRemoved | undefined {
	if (value === undefined) return undefined;
	if (value === 'archived' || value === 'deactivated') return value;
	return 'unknown';
}

/**
 * One node as a listing row. An open node whose path the builder refuses
 * can't be linked, so it shows as a card: it fails closed, never as a link
 * built from pieces the builder didn't accept.
 */
function toEntry(node: DenReadNode, query: DenQuery): DenEntry {
	const card: DenEntry = {
		view: 'card',
		name: denName(node.name),
		type: denType(node.type),
		mounted: node.mount
	};
	// Anything but exactly `open` — a card, or a value this build doesn't know — fails closed.
	if (node.access.access !== 'open') return card;
	const path = denPathFromSegments(node.segments);
	const href = path === undefined ? undefined : denHrefOf(path, query);
	if (href === undefined) return card;
	const open: OpenEntry = {
		view: 'open',
		href,
		name: denName(node.name),
		type: denType(node.type),
		kind: toKind(node.kind),
		mounted: node.mount,
		ownLevel: toLevel(node.access.ownLevel),
		contentNotShown: node.access.contentNotShown,
		removed: toRemoved(node.access.removed)
	};
	return open;
}

/** The query every link inside a listing carries: the flag, never a page token. */
function linkQuery(query: DenQuery): DenQuery {
	return { includeDeleted: query.includeDeleted, pageToken: undefined };
}

/** A listing's "More" link: the same folder, at the next page. */
function moreHref(
	path: SegmentPath,
	query: DenQuery,
	nextPageToken: string | undefined
): DenHref | undefined {
	if (nextPageToken === undefined) return undefined;
	const token = pageToken(nextPageToken);
	if (token === undefined) return undefined;
	return denHrefOf(path, { includeDeleted: query.includeDeleted, pageToken: token });
}

/** What an open node holds, with its links built. */
function toBody(content: DenReadContent, path: SegmentPath, query: DenQuery): DenBody {
	switch (content.content) {
		case 'listing':
			return {
				body: 'listing',
				entries: content.entries.map((entry) => toEntry(entry, linkQuery(query))),
				more: moreHref(path, query, content.nextPageToken)
			};
		case 'file':
			return { body: 'file' };
		case 'link':
			return { body: 'link' };
		case 'none':
			return { body: 'notShown' };
	}
}

/** The crumbs with their links, or `undefined` if the builder refuses any of them. */
function toCrumbs(read: DenRead, query: DenQuery): readonly DenCrumb[] | undefined {
	const crumbs: DenCrumb[] = [];
	for (const crumb of read.crumbs) {
		const path = denPathFromSegments(crumb.segments);
		const href = path === undefined ? undefined : denHrefOf(path, linkQuery(query));
		if (href === undefined) return undefined;
		crumbs.push({ href, name: denName(crumb.name) });
	}
	return crumbs;
}

/** The read as the web's page view, or a contract error naming only the template. */
function toPage(
	read: DenRead,
	path: SegmentPath,
	query: DenQuery
): Effect.Effect<DenPage, ContractViolation> {
	const crumbs = toCrumbs(read, query);
	if (crumbs === undefined) return Effect.fail(denViolation('a crumb the path builder refuses'));
	const selfHref = denHrefOf(path, linkQuery(query));
	if (selfHref === undefined) return Effect.fail(denViolation('a path the builder refuses'));

	const node = read.node;
	if (node.access.access !== 'open') {
		const card: DenPage = {
			view: 'card',
			node: {
				view: 'card',
				name: denName(node.name),
				type: denType(node.type),
				mounted: node.mount
			},
			crumbs
		};
		return Effect.succeed(card);
	}
	const open: OpenEntry = {
		view: 'open',
		href: selfHref,
		name: denName(node.name),
		type: denType(node.type),
		kind: toKind(node.kind),
		mounted: node.mount,
		ownLevel: toLevel(node.access.ownLevel),
		contentNotShown: node.access.contentNotShown,
		removed: toRemoved(node.access.removed)
	};
	const page: DenPage = {
		view: 'open',
		node: open,
		crumbs,
		body: toBody(read.content, path, query)
	};
	return Effect.succeed(page);
}

/** Read `path` through the port: the root read for an empty path, the node read otherwise. */
function readDen(path: SegmentPath, query: DenQuery) {
	return Effect.gen(function* () {
		const api = yield* ZurfurApi;
		return yield* path.length === 0 ? api.denRoot(query) : api.denNode(path, query);
	});
}

/**
 * The listings of the folders above `path` — the root and each prefix —
 * read in parallel with the first page and no page token. A folder that
 * fails to read is left out; the tree fills it in later.
 */
function readAncestors(
	path: SegmentPath,
	query: DenQuery
): Effect.Effect<readonly DenListing[], never, ZurfurApi> {
	const prefixes = path.slice(0, -1).map((_, index) => path.slice(0, index + 1));
	const folders: SegmentPath[] = [[], ...prefixes];
	const plain = linkQuery(query);
	const reads = folders.map((folder) =>
		readDen(folder, plain).pipe(
			Effect.map((read): DenListing | undefined => {
				const folderHref = denHrefOf(folder, plain);
				if (folderHref === undefined || read.node.access.access !== 'open') return undefined;
				const body = toBody(read.content, folder, plain);
				if (body.body !== 'listing') return undefined;
				return { folder: folderHref, entries: body.entries, more: body.more };
			}),
			Effect.catchAll(() => Effect.succeed(undefined))
		)
	);
	return Effect.all(reads, { concurrency: 'unbounded' }).pipe(
		Effect.map((listings) => listings.filter((listing) => listing !== undefined))
	);
}

/** The path bar for a page: `~`, each crumb below the root, then the node itself. */
function pageTrail(rootHref: DenHref, outcome: DenOutcome): Trail {
	const root: TrailStep = { step: 'root', href: rootHref };
	if (outcome.outcome !== 'page') return [root];
	const { page } = outcome;
	if (page.crumbs.length === 0 && page.view === 'open') return [{ step: 'root', href: undefined }];
	const below: TrailStep[] = page.crumbs
		.slice(1)
		.map((crumb) => ({ step: 'named', label: crumb.name, href: crumb.href }));
	const self: TrailStep = { step: 'named', label: page.node.name, href: undefined };
	return [root, ...below, self];
}

/**
 * Every Den page's title: the same generic words, so no Private name lands in
 * the browser's history. The pane announces the open item's name itself.
 */
export const DEN_TITLE = 'My Den · Zurfur';

/** The outcome of reading `path`, or how the load must answer instead. */
function readOutcome(
	path: SegmentPath,
	query: DenQuery
): Effect.Effect<
	DenOutcome | { readonly reload: true } | { readonly signedOut: true },
	NetworkFailure | ContractViolation,
	ZurfurApi
> {
	return readDen(path, query).pipe(
		Effect.flatMap((read) => toPage(read, path, query)),
		Effect.flatMap((page) => {
			const on = denHrefOf(path, { includeDeleted: true, pageToken: undefined });
			const off = denHrefOf(path, PLAIN_QUERY);
			if (on === undefined || off === undefined)
				return Effect.fail(denViolation('a path the builder refuses'));
			const outcome: DenOutcome = { outcome: 'page', page, flagLinks: { on, off } };
			return Effect.succeed(outcome);
		}),
		Effect.catchTags({
			DenNotConnected: () => Effect.succeed<DenOutcome>({ outcome: 'notConnected' }),
			NotAuthenticated: () => Effect.succeed({ signedOut: true } as const),
			ApiProblem: ({ problem }) => {
				if (problem.code === ProblemKind.NodeNotFound.code)
					return Effect.succeed<DenOutcome>({ outcome: 'notFound' });
				if (problem.code === ProblemKind.NotAuthenticated.code)
					return Effect.succeed({ signedOut: true } as const);
				// The web sends only allowlisted parameters, so a 400 is a web bug,
				// and a 422 with no page token can't come from a stale token.
				if (problem.status === HttpStatus.BadRequest)
					return Effect.fail(denViolation('the Den refused the query'));
				if (problem.status === HttpStatus.UnprocessableContent) {
					if (query.pageToken !== undefined) return Effect.succeed({ reload: true } as const);
					return Effect.fail(denViolation('the Den refused the query'));
				}
				return Effect.succeed<DenOutcome>({ outcome: 'problem', problem });
			}
		})
	);
}

/**
 * The den page for a browser address: the node at its path, read through the
 * port, mapped to the web's view. `withAncestors` (a full page load) also
 * reads the folders above it, in parallel, so the first paint shows the tree
 * opened down to the node. A refused address is the not-found without any
 * read.
 */
export function denPage(
	pathname: string,
	search: URLSearchParams,
	withAncestors: boolean
): Effect.Effect<DenPageResult, NetworkFailure | ContractViolation, ZurfurApi> {
	return Effect.gen(function* () {
		const query = denQueryFromSearch(search);
		const plain = linkQuery(query);
		const rootHref = denHrefOf([], plain);
		if (rootHref === undefined) return yield* denViolation('the root the builder refuses');

		const path = denPathFromPathname(pathname);
		if (path === undefined) {
			const outcome: DenOutcome = { outcome: 'notFound' };
			const data: DenPageData = {
				kind: 'denPage',
				outcome,
				rootHref,
				includeDeleted: query.includeDeleted,
				continued: false,
				ancestors: [],
				title: DEN_TITLE,
				trail: pageTrail(rootHref, outcome)
			};
			return { result: 'page', data } satisfies DenPageResult;
		}

		const ancestorsRead =
			withAncestors && path.length > 0 ? readAncestors(path, query) : Effect.succeed([]);
		const [read, ancestors] = yield* Effect.all([readOutcome(path, query), ancestorsRead], {
			concurrency: 'unbounded'
		});

		if ('signedOut' in read) return { result: 'signedOut' } satisfies DenPageResult;
		if ('reload' in read) {
			const location = denHrefOf(path, plain);
			if (location === undefined) return yield* denViolation('a path the builder refuses');
			return { result: 'reload', location } satisfies DenPageResult;
		}

		const data: DenPageData = {
			kind: 'denPage',
			outcome: read,
			rootHref,
			includeDeleted: query.includeDeleted,
			continued: query.pageToken !== undefined,
			ancestors,
			title: DEN_TITLE,
			trail: pageTrail(rootHref, read)
		};
		return { result: 'page', data } satisfies DenPageResult;
	});
}
