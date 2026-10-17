import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { DenEntry, DenPageData, OpenEntry } from '$lib/api/den';
import { TREE_MEMORY_KEY, TreeMemory } from '$lib/components/explorer/tree-memory.svelte';
import { denHref, denName, denType, did } from '$lib/types/brand';

/** What the mocked SvelteKit runtime holds for this spec. */
interface Kit {
	afterNavigate: ((navigation: { type: string }) => void)[];
	navigatingTo: { route: { id: string } } | undefined;
	preloaded: DenPageData | undefined;
	/** When set, the preload waits for this before answering. */
	held: Promise<void> | undefined;
	/** When set, the preload's fetch fails, and it answers with the current page as SvelteKit does. */
	offline: boolean;
	/** The page rendered now. */
	current: unknown;
	/** Every address the preload was asked for. */
	asked: string[];
}
const kit = vi.hoisted((): Kit => ({
	afterNavigate: [],
	navigatingTo: undefined,
	preloaded: undefined,
	held: undefined,
	offline: false,
	current: undefined,
	asked: []
}));

vi.mock('$app/navigation', async (importOriginal) => ({
	// The rest of the module stays real: the "New commission" form's superForm imports it.
	...(await importOriginal<typeof import('$app/navigation')>()),
	afterNavigate: (callback: (navigation: { type: string }) => void) => {
		kit.afterNavigate.push(callback);
	},
	preloadData: async (href: string) => {
		kit.asked.push(href);
		const answer = kit.preloaded;
		await kit.held;
		if (kit.offline) return { type: 'loaded', status: 200, data: { den: kit.current } };
		// The den page's load answers for the address asked, reading only its allowlisted parameters.
		const url = new URL(href, 'http://den.invalid');
		url.searchParams.delete('retry');
		const address = `${url.pathname}${url.search}`;
		return {
			type: 'loaded',
			status: 200,
			data: { den: answer === undefined ? undefined : { ...answer, address } }
		};
	}
}));

vi.mock('$app/state', () => ({
	navigating: {
		get to() {
			return kit.navigatingTo;
		}
	}
}));

const { default: DenPage } = await import('./+page.svelte');

/** An open entry at `href`. */
function open(href: string, name: string, extra: Partial<OpenEntry> = {}): OpenEntry {
	return {
		view: 'open',
		href: denHref(href),
		name: denName(name),
		type: denType('folder'),
		kind: 'file',
		mounted: false,
		ownLevel: 'private',
		contentNotShown: false,
		removed: undefined,
		...extra
	};
}

/** The den page's data for a folder listing `entries`, with `more` behind them. */
function folderData(entries: readonly DenEntry[], more?: string): DenPageData {
	const node = open('/den/batch', 'Big ref batch', { kind: 'directory' });
	return {
		kind: 'denPage',
		address: denHref('/den/batch'),
		outcome: {
			outcome: 'page',
			page: {
				view: 'open',
				node,
				crumbs: [{ href: denHref('/den'), name: denName('Alice') }],
				body: { body: 'listing', entries, more: more === undefined ? undefined : denHref(more) }
			},
			flagLinks: { on: node.href, off: node.href }
		},
		rootHref: denHref('/den'),
		includeDeleted: false,
		continued: false,
		ancestors: [],
		title: 'My Den · Zurfur',
		trail: []
	};
}

function renderPage(den: DenPageData) {
	kit.current = den;
	return render(DenPage, { data: { den, trail: den.trail } } as never);
}

/** The page rendered inside a frame whose tree memory is `memory`. */
function renderInFrame(den: DenPageData, memory: TreeMemory) {
	kit.current = den;
	return render(DenPage, {
		props: { data: { den, trail: den.trail } },
		context: new Map([[TREE_MEMORY_KEY, memory]])
	} as never);
}

/** The names the tree memory lists under `folder`. */
function treeNames(memory: TreeMemory, folder: string): string[] {
	const state = memory.folders.get(denHref(folder));
	if (state?.state !== 'loaded') return [];
	return state.entries.map((entry) => entry.name);
}

/** The den page's data for another folder, as after a navigation. */
function otherFolderData(): DenPageData {
	const data = folderData([open('/den/other/x', 'x.png')]);
	const { outcome } = data;
	if (outcome.outcome !== 'page' || outcome.page.view !== 'open') throw new Error('fixture');
	const node = open('/den/other', 'Other folder', { kind: 'directory' });
	return { ...data, outcome: { ...outcome, page: { ...outcome.page, node } } };
}

