/**
 * The Den's path builder: the only code that turns a browser address into a
 * Den read, and the only code that mints a {@link DenHref}. It refuses empty,
 * `.` and `..` pieces (raw or percent-encoded), anything outside the segment
 * allowlist, and more than 16 pieces; it decodes each piece exactly once;
 * and it carries only allowlisted query parameters, never the incoming query
 * string. Used only by the den page's load and the den program it runs.
 */

import { DEN_ROOT } from '$lib/api/den-route';
import { denHref, pageToken, type DenHref } from '$lib/types/brand';
import { joinPath, segmentPath, withSearch, type SegmentPath } from '../path-builder';
import type { DenQuery } from '../api/den-read';

/** The query parameter for "Show archived and deactivated". */
export const INCLUDE_DELETED_PARAM = 'includeDeleted';

/** The query parameter for a later page's token. */
export const PAGE_TOKEN_PARAM = 'pageToken';

/** The first page with nothing deleted shown: the query every plain Den link carries. */
export const PLAIN_QUERY: DenQuery = { includeDeleted: false, pageToken: undefined };

/** Each piece decoded once, strictly; `undefined` for a malformed escape. */
function decodeOnce(piece: string): string | undefined {
	try {
		return decodeURIComponent(piece);
	} catch {
		return undefined;
	}
}

/**
 * The Den path a browser pathname asks for: what follows `/den`, split on
 * `/`, each piece decoded once and minted as a segment. `undefined` when the
 * address is refused — which the page shows as the one not-found.
 */
export function denPathFromPathname(pathname: string): SegmentPath | undefined {
	const root = DEN_ROOT;
	if (pathname === root) return [];
	if (!pathname.startsWith(`${root}/`)) return undefined;

	const pieces: string[] = [];
	for (const raw of pathname.slice(root.length + 1).split('/')) {
		const decoded = decodeOnce(raw);
		if (decoded === undefined) return undefined;
		pieces.push(decoded);
	}
	return segmentPath(pieces);
}

/**
 * The query a browser address asks for, from the allowlist only: the flag
 * is on for exactly one `includeDeleted=true`, and a page token is kept when
 * it is exactly one well-formed token. Everything else is dropped.
 */
export function denQueryFromSearch(search: URLSearchParams): DenQuery {
	const flags = search.getAll(INCLUDE_DELETED_PARAM);
	const tokens = search.getAll(PAGE_TOKEN_PARAM);
	const [onlyFlag] = flags;
	const [onlyToken] = tokens;
	return {
		includeDeleted: flags.length === 1 && onlyFlag === 'true',
		pageToken: tokens.length === 1 && onlyToken !== undefined ? pageToken(onlyToken) : undefined
	};
}

/** The allowlisted query parameters for `query`, in a fixed order. */
function querySearch(query: DenQuery): URLSearchParams {
	const search = new URLSearchParams();
	if (query.includeDeleted) search.set(INCLUDE_DELETED_PARAM, 'true');
	if (query.pageToken !== undefined) search.set(PAGE_TOKEN_PARAM, query.pageToken);
	return search;
}

/** The browser link to `path` under `query`, or `undefined` if the builder refuses it. */
export function denHrefOf(path: SegmentPath, query: DenQuery): DenHref | undefined {
	const built = joinPath(DEN_ROOT, path);
	if (built === undefined) return undefined;
	return denHref(withSearch(built, querySearch(query)));
}

/**
 * The API path (below the `/api/v1` prefix) that reads `path` under
 * `query`: `/den` for the root, `/den/<segment>/…` below it.
 */
export function denApiPathOf(path: SegmentPath, query: DenQuery): string | undefined {
	const built = joinPath('/den', path);
	if (built === undefined) return undefined;
	return withSearch(built, querySearch(query));
}

/** The pieces the backend sent for a node, as a path the builder accepts, or `undefined`. */
export function denPathFromSegments(segments: readonly string[]): SegmentPath | undefined {
	return segmentPath(segments);
}
