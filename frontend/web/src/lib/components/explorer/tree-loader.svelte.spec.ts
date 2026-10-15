import { describe, expect, it } from 'vitest';
import { denHref, did, type DenHref } from '$lib/types/brand';
import {
	FetchQueue,
	fetchListing,
	FOLDER_FETCHES_IN_FLIGHT,
	loadFolder,
	loadFolders,
	loadMore,
	type Preload
} from './tree-loader';
import type { DenEntry, DenPageData, OpenEntry } from '$lib/api/den';
import { denName, denType } from '$lib/types/brand';
import { TreeMemory } from './tree-memory.svelte';

const ROOT = denHref('/den');

/** A preload that answers only when `release` is called, with an empty Den listing. */
function heldPreload() {
	let release: () => void = () => undefined;
	const preload: Preload = () =>
		new Promise((answer) => {
			release = () => {
				answer({
					type: 'loaded',
					status: 500,
					data: {}
				});
			};
		});
	return {
		preload,
		release: () => {
			release();
		}
	};
}

describe('tree loader', () => {
	it('takes an answer only for the address it asked for, never the current page SvelteKit falls back to', async () => {
		expect(await fetchListing(PAGE_TWO, { preload: offline() })).toEqual({ fetched: 'failed' });
		expect(await fetchListing(denHref('/den/other'), { preload: offline() })).toEqual({
			fetched: 'failed'
		});
		expect(await fetchListing(PAGE_TWO, { preload: answering([second]) })).toEqual({
			fetched: 'listing',
			entries: [second],
			more: undefined
		});
	});

	it('marks a folder failed, never listing the current page under it, when its fetch fails', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		await loadFolder(memory, denHref('/den/other'), { preload: offline() });
		expect(memory.folders.get(denHref('/den/other'))).toEqual({ state: 'failed' });
	});

	it('marks "More" failed, never appending the current page again, when its fetch fails', async () => {
		const memory = pagedFolder();
		const appended = await loadMore(memory, FOLDER, PAGE_TWO, { preload: offline() });
		expect(appended).toBe(false);
		expect(names(memory)).toEqual(['first']);
		expect(memory.folders.get(FOLDER)).toMatchObject({ moreState: 'failed', more: PAGE_TWO });
	});

	it('preloads a new address on every retry, so SvelteKit’s cache can’t replay the failure', async () => {
		const asked: string[] = [];
		const preload = answering([second], asked);
		await fetchListing(PAGE_TWO, { preload });
		const retried = await fetchListing(PAGE_TWO, { preload, attempt: 'retry' });
		await fetchListing(PAGE_TWO, { preload, attempt: 'retry' });

		expect(retried).toMatchObject({ fetched: 'listing' });
		expect(asked[0]).toBe(PAGE_TWO);
		expect(new Set(asked).size).toBe(3);
		expect(asked.every((href) => href.startsWith(PAGE_TWO))).toBe(true);
	});

	it('a failed tree "More…" after the pane appended that page leaves the next page unmarked', async () => {
		const memory = pagedFolder();
		const { preload, release } = heldPreload();
		const pending = loadMore(memory, FOLDER, PAGE_TWO, { preload });
		memory.appended(FOLDER, [second], denHref('/den/batch?pageToken=p1.2'), PAGE_TWO);
		release();
		await pending;

		expect(memory.folders.get(FOLDER)).toMatchObject({ moreState: 'idle' });
	});

	it('drops a folder fetch that finishes after the memory started over', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		const { preload, release } = heldPreload();
		const pending = loadFolder(memory, ROOT, { preload });

		memory.restart(ROOT);
		release();
		await pending;

		expect(memory.folders.has(ROOT)).toBe(false);
	});

	it('treats a redirect back into the Den (a stale token) as a failure, never a page load', async () => {
		const preload: Preload = () =>
			Promise.resolve({ type: 'redirect', location: '/den/commissions' });
		expect(await fetchListing(ROOT, { preload })).toEqual({ fetched: 'failed' });
	});

	it('tree "More…" then the pane’s stale "More": the page is appended once', async () => {
		const memory = pagedFolder();
		await loadMore(memory, FOLDER, PAGE_TWO, { preload: answering([second]) });
		memory.appended(FOLDER, [second], undefined, PAGE_TWO);

		expect(names(memory)).toEqual(['first', 'second']);
	});

	it('the pane’s "More" then the tree’s stale "More…": the page is appended once', async () => {
		const memory = pagedFolder();
		memory.appended(FOLDER, [second], undefined, PAGE_TWO);
		await loadMore(memory, FOLDER, PAGE_TWO, { preload: answering([second]) });

		expect(names(memory)).toEqual(['first', 'second']);
	});
});

