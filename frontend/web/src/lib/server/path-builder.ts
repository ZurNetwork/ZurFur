/**
 * The one way the web server puts a path together from pieces: every piece
 * is minted as a {@link PathSegment} (the allowlist in `segment-format.ts`),
 * at most {@link MAX_SEGMENTS} of them, each encoded on its own, and the
 * result is parsed back as a URL to prove the parser sees exactly the string
 * that was built — so no piece can climb out of its prefix or reach another
 * endpoint. Anything refused comes back as `undefined`.
 */

import { MAX_SEGMENTS } from '$lib/types/segment-format';
import { pathSegment, type PathSegment } from '$lib/types/brand';

/** A path the builder accepted: its pieces, each a minted segment. */
export type SegmentPath = readonly PathSegment[];

/** An origin no request ever reaches, for parsing a built path on its own. */
const PARSE_BASE = 'http://path-builder.invalid';

/** `pieces` as a {@link SegmentPath}, or `undefined` if any piece, or their count, is refused. */
export function segmentPath(pieces: readonly string[]): SegmentPath | undefined {
	if (pieces.length > MAX_SEGMENTS) return undefined;
	const minted: PathSegment[] = [];
	for (const piece of pieces) {
		const segment = pathSegment(piece);
		if (segment === undefined) return undefined;
		minted.push(segment);
	}
	return minted;
}

/**
 * One segment as it sits in a URL path. A segment holds only characters a
 * path may carry as they are, apart from `%`, which is escaped so the
 * receiver's single decode gives the segment back unchanged.
 */
function encodeSegment(segment: PathSegment): string {
	return segment.replaceAll('%', '%25');
}

/**
 * `prefix` followed by each segment of `path`, or `undefined` when the URL
 * parser would read the result as anything else (a dot segment it resolves,
 * a character it rewrites). `prefix` is a fixed, absolute path such as
 * `/den` or `/accounts`, never input.
 */
export function joinPath(prefix: string, path: SegmentPath): string | undefined {
	const built = [prefix, ...path.map(encodeSegment)].join('/');
	const parsed = new URL(built, PARSE_BASE);
	if (parsed.pathname !== built || parsed.search !== '' || parsed.hash !== '') return undefined;
	return built;
}

/**
 * `search` appended to an already built path, when it has any parameter.
 * The parameters come from an allowlist the caller owns, never from an
 * incoming query string.
 */
export function withSearch(path: string, search: URLSearchParams): string {
	const query = search.toString();
	return query === '' ? path : `${path}?${query}`;
}
