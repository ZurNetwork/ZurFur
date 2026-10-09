import '$lib/styles/tokens.css';
import { page, userEvent } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { DenEntry, DenPageData, OpenEntry } from '$lib/api/den';
import { denHref, denName, denType, did, type DenHref } from '$lib/types/brand';
import Tree from './Tree.svelte';
import type { Preload } from './tree-loader';
import { TreeMemory } from './tree-memory.svelte';
import type { TreeCurrent } from './tree-model';

const ROOT = denHref('/den');

/** An open entry at `href`. */
function entry(href: string, name: string, extra: Partial<OpenEntry> = {}): OpenEntry {
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

const accounts = entry('/den/accounts', 'accounts');
const characters = entry('/den/characters', 'characters');
const commissions = entry('/den/commissions', 'commissions');
const posts = entry('/den/posts', 'posts', { mounted: true, contentNotShown: true });
const notes = entry('/den/notes', 'notes', { kind: 'file' });
const untitled = entry('/den/commissions/c1', 'Untitled', { mounted: true });
const someCard: DenEntry = {
	view: 'card',
	name: denName('Some commission'),
	type: denType('commission'),
	mounted: true
};
const ember = entry('/den/characters/ember', 'Ember', { mounted: true });

/** What the fake preload answers for each link: a listing, or a failure. */
const listings = new Map<string, readonly DenEntry[] | 'fail'>();

/** Every link the fake preload was asked for. */
let preloaded: string[] = [];

/** A Den page answer listing `entries`, as the den page's load returns it. */
function pageWith(href: DenHref, entries: readonly DenEntry[], more?: DenHref): DenPageData {
	return {
		kind: 'denPage',
		outcome: {
			outcome: 'page',
			page: {
				view: 'open',
				node: entry(href, 'x'),
				crumbs: [],
				body: { body: 'listing', entries, more }
			},
			flagLinks: { on: href, off: href }
		},
		rootHref: ROOT,
		includeDeleted: false,
		continued: false,
		ancestors: [],
		title: 'x',
		trail: []
	};
}

/** SvelteKit's preloadData, faked over {@link listings}. */
const fakePreload: Preload = (href: string) => {
	preloaded.push(href);
	const listing = listings.get(href);
	if (listing === undefined || listing === 'fail') {
		return Promise.resolve({ type: 'loaded' as const, status: 500, data: {} });
	}
	return Promise.resolve({
		type: 'loaded' as const,
		status: 200,
		data: { den: pageWith(denHref(href), listing) }
	});
};

/**
 * Links followed during a test: a click that reaches the document unhandled
 * would follow its link, so it is recorded and stopped there, keeping the
 * test page in place.
 */
let followed: string[] = [];

function stopNavigation(event: MouseEvent): void {
	const link = event.target instanceof Element ? event.target.closest('a') : null;
	if (link === null || event.defaultPrevented) return;
	event.preventDefault();
	followed.push(link.getAttribute('href') ?? '');
}

beforeEach(() => {
	listings.clear();
	preloaded = [];
	followed = [];
	document.addEventListener('click', stopNavigation);
});

afterEach(() => {
	document.removeEventListener('click', stopNavigation);
});

/** A memory holding the root's listing (and, open, `commissions`), with `current` open. */
function renderTree(
	options: { current?: OpenEntry; commissionsOpen?: boolean; rootMore?: DenHref } = {}
) {
	const memory = new TreeMemory(did('did:plc:alice'));
	memory.loaded(ROOT, [accounts, characters, commissions, notes, posts], options.rootMore);
	memory.expanded.add(ROOT);
	memory.rootHref = ROOT;
	if (options.commissionsOpen === true) {
		memory.loaded(commissions.href, [untitled, someCard], undefined);
		memory.expanded.add(commissions.href);
	}
	const current: TreeCurrent =
		options.current === undefined
			? { href: undefined, parentHref: undefined, entry: undefined }
			: { href: options.current.href, parentHref: commissions.href, entry: options.current };
	render(Tree, { memory, current, fallbackRootHref: ROOT, preload: fakePreload });
	return memory;
}

/** The tree item named `name`. */
function item(name: string) {
	return page.getByRole('treeitem', { name, exact: false });
}

/** The tree items' tabindex values, by accessible text. */
function tabStops(): string[] {
	const tree = page.getByTestId('tree').element();
	return [...tree.querySelectorAll<HTMLElement>('[role="treeitem"][tabindex="0"]')].map(
		(row) => row.dataset.treeKey ?? ''
	);
}

describe('Tree: structure', () => {
	it('is a labelled tree of tree items once it mounts', async () => {
		renderTree();
		await expect.element(page.getByRole('tree', { name: 'My Den' })).toBeInTheDocument();
		await expect.element(item('commissions')).toHaveAttribute('aria-expanded', 'false');
		await expect.element(item('My Den')).toHaveAttribute('aria-expanded', 'true');
	});

	it('shows folders first, then the rest, each in the server’s order', async () => {
		renderTree();
		await expect.element(item('notes')).toBeInTheDocument();
		const keys = [
			...page.getByTestId('tree').element().querySelectorAll<HTMLElement>('[data-tree-key]')
		].map((row) => row.dataset.treeKey);
		expect(keys).toEqual([
			'/den',
			'/den/accounts',
			'/den/characters',
			'/den/commissions',
			'/den/posts',
			'/den/notes'
		]);
	});

	it('owns an open folder’s group, and marks the open item current', async () => {
		renderTree({ current: untitled, commissionsOpen: true });
		const folder = item('commissions').element();
		const groupId = folder.getAttribute('aria-owns') ?? '';
		expect(document.getElementById(groupId)?.getAttribute('role')).toBe('group');
		await expect.element(item('Untitled')).toHaveAttribute('aria-current', 'page');
	});

	it('marks the open item with a bar, not by its tint alone', async () => {
		renderTree({ current: untitled, commissionsOpen: true });
		await expect.element(item('Untitled')).toHaveAttribute('aria-current', 'page');
		expect(getComputedStyle(item('Untitled').element()).boxShadow).toContain('inset');
	});

	it('puts exactly one row in the Tab order: the open item', async () => {
		renderTree({ current: untitled, commissionsOpen: true });
		await expect.element(item('Untitled')).toBeInTheDocument();
		expect(tabStops()).toEqual(['/den/commissions/c1']);
	});

	it('puts the root in the Tab order when no Den item is open', async () => {
		renderTree();
		await expect.element(item('My Den')).toBeInTheDocument();
		expect(tabStops()).toEqual(['/den']);
	});

	it('shows a card as a tree item that is not a link and says it can’t be opened', () => {
		renderTree({ commissionsOpen: true });
		const cardRow = item('Some commission').element();
		expect(cardRow.tagName).toBe('SPAN');
		expect(cardRow.textContent).toContain("Can't be opened");
	});

	it('gives a "not shown yet" folder no arrow and a hint', () => {
		renderTree();
		const postsRow = item('posts').element();
		expect(postsRow.hasAttribute('aria-expanded')).toBe(false);
		expect(postsRow.textContent).toContain('not shown yet');
	});

	it('gives every row of a partly loaded folder its level and place, and an unknown set size', async () => {
		renderTree({ rootMore: denHref('/den?pageToken=p1.4') });
		await expect.element(item('commissions')).toHaveAttribute('aria-setsize', '-1');
		await expect.element(item('commissions')).toHaveAttribute('aria-posinset', '3');
		await expect.element(item('commissions')).toHaveAttribute('aria-level', '2');
	});

	it('keeps the tree links on focus and preloads them only on tap', () => {
		renderTree();
		const link = item('accounts').element();
		expect(link.hasAttribute('data-sveltekit-keepfocus')).toBe(true);
		expect(link.getAttribute('data-sveltekit-preload-data')).toBe('tap');
	});
});

describe('Tree: keyboard (W3C tree pattern)', () => {
	it('moves with Down and Up, and jumps with Home and End', async () => {
		renderTree();
		item('My Den').element().focus();

		await userEvent.keyboard('{ArrowDown}');
		await expect.element(item('accounts')).toHaveFocus();
		await userEvent.keyboard('{ArrowDown}');
		await expect.element(item('characters')).toHaveFocus();
		await userEvent.keyboard('{ArrowUp}');
		await expect.element(item('accounts')).toHaveFocus();
		await userEvent.keyboard('{End}');
		await expect.element(item('notes')).toHaveFocus();
		await userEvent.keyboard('{Home}');
		await expect.element(item('My Den')).toHaveFocus();
	});

	it('opens a closed folder with Right, fetching its listing, then steps into it', async () => {
		listings.set('/den/characters', [ember]);
		renderTree();
		item('characters').element().focus();

		await userEvent.keyboard('{ArrowRight}');
		await expect.element(item('characters')).toHaveAttribute('aria-expanded', 'true');
		await expect.element(item('Ember')).toBeInTheDocument();
		expect(preloaded).toEqual(['/den/characters']);

		await userEvent.keyboard('{ArrowRight}');
		await expect.element(item('Ember')).toHaveFocus();
	});

	it('steps out to the parent with Left, then closes the folder with Left', async () => {
		renderTree({ commissionsOpen: true });
		item('Untitled').element().focus();

		await userEvent.keyboard('{ArrowLeft}');
		await expect.element(item('commissions')).toHaveFocus();
		await userEvent.keyboard('{ArrowLeft}');
		await expect.element(item('commissions')).toHaveAttribute('aria-expanded', 'false');
		await expect.element(item('Untitled')).not.toBeInTheDocument();
	});

	it('jumps to the next name starting with a typed letter, wrapping round', async () => {
		renderTree();
		item('My Den').element().focus();

		await userEvent.keyboard('p');
		await expect.element(item('posts')).toHaveFocus();
		await userEvent.keyboard('c');
		await expect.element(item('characters')).toHaveFocus();
		await userEvent.keyboard('c');
		await expect.element(item('commissions')).toHaveFocus();
	});

	it('opens an item in the pane with Enter or Space', async () => {
		renderTree();
		item('accounts').element().focus();

		await userEvent.keyboard('{Enter}');
		await userEvent.keyboard(' ');

		expect(followed).toEqual(['/den/accounts', '/den/accounts']);
	});

	it('does nothing on Enter over a card', async () => {
		renderTree({ commissionsOpen: true });
		item('Some commission').element().focus();

		await userEvent.keyboard('{Enter}');

		expect(followed).toEqual([]);
		await expect.element(item('Some commission')).toHaveFocus();
	});

	it('moves the single Tab stop with focus, and back to the open item when focus leaves', async () => {
		renderTree({ current: untitled, commissionsOpen: true });
		item('Untitled').element().focus();

		await userEvent.keyboard('{ArrowUp}');
		expect(tabStops()).toEqual(['/den/commissions']);

		const focused = document.activeElement;
		if (focused instanceof HTMLElement) focused.blur();
		await expect.element(item('Untitled')).toHaveAttribute('tabindex', '0');
		expect(tabStops()).toEqual(['/den/commissions/c1']);
	});
});

describe('Tree: mouse', () => {
	it('opens a folder from its arrow without following its link', async () => {
		listings.set('/den/characters', [ember]);
		renderTree();

		const arrow = item('characters')
			.element()
			.querySelector<HTMLElement>('[data-testid="tree-arrow"]');
		arrow?.click();

		await expect.element(item('Ember')).toBeInTheDocument();
		expect(followed).toEqual([]);
	});

	it('opens a folder in the pane from its name, leaving it closed in the tree', async () => {
		renderTree();

		await item('characters').click();

		expect(followed).toEqual(['/den/characters']);
		await expect.element(item('characters')).toHaveAttribute('aria-expanded', 'false');
	});
});

describe('Tree: loading', () => {
	it('shows "Couldn’t load. Retry." when a folder fails, and retries on Enter', async () => {
		listings.set('/den/characters', 'fail');
		renderTree();
		item('characters').element().focus();
		await userEvent.keyboard('{ArrowRight}');

		const retry = item("Couldn't load. Retry.");
		await expect.element(retry).toBeInTheDocument();

		listings.set('/den/characters', [ember]);
		retry.element().focus();
		await userEvent.keyboard('{Enter}');
		await expect.element(item('Ember')).toBeInTheDocument();
	});

	it('gives the "More…" row its level and place too', async () => {
		renderTree({ rootMore: denHref('/den?pageToken=p1.4') });
		const more = item('More…');
		await expect.element(more).toHaveAttribute('aria-level', '2');
		await expect.element(more).toHaveAttribute('aria-posinset', '6');
		await expect.element(more).toHaveAttribute('aria-setsize', '-1');
	});

	it('moves focus to the first new row after the last "More…"', async () => {
		listings.set('/den?pageToken=p1.4', [entry('/den/zeta', 'zeta')]);
		renderTree({ rootMore: denHref('/den?pageToken=p1.4') });
		item('More…').element().focus();

		await userEvent.keyboard('{Enter}');

		await expect.element(item('zeta')).toHaveFocus();
	});

	it('appends the next page from "More…"', async () => {
		listings.set('/den?pageToken=p1.4', [entry('/den/zeta', 'zeta')]);
		renderTree({ rootMore: denHref('/den?pageToken=p1.4') });
		const more = item('More…');
		more.element().focus();

		await userEvent.keyboard('{Enter}');

		await expect.element(item('zeta')).toBeInTheDocument();
		await expect.element(item('More…')).not.toBeInTheDocument();
	});
});
