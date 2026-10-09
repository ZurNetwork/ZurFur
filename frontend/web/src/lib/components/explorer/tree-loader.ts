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

/** The listing behind `href`, fetched through the den page's load. */
export async function fetchListing(
	href: DenHref,
	preload: Preload = preloadData
): Promise<ListingFetch> {
	let result: Awaited<ReturnType<Preload>>;
	try {
		result = await preload(href);
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
	preload?: Preload
): Promise<void> {
	const generation = memory.generation;
	memory.loading(folder);
	const fetched = await fetchListing(folder, preload);
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
 * append it. Answers whether it was appended (not dropped as stale, nor failed).
 */
export async function loadMore(
	memory: TreeMemory,
	folder: DenHref,
	more: DenHref,
	preload?: Preload
): Promise<boolean> {
	const generation = memory.generation;
	memory.moreState(folder, 'loading');
	const fetched = await fetchListing(more, preload);
	if (memory.generation !== generation) return false;
	switch (fetched.fetched) {
		case 'listing':
			return memory.appended(folder, fetched.entries, fetched.more, more);
		case 'failed':
			memory.moreState(folder, 'failed');
			return false;
		case 'redirected':
			return false;
	}
}
