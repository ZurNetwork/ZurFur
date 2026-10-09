import { describe, expect, it } from 'vitest';
import type { CardEntry, DenEntry, OpenEntry } from '$lib/api/den';
import { denHref, denName, denType, type DenHref } from '$lib/types/brand';
import type { FolderState } from './tree-memory.svelte';
import { treeModel, visibleKeys, type TreeCurrent, type TreeNode } from './tree-model';

const ROOT = denHref('/den');
const COMMISSIONS = denHref('/den/commissions');

/** An open entry at `href`. */
function open(href: string, name: string, extra: Partial<OpenEntry> = {}): OpenEntry {
	return {
		view: 'open',
		href: denHref(href),
		name: denName(name),
		type: denType('folder'),
		kind: 'directory',
		mounted: false,
		ownLevel: 'private',
		contentNotShown: false,
		removed: undefined,
		...extra
	};
}

/** A card named `name`. */
function card(name: string): CardEntry {
	return { view: 'card', name: denName(name), type: denType('commission'), mounted: true };
}

/** A loaded folder state. */
function loaded(entries: readonly DenEntry[], more?: string): FolderState {
	return {
		state: 'loaded',
		entries,
		more: more === undefined ? undefined : denHref(more),
		moreState: 'idle'
	};
}

const NOTHING_OPEN: TreeCurrent = { href: undefined, parentHref: undefined, entry: undefined };

/** The tree for these folders, open folders and open item. */
function model(
	folders: ReadonlyMap<DenHref, FolderState>,
	expanded: readonly DenHref[],
	current: TreeCurrent = NOTHING_OPEN
): TreeNode {
	return treeModel({ rootHref: ROOT, folders, expanded: new Set(expanded), current });
}

/** The child rows under a node, or a test failure. */
function childrenOf(node: TreeNode | undefined): readonly TreeNode[] {
	if (node === undefined || (node.item !== 'root' && node.item !== 'folder'))
		throw new Error('not a folder');
	if (node.children.state !== 'shown') throw new Error(`children ${node.children.state}`);
	return node.children.nodes;
}

describe('treeModel', () => {
	it('shows folders first, then the rest, each in the server’s order', () => {
		const entries = [
			open('/den/b-file', 'b-file', { kind: 'file' }),
			open('/den/z-folder', 'z-folder'),
			card('a-card'),
			open('/den/a-folder', 'a-folder')
		];
		const rows = childrenOf(model(new Map([[ROOT, loaded(entries)]]), [ROOT]));

		expect(rows.map((row) => row.item)).toEqual(['folder', 'folder', 'leaf', 'card']);
		expect(rows.map((row) => ('entry' in row ? row.entry.name : row.key))).toEqual([
			'z-folder',
			'a-folder',
			'b-file',
			'a-card'
		]);
	});

	it('shows a closed root with no rows under it', () => {
		const root = model(new Map([[ROOT, loaded([open('/den/a', 'a')])]]), []);
		expect(root).toMatchObject({ item: 'root', expanded: false, children: { state: 'closed' } });
	});

	it('marks an open folder whose listing is still coming as loading', () => {
		const root = model(new Map(), [ROOT]);
		expect(root).toMatchObject({ children: { state: 'loading' } });
	});

	it('shows "Empty" in an empty open folder, and a retry row in a failed one', () => {
		const empty = childrenOf(model(new Map([[ROOT, loaded([])]]), [ROOT]));
		const failed = childrenOf(model(new Map([[ROOT, { state: 'failed' }]]), [ROOT]));
		expect(empty.map((row) => row.item)).toEqual(['empty']);
		expect(failed.map((row) => row.item)).toEqual(['failed']);
	});

	it('gives a "content not shown yet" folder no arrow: it is a leaf with a hint', () => {
		const posts = open('/den/posts', 'posts', { contentNotShown: true, mounted: true });
		const [row] = childrenOf(model(new Map([[ROOT, loaded([posts])]]), [ROOT]));
		expect(row).toMatchObject({ item: 'leaf', notShown: true });
	});

	it('positions every row of a partly loaded folder, the More row included, and no count', () => {
		const partial = childrenOf(
			model(
				new Map([
					[ROOT, loaded([open('/den/a', 'a'), open('/den/b', 'b')], '/den?pageToken=p1.2')]
				]),
				[ROOT]
			)
		);
		const whole = childrenOf(model(new Map([[ROOT, loaded([open('/den/a', 'a')])]]), [ROOT]));

		expect(partial.map((row) => ('position' in row ? row.position : row.item))).toEqual([
			{ level: 2, posinset: 1 },
			{ level: 2, posinset: 2 },
			{ level: 2, posinset: 3 }
		]);
		expect(partial.map((row) => row.item)).toEqual(['folder', 'folder', 'more']);
		expect(whole.map((row) => ('position' in row ? row.position : undefined))).toEqual([undefined]);
	});

	it('marks the open item current and keeps it closed while its parents open down to it', () => {
		const untitled = open('/den/commissions/c1', 'Untitled', { mounted: true });
		const folders = new Map([
			[ROOT, loaded([open('/den/commissions', 'commissions')])],
			[COMMISSIONS, loaded([untitled])]
		]);
		const current: TreeCurrent = { href: untitled.href, parentHref: COMMISSIONS, entry: untitled };
		const [commissions] = childrenOf(model(folders, [ROOT, COMMISSIONS], current));
		const [row] = childrenOf(commissions);

		expect(row).toMatchObject({ item: 'folder', current: true, expanded: false });
	});

	it('shows the open item after its folder’s More row when it lies beyond the loaded pages', () => {
		const beyond = open('/den/commissions/c99', 'Far away');
		const folders = new Map([
			[ROOT, loaded([open('/den/commissions', 'commissions')])],
			[COMMISSIONS, loaded([open('/den/commissions/c1', 'One')], '/den/commissions?pageToken=p1.1')]
		]);
		const current: TreeCurrent = { href: beyond.href, parentHref: COMMISSIONS, entry: beyond };
		const [commissions] = childrenOf(model(folders, [ROOT, COMMISSIONS], current));
		const rows = childrenOf(commissions);

		expect(rows.map((row) => row.item)).toEqual(['folder', 'more', 'folder']);
		expect(rows[2]).toMatchObject({
			current: true,
			entry: { name: 'Far away' },
			position: { level: 3, posinset: 3 }
		});
	});

	it('never adds a card page as a stray row, so "More…" can’t list it twice', () => {
		const someCard = card('Some commission');
		const folders = new Map([
			[ROOT, loaded([open('/den/commissions', 'commissions')])],
			[
				COMMISSIONS,
				loaded([open('/den/commissions/c1', 'One'), someCard], '/den/commissions?pageToken=p1.2')
			]
		]);
		const current: TreeCurrent = { href: undefined, parentHref: COMMISSIONS, entry: someCard };
		const [commissions] = childrenOf(model(folders, [ROOT, COMMISSIONS], current));
		const cards = childrenOf(commissions).filter((row) => row.item === 'card');

		expect(cards).toHaveLength(1);
	});

	it('lists the visible keys in order, for the roving focus', () => {
		const folders = new Map([[ROOT, loaded([open('/den/a', 'a'), card('c')])]]);
		expect(visibleKeys(model(folders, [ROOT]))).toEqual(['/den', '/den/a', '/den#card-1']);
	});
});