describe('the frame’s folder fetches', () => {
	it('runs sixteen missing folders at most four at a time, and loads them all', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		const folders = sixteenFolders();
		const network = slowNetwork();

		await loadFolders(memory, folders, new FetchQueue(FOLDER_FETCHES_IN_FLIGHT), {
			preload: network.preload
		});

		expect(network.asked()).toHaveLength(16);
		expect(network.most()).toBe(4);
		expect(folders.every((folder) => memory.folders.get(folder)?.state === 'loaded')).toBe(true);
	});

	it('goes on past a failing fetch, marking only that folder failed', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		const folders = sixteenFolders();
		const failing = folders[0] ?? ROOT;
		const network = slowNetwork(failing);

		await loadFolders(memory, folders, new FetchQueue(FOLDER_FETCHES_IN_FLIGHT), {
			preload: network.preload
		});

		expect(network.most()).toBeLessThanOrEqual(4);
		expect(memory.folders.get(failing)).toEqual({ state: 'failed' });
		const rest = folders.slice(1).map((folder) => memory.folders.get(folder)?.state);
		expect(rest).toEqual(Array.from({ length: 15 }, () => 'loaded'));
	});

	it('never fetches a folder still waiting when the memory starts over', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		const network = slowNetwork();
		const loads = loadFolders(memory, sixteenFolders(), new FetchQueue(FOLDER_FETCHES_IN_FLIGHT), {
			preload: network.preload
		});

		memory.clear();
		await loads;

		expect(network.asked()).toHaveLength(4);
		expect(memory.folders.size).toBe(0);
	});

	it('starts waiting fetches in the order they came', async () => {
		const queue = new FetchQueue(1);
		const started: string[] = [];
		const task = (name: string) => async () => {
			started.push(name);
			await new Promise((settle) => setTimeout(settle, 0));
		};

		await Promise.all([queue.run(task('a')), queue.run(task('b')), queue.run(task('c'))]);

		expect(started).toEqual(['a', 'b', 'c']);
	});
});

/** Sixteen sibling folders under `~`. */
function sixteenFolders(): readonly DenHref[] {
	return Array.from({ length: 16 }, (_, place) => denHref(`/den/folder-${String(place)}`));
}

/**
 * A preload whose every answer takes a moment, an empty listing for the
 * address asked (or a rejection for `failing`), counting how many are in
 * flight at once.
 */
function slowNetwork(failing?: DenHref) {
	const asked: string[] = [];
	let inFlight = 0;
	let most = 0;
	const preload: Preload = async (href: string) => {
		asked.push(href);
		inFlight += 1;
		most = Math.max(most, inFlight);
		await new Promise((settle) => setTimeout(settle, 1));
		inFlight -= 1;
		if (href === failing) return Promise.reject(new TypeError('fetch failed'));
		return { type: 'loaded', status: 200, data: { den: batchPage(denHref(href), []) } };
	};
	return { preload, asked: () => asked, most: () => most };
}

const FOLDER = denHref('/den/batch');
const PAGE_TWO = denHref('/den/batch?pageToken=p1.1');

/** A file entry named `name`. */
function fileEntry(name: string): OpenEntry {
	return {
		view: 'open',
		href: denHref(`/den/batch/${name}`),
		name: denName(name),
		type: denType('file'),
		kind: 'file',
		mounted: false,
		ownLevel: 'private',
		contentNotShown: false,
		removed: undefined
	};
}

const second = fileEntry('second');

/** A memory whose folder has its first page loaded, with a second page behind "More". */
function pagedFolder(): TreeMemory {
	const memory = new TreeMemory(did('did:plc:alice'));
	memory.loaded(FOLDER, [fileEntry('first')], PAGE_TWO);
	return memory;
}

/** The names the folder lists in memory. */
function names(memory: TreeMemory): readonly string[] {
	const state = memory.folders.get(FOLDER);
	return state?.state === 'loaded' ? state.entries.map((entry) => entry.name) : [];
}

/** The batch folder's page `address`, a last page holding `entries`. */
function batchPage(address: DenHref, entries: readonly DenEntry[]): DenPageData {
	return {
		kind: 'denPage',
		address,
		outcome: {
			outcome: 'page',
			page: {
				view: 'open',
				node: { ...fileEntry('batch'), href: FOLDER, kind: 'directory' },
				crumbs: [{ href: denHref('/den'), name: denName('Alice') }],
				body: { body: 'listing', entries, more: undefined }
			},
			flagLinks: { on: FOLDER, off: FOLDER }
		},
		rootHref: denHref('/den'),
		includeDeleted: false,
		continued: true,
		ancestors: [],
		title: 'My Den · Zurfur',
		trail: []
	};
}

/**
 * A preload over a working network: the den page's load answers for the
 * address asked, minus any parameter outside its allowlist (as the server
 * reads it), with a last page holding `entries`.
 */
function answering(entries: readonly DenEntry[], asked: string[] = []): Preload {
	return (href: string) => {
		asked.push(href);
		const url = new URL(href, 'http://den.invalid');
		url.searchParams.delete('retry');
		const address = denHref(`${url.pathname}${url.search}`);
		return Promise.resolve({
			type: 'loaded',
			status: 200,
			data: { den: batchPage(address, entries) }
		});
	};
}

/**
 * A preload whose fetch failed (offline, a reset connection, a 502): as
 * SvelteKit does, it answers with the current page, here the folder's first.
 */
function offline(): Preload {
	const current = batchPage(FOLDER, [fileEntry('first')]);
	return () => Promise.resolve({ type: 'loaded', status: 200, data: { den: current } });
}
