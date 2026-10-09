import { describe, expect, it } from 'vitest';
import type { DenPageData, OpenEntry } from '$lib/api/den';
import { denHref, denName, denType, did } from '$lib/types/brand';
import { TreeMemory } from './tree-memory.svelte';

const ALICE = did('did:plc:alice');

/** An open folder entry at `href`. */
function folder(href: string, name: string): OpenEntry {
	return {
		view: 'open',
		href: denHref(href),
		name: denName(name),
		type: denType('folder'),
		kind: 'directory',
		mounted: false,
		ownLevel: 'private',
		contentNotShown: false,
		removed: undefined
	};
}

/** A Den page of the folder `node` under `crumbs`, listing `entries`. */
function denPage(
	node: OpenEntry,
	crumbs: readonly OpenEntry[],
	entries: readonly OpenEntry[],
	options: { includeDeleted?: boolean; continued?: boolean } = {}
): DenPageData {
	const includeDeleted = options.includeDeleted ?? false;
	return {
		kind: 'denPage',
		outcome: {
			outcome: 'page',
			page: {
				view: 'open',
				node,
				crumbs: crumbs.map((crumb) => ({ href: crumb.href, name: crumb.name })),
				body: { body: 'listing', entries, more: undefined }
			},
			flagLinks: { on: denHref(`${node.href}?includeDeleted=true`), off: node.href }
		},
		rootHref: denHref(includeDeleted ? '/den?includeDeleted=true' : '/den'),
		includeDeleted,
		continued: options.continued ?? false,
		ancestors: [],
		title: 'x',
		trail: []
	};
}

const root = folder('/den', 'Alice');
const commissions = folder('/den/commissions', 'commissions');
const untitled = folder('/den/commissions/c1', 'Untitled');

describe('TreeMemory', () => {
	it('keeps the open folder’s listing and opens its crumbs, leaving the folder itself closed', () => {
		const memory = new TreeMemory(ALICE);
		memory.absorb(denPage(commissions, [root], [untitled]));

		expect(memory.folders.get(commissions.href)).toMatchObject({
			state: 'loaded',
			entries: [untitled]
		});
		expect(memory.expanded.has(root.href)).toBe(true);
		expect(memory.expanded.has(commissions.href)).toBe(false);
	});

	it('forgets everything when "Show archived" flips', () => {
		const memory = new TreeMemory(ALICE);
		memory.absorb(denPage(commissions, [root], [untitled]));
		memory.absorb(
			denPage(folder('/den/x?includeDeleted=true', 'x'), [], [], { includeDeleted: true })
		);

		expect(memory.folders.has(commissions.href)).toBe(false);
		expect(memory.includeDeleted).toBe(true);
	});

	it('does not take a later page as the folder’s first', () => {
		const memory = new TreeMemory(ALICE);
		memory.absorb(denPage(commissions, [root], [untitled], { continued: true }));
		expect(memory.folders.has(commissions.href)).toBe(false);
	});

	it('starts over for another visitor', () => {
		const memory = new TreeMemory(ALICE);
		memory.absorb(denPage(commissions, [root], [untitled]));
		memory.ensureOwner(did('did:plc:bob'));
		expect(memory.folders.size).toBe(0);
		expect(memory.rootHref).toBeUndefined();
	});

	it('names the crumbs whose listings it lacks', () => {
		const memory = new TreeMemory(ALICE);
		const page = denPage(untitled, [root, commissions], []);
		memory.absorb(page);
		expect(memory.missingCrumbs(page)).toEqual([root.href, commissions.href]);
	});

	it('appends a later page to a loaded folder', () => {
		const memory = new TreeMemory(ALICE);
		memory.loaded(commissions.href, [untitled], denHref('/den/commissions?pageToken=p1.1'));
		const second = folder('/den/commissions/c2', 'Second');
		memory.appended(
			commissions.href,
			[second],
			undefined,
			denHref('/den/commissions?pageToken=p1.1')
		);
		expect(memory.folders.get(commissions.href)).toMatchObject({
			entries: [untitled, second],
			more: undefined
		});
	});

	it('starts over on a restart (a form action’s success) but keeps ~ open, fetching it again', () => {
		const memory = new TreeMemory(ALICE);
		memory.absorb(denPage(commissions, [root], [untitled]));
		memory.loaded(root.href, [commissions], undefined);

		memory.restart(denHref('/den'));
		const toFetch = memory.navigated(ALICE, undefined);

		expect(memory.folders.has(commissions.href)).toBe(false);
		expect(memory.expanded.has(root.href)).toBe(true);
		expect(toFetch).toEqual([root.href]);
	});

	it('restarts on the bare root when no Den page was seen yet', () => {
		const memory = new TreeMemory(ALICE);
		memory.restart(denHref('/den'));
		expect(memory.navigated(ALICE, undefined)).toEqual([denHref('/den')]);
	});

	it('keeps what it knows across a navigation, even an in-place reload', () => {
		const memory = new TreeMemory(ALICE);
		memory.absorb(denPage(commissions, [root], [untitled]));
		memory.loaded(denHref('/den/accounts'), [], undefined);
		memory.navigated(ALICE, undefined);
		expect(memory.folders.has(denHref('/den/accounts'))).toBe(true);
	});

	it('fetches ~ when a page opened it but brought no listing for it (a refused address)', () => {
		const memory = new TreeMemory(ALICE);
		const refused: DenPageData = { ...denPage(root, [], []), outcome: { outcome: 'notFound' } };
		expect(memory.navigated(ALICE, refused)).toEqual([root.href]);
	});

	it('bumps its generation on every clear, so a late fetch can tell', () => {
		const memory = new TreeMemory(ALICE);
		const before = memory.generation;
		memory.restart(denHref('/den'));
		expect(memory.generation).toBe(before + 1);
	});
});