/** Fire the layout-level navigation callbacks as SvelteKit would. */
function navigated(type: string): void {
	for (const callback of kit.afterNavigate) callback({ type });
}

/** A button outside the pane, standing in for the tree, removed after each test. */
let outside: HTMLButtonElement | undefined;

/** Hold the preload's answers until the returned function is called. */
function holdPreloads(): () => void {
	let release: () => void = () => undefined;
	kit.held = new Promise((done) => {
		release = done;
	});
	return () => {
		release();
	};
}

afterEach(() => {
	kit.afterNavigate.length = 0;
	kit.held = undefined;
	kit.offline = false;
	kit.asked = [];
	outside?.remove();
	outside = undefined;
	kit.navigatingTo = undefined;
	vi.useRealTimers();
});

describe('den page', () => {
	it('moves focus to the pane’s heading after opening an item from outside the tree', async () => {
		renderPage(folderData([open('/den/batch/a', 'a.png')]));
		await expect.element(page.getByRole('heading', { name: 'Big ref batch' })).toBeInTheDocument();
		const active = document.activeElement;
		if (active instanceof HTMLElement) active.blur();

		navigated('link');

		await expect.element(page.getByRole('heading', { name: 'Big ref batch' })).toHaveFocus();
	});

	it('announces the opened item’s name, though the title stays generic', async () => {
		renderPage(folderData([]));
		navigated('link');
		await expect.element(page.getByTestId('den-announcer')).toHaveTextContent('Big ref batch');
		expect(document.title).not.toContain('Big ref batch');
	});

	it('announces a second item with the same name as the first', async () => {
		renderPage(folderData([]));
		navigated('link');
		const announcer = page.getByTestId('den-announcer');
		await expect.element(announcer).toHaveTextContent('Big ref batch');
		const spokenTexts: string[] = [];
		const observer = new MutationObserver(() => {
			spokenTexts.push(announcer.element().textContent);
		});
		observer.observe(announcer.element(), {
			childList: true,
			characterData: true,
			subtree: true
		});

		navigated('link');

		await expect.poll(() => spokenTexts).toEqual(['', 'Big ref batch']);
		observer.disconnect();
	});

	it('never says the name of an item left before its announcement was due', async () => {
		const screen = renderPage(folderData([]));
		const announcer = page.getByTestId('den-announcer');
		const spokenTexts: string[] = [];
		const observer = new MutationObserver(() => {
			spokenTexts.push(announcer.element().textContent);
		});
		observer.observe(announcer.element(), {
			childList: true,
			characterData: true,
			subtree: true
		});

		navigated('link');
		const next = otherFolderData();
		await screen.rerender({ data: { den: next, trail: next.trail } } as never);
		navigated('link');

		await expect.poll(() => spokenTexts.at(-1)).toBe('Other folder');
		await new Promise((settle) => setTimeout(settle, 200));
		expect(spokenTexts).not.toContain('Big ref batch');
		observer.disconnect();
	});

	it('shows busy only after the delay while another Den item loads', async () => {
		vi.useFakeTimers();
		kit.navigatingTo = { route: { id: '/(session)/den/[...path]' } };
		renderPage(folderData([open('/den/batch/a', 'a.png')]));
		const pane = page.getByTestId('den-pane');

		await vi.advanceTimersByTimeAsync(250);
		expect(pane.element().getAttribute('aria-busy')).toBeNull();
		await vi.advanceTimersByTimeAsync(100);
		expect(pane.element().getAttribute('aria-busy')).toBe('true');
	});

	it('appends the next page from "More" and moves focus to its first new row', async () => {
		kit.preloaded = folderData([open('/den/batch/c', 'c.png'), open('/den/batch/d', 'd.png')]);
		renderPage(
			folderData(
				[open('/den/batch/a', 'a.png'), open('/den/batch/b', 'b.png')],
				'/den/batch?pageToken=p1.2'
			)
		);

		await page.getByTestId('entry-more').click();

		await expect.element(page.getByRole('link', { name: 'c.png' })).toHaveFocus();
		await expect.element(page.getByTestId('entry-more')).not.toBeInTheDocument();
	});

	it('extends the tree’s listing of the folder with the page "More" brought', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		const firstPage = [open('/den/batch/a', 'a.png'), open('/den/batch/b', 'b.png')];
		memory.loaded(denHref('/den/batch'), firstPage, denHref('/den/batch?pageToken=p1.2'));
		kit.preloaded = folderData([open('/den/batch/c', 'c.png')]);
		renderInFrame(folderData(firstPage, '/den/batch?pageToken=p1.2'), memory);

		await page.getByTestId('entry-more').click();

		await expect.element(page.getByRole('link', { name: 'c.png' })).toHaveFocus();
		expect(treeNames(memory, '/den/batch')).toEqual(['a.png', 'b.png', 'c.png']);
		expect(memory.folders.get(denHref('/den/batch'))).toMatchObject({ more: undefined });
	});

	it('never appends to the tree a page the tree’s own "More…" already appended', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		const firstPage = [open('/den/batch/a', 'a.png'), open('/den/batch/b', 'b.png')];
		const c = open('/den/batch/c', 'c.png');
		memory.loaded(denHref('/den/batch'), [...firstPage, c], undefined);
		kit.preloaded = folderData([c]);
		renderInFrame(folderData(firstPage, '/den/batch?pageToken=p1.2'), memory);

		await page.getByTestId('entry-more').click();

		await expect.element(page.getByRole('link', { name: 'c.png' })).toHaveFocus();
		expect(treeNames(memory, '/den/batch')).toEqual(['a.png', 'b.png', 'c.png']);
	});

	it('shows "Couldn’t load more. Retry." when "More" fails, never appending the current page again', async () => {
		kit.offline = true;
		renderPage(folderData([open('/den/batch/a', 'a.png')], '/den/batch?pageToken=p1.1'));

		await page.getByTestId('entry-more').click();

		await expect
			.element(page.getByTestId('entry-more'))
			.toHaveTextContent("Couldn't load more. Retry.");
		expect(page.getByRole('link', { name: 'a.png' }).elements()).toHaveLength(1);
	});

	it('retries "More" through a fresh preload, so SvelteKit’s cached failure isn’t replayed', async () => {
		kit.offline = true;
		kit.preloaded = folderData([open('/den/batch/c', 'c.png')]);
		renderPage(folderData([open('/den/batch/a', 'a.png')], '/den/batch?pageToken=p1.1'));
		await page.getByTestId('entry-more').click();
		await expect
			.element(page.getByTestId('entry-more'))
			.toHaveTextContent("Couldn't load more. Retry.");

		kit.offline = false;
		await page.getByTestId('entry-more').click();

		await expect.element(page.getByRole('link', { name: 'c.png' })).toHaveFocus();
		expect(kit.asked[0]).toBe('/den/batch?pageToken=p1.1');
		expect(kit.asked[1]).not.toBe(kit.asked[0]);
		expect(kit.asked[1]?.startsWith('/den/batch?pageToken=p1.1&')).toBe(true);
	});

	it('leaves focus in the tree when "More" lands after focus moved there', async () => {
		kit.preloaded = folderData([open('/den/batch/c', 'c.png')]);
		const release = holdPreloads();
		renderPage(folderData([open('/den/batch/a', 'a.png')], '/den/batch?pageToken=p1.1'));
		await page.getByTestId('entry-more').click();
		await expect.poll(() => kit.asked).toHaveLength(1);

		outside = document.createElement('button');
		outside.textContent = 'In the tree';
		document.body.append(outside);
		outside.focus();
		release();

		await expect.element(page.getByRole('link', { name: 'c.png' })).toBeInTheDocument();
		await new Promise((settle) => setTimeout(settle, 50));
		expect(document.activeElement).toBe(outside);
	});

	it('drops a "More" that answers after the page changed, never appending to the new one', async () => {
		let release: () => void = () => undefined;
		kit.held = new Promise((done) => {
			release = done;
		});
		kit.preloaded = folderData([open('/den/batch/c', 'c.png')]);
		const screen = renderPage(
			folderData([open('/den/batch/a', 'a.png')], '/den/batch?pageToken=p1.1')
		);

		await page.getByTestId('entry-more').click();
		const next = otherFolderData();
		await screen.rerender({ data: { den: next, trail: next.trail } } as never);
		navigated('link');
		release();

		await expect.element(page.getByRole('heading', { name: 'Other folder' })).toBeInTheDocument();
		await new Promise((settle) => setTimeout(settle, 50));
		await expect.element(page.getByRole('link', { name: 'c.png' })).not.toBeInTheDocument();
	});
});
