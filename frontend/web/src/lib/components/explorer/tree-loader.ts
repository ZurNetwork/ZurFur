/**
 * How the tree and the pane fetch a folder's listing without opening it: the
 * den page's own load, run through SvelteKit's `preloadData` on a link the
 * server built. A redirect (the session ended) is followed as a full page
 * load; any other failure is a value the caller shows as "Couldn't load".
 */

import { preloadData } from '$app/navigation';
import { isDenLink, isDenPageData, type DenEntry } from '$lib/api/den';
import { HttpStatus } from '$lib/api/http-status';
import type { DenHref } from '$lib/types/brand';
import type { TreeMemory } from './tree-memory.svelte';

/** How a listing fetch came back. */
export type ListingFetch =
	| {
			readonly fetched: 'listing';
			readonly entries: readonly DenEntry[];
			readonly more: DenHref | undefined;
	  }
	| { readonly fetched: 'failed' }
	| { readonly fetched: 'redirected' };

/** The function that preloads a page's data; SvelteKit's, unless a test hands in its own. */
export type Preload = typeof preloadData;

/** Whether a fetch is the first try for its link, or a retry after a failure. */
export type Attempt = 'first' | 'retry';

/** How a fetch runs: as a first try or a retry, and through which preload. */
export interface FetchOptions {
	readonly attempt?: Attempt | undefined;
	readonly preload?: Preload | undefined;
}

/**
 * The query parameter a retry carries, so SvelteKit's preload cache (keyed by
 * path and query) can't replay the failed answer. The den page's load reads
 * only its allowlisted parameters, so it never sees this one.
 */
const RETRY_PARAM = 'retry';

/** How many retries this page has run: each one's address is new. */
let retries = 0;

/** The address to preload for `href`: itself on a first try, made unique on a retry. */
function preloadAddress(href: DenHref, attempt: Attempt): string {
	if (attempt === 'first') return href;
	retries += 1;
	const separator = href.includes('?') ? '&' : '?';
	return `${href}${separator}${RETRY_PARAM}=${String(retries)}`;
}

/**
 * The listing behind `href`, fetched through the den page's load. Only an
 * answer for `href` itself counts: when its fetch fails, SvelteKit answers
 * with the current page instead, and that is a failure here.
 */
export async function fetchListing(
	href: DenHref,
	{ attempt = 'first', preload = preloadData }: FetchOptions = {}
): Promise<ListingFetch> {
	let result: Awaited<ReturnType<Preload>>;
	try {
		result = await preload(preloadAddress(href, attempt));
	} catch {
		return { fetched: 'failed' };
	}
	if (result.type === 'redirect') {
		// Only the sign-in redirect is followed. A redirect back into the Den (a
		// stale page token reloading its folder) would be a full page load into
		// that folder, so the fetch just fails.
		if (isDenLink(result.location)) return { fetched: 'failed' };
		window.location.assign(result.location);
		return { fetched: 'redirected' };
	}
	const den: unknown = result.data.den;
	if (result.status !== HttpStatus.Ok || !isDenPageData(den)) return { fetched: 'failed' };
	if (den.address !== href) return { fetched: 'failed' };
	const { outcome } = den;
	if (outcome.outcome !== 'page' || outcome.page.view !== 'open') return { fetched: 'failed' };
	const { body } = outcome.page;
	if (body.body !== 'listing') return { fetched: 'failed' };
	return { fetched: 'listing', entries: body.entries, more: body.more };
}

/** Fetch `folder`'s first page into the tree's memory. */
export async function loadFolder(
	memory: TreeMemory,
	folder: DenHref,
	options: FetchOptions = {}
): Promise<void> {
	const generation = memory.generation;
	memory.loading(folder);
	const fetched = await fetchListing(folder, options);
	if (memory.generation !== generation) return;
	switch (fetched.fetched) {
		case 'listing':
			memory.loaded(folder, fetched.entries, fetched.more);
			return;
		case 'failed':
			memory.failed(folder);
			return;
		case 'redirected':
			return;
	}
}

/**
 * Fetch the page after `folder`'s loaded ones, through its "More" link, and
 * append it. Answers whether it was appended (not dropped as stale, nor
 * failed). A failure counts only while the folder still expects that page.
 */
export async function loadMore(
	memory: TreeMemory,
	folder: DenHref,
	more: DenHref,
	options: FetchOptions = {}
): Promise<boolean> {
	const generation = memory.generation;
	memory.moreState(folder, 'loading');
	const fetched = await fetchListing(more, options);
	if (memory.generation !== generation) return false;
	switch (fetched.fetched) {
		case 'listing':
			return memory.appended(folder, fetched.entries, fetched.more, more);
		case 'failed':
			memory.moreFailed(folder, more);
			return false;
		case 'redirected':
			return false;
	}
}
