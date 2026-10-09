/**
 * The tree's memory: every folder listing the frame has seen, which folders
 * are open, and whose Den it is. It lives in the signed-in frame (one per
 * frame instance, never module-level, so it is never shared between
 * visitors) and nothing in it is ever written to browser storage.
 */

import { getContext, setContext } from 'svelte';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import type { DenEntry, DenPageData } from '$lib/api/den';
import type { DenHref, Did } from '$lib/types/brand';

/** Where a folder's listing stands in the tree. */
export type FolderState =
	| { readonly state: 'loading' }
	| { readonly state: 'failed' }
	| {
			readonly state: 'loaded';
			readonly entries: readonly DenEntry[];
			readonly more: DenHref | undefined;
			/** Whether the next page is being fetched, or failed to. */
			readonly moreState: 'idle' | 'loading' | 'failed';
	  };

/** The tree's memory for one signed-in visitor. */
export class TreeMemory {
	/** Each folder's listing, keyed by its link. */
	readonly folders = new SvelteMap<DenHref, FolderState>();
	/** The folders open in the tree. */
	readonly expanded = new SvelteSet<DenHref>();
	/** My Den's root link, once a Den page has said it. */
	rootHref: DenHref | undefined = $state();
	/** Whether the remembered listings were read with "Show archived and deactivated" on. */
	includeDeleted = $state(false);
	/** Bumped on every clear, so a fetch that started before it can't write back after it. */
	generation = 0;
	#owner: Did;

	constructor(owner: Did) {
		this.#owner = owner;
	}

	/** Forget every listing, every open folder and the root. */
	clear(): void {
		this.folders.clear();
		this.expanded.clear();
		this.rootHref = undefined;
		this.generation += 1;
	}

	/**
	 * Start over after a form action succeeded (what any listing holds may
	 * have changed, such as a newly founded Account's mount): forget every
	 * listing, but keep `~` open, so its first page is fetched again.
	 */
	restart(fallbackRoot: DenHref): void {
		const root = this.rootHref ?? fallbackRoot;
		this.clear();
		this.rootHref = root;
		this.expanded.add(root);
	}

	/** Start over when the visitor is someone else. */
	ensureOwner(owner: Did): void {
		if (owner === this.#owner) return;
		this.clear();
		this.#owner = owner;
	}

	/**
	 * Take in what a Den page brought: its listing and its ancestors'
	 * listings, its crumbs opened in the tree (the open item itself stays
	 * closed). A flipped "Show archived" flag forgets everything first, so the
	 * tree and the pane never disagree. A later page of a folder is not its
	 * first page, so it doesn't replace the remembered listing.
	 */
	absorb(data: DenPageData | undefined): void {
		if (data === undefined) return;
		if (data.includeDeleted !== this.includeDeleted) {
			this.clear();
			this.includeDeleted = data.includeDeleted;
		}
		if (this.rootHref !== data.rootHref) this.expanded.add(data.rootHref);
		this.rootHref = data.rootHref;

		for (const ancestor of data.ancestors)
			this.loaded(ancestor.folder, ancestor.entries, ancestor.more);

		const { outcome } = data;
		if (outcome.outcome !== 'page') return;
		const { page } = outcome;
		for (const crumb of page.crumbs) this.expanded.add(crumb.href);
		if (page.view === 'open' && page.body.body === 'listing' && !data.continued) {
			this.loaded(page.node.href, page.body.entries, page.body.more);
		}
	}

	/**
	 * Catch up after a navigation: start over for another visitor, take in the
	 * new page, and hand back the folders the tree has to fetch.
	 */
	navigated(owner: Did, data: DenPageData | undefined): readonly DenHref[] {
		this.ensureOwner(owner);
		this.absorb(data);
		return this.foldersToFetch(data);
	}

	/**
	 * The folders the tree shows open but has no listing for: the crumbs of
	 * `data` it lacks, and `~` itself when it is open with nothing under it
	 * (after a restart, or on a not-found that read no ancestors).
	 */
	foldersToFetch(data: DenPageData | undefined): readonly DenHref[] {
		const missing = this.missingCrumbs(data);
		const root = this.rootHref;
		const rootLacking =
			root !== undefined &&
			this.expanded.has(root) &&
			!this.folders.has(root) &&
			!missing.includes(root);
		return rootLacking ? [root, ...missing] : missing;
	}

	/** The crumbs of `data` whose listings the tree doesn't have and isn't fetching. */
	missingCrumbs(data: DenPageData | undefined): readonly DenHref[] {
		if (data?.outcome.outcome !== 'page') return [];
		return data.outcome.page.crumbs
			.map((crumb) => crumb.href)
			.filter((href) => !this.folders.has(href));
	}

	/** Remember `folder`'s first page. */
	loaded(folder: DenHref, entries: readonly DenEntry[], more: DenHref | undefined): void {
		this.folders.set(folder, { state: 'loaded', entries, more, moreState: 'idle' });
	}

	/** Mark `folder` as being fetched. */
	loading(folder: DenHref): void {
		this.folders.set(folder, { state: 'loading' });
	}

	/** Mark `folder` as failed to fetch. */
	failed(folder: DenHref): void {
		this.folders.set(folder, { state: 'failed' });
	}

	/** Mark `folder`'s next page as being fetched, or as failed. */
	moreState(folder: DenHref, moreState: 'idle' | 'loading' | 'failed'): void {
		const current = this.folders.get(folder);
		if (current?.state !== 'loaded') return;
		this.folders.set(folder, { ...current, moreState });
	}

	/**
	 * Append a later page to `folder`'s listing — only when it is the page the
	 * folder still expects next (`fetchedFrom` is its current "More" link), so
	 * the pane and the tree fetching the same page can't append it twice.
	 */
	appended(
		folder: DenHref,
		entries: readonly DenEntry[],
		more: DenHref | undefined,
		fetchedFrom: DenHref
	): void {
		const current = this.folders.get(folder);
		if (current?.state !== 'loaded' || current.more !== fetchedFrom) return;
		this.folders.set(folder, {
			state: 'loaded',
			entries: [...current.entries, ...entries],
			more,
			moreState: 'idle'
		});
	}
}

/** The context key the frame shares its tree memory under. */
const TREE_MEMORY_KEY = Symbol('tree-memory');

/** Share `memory` with the pages inside the frame. */
export function provideTreeMemory(memory: TreeMemory): void {
	setContext(TREE_MEMORY_KEY, memory);
}

/** The frame's tree memory, when the page sits inside the frame. */
export function treeMemoryFromContext(): TreeMemory | undefined {
	const memory: unknown = getContext(TREE_MEMORY_KEY);
	return memory instanceof TreeMemory ? memory : undefined;
}
