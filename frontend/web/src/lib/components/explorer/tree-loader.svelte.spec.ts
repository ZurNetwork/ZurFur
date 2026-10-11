import { describe, expect, it } from 'vitest';
import { denHref, did } from '$lib/types/brand';
import { fetchListing, loadFolder, loadMore, type Preload } from './tree-loader';
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
	it('drops a folder fetch that finishes after the memory started over', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		const { preload, release } = heldPreload();
		const pending = loadFolder(memory, ROOT, preload);

		memory.restart(ROOT);
		release();
		await pending;

		expect(memory.folders.has(ROOT)).toBe(false);
	});

	it('treats a redirect back into the Den (a stale token) as a failure, never a page load', async () => {
		const preload: Preload = () =>
			Promise.resolve({ type: 'redirect', location: '/den/commissions' });
		expect(await fetchListing(ROOT, preload)).toEqual({ fetched: 'failed' });
	});

	it('tree "More…" then the pane’s stale "More": the page is appended once', async () => {
		const memory = pagedFolder();
		await loadMore(memory, FOLDER, PAGE_TWO, answering([second]));
		memory.appended(FOLDER, [second], undefined, PAGE_TWO);

		expect(names(memory)).toEqual(['first', 'second']);
	});

	it('the pane’s "More" then the tree’s stale "More…": the page is appended once', async () => {
		const memory = pagedFolder();
		memory.appended(FOLDER, [second], undefined, PAGE_TWO);
		await loadMore(memory, FOLDER, PAGE_TWO, answering([second]));

		expect(names(memory)).toEqual(['first', 'second']);
	});
});

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

/** A preload answering with a last page holding `entries`. */
function answering(entries: readonly DenEntry[]): Preload {
	const den: DenPageData = {
		kind: 'denPage',
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
	return () => Promise.resolve({ type: 'loaded', status: 200, data: { den } });
}
