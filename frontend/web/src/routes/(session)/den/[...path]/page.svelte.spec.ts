import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { DenEntry, DenPageData, OpenEntry } from '$lib/api/den';
import { denHref, denName, denType } from '$lib/types/brand';

/** What the mocked SvelteKit runtime holds for this spec. */
interface Kit {
	afterNavigate: ((navigation: { type: string }) => void)[];
	navigatingTo: { route: { id: string } } | undefined;
	preloaded: unknown;
	/** When set, the preload waits for this before answering. */
	held: Promise<void> | undefined;
}
const kit = vi.hoisted((): Kit => ({
	afterNavigate: [],
	navigatingTo: undefined,
	preloaded: undefined,
	held: undefined
}));

vi.mock('$app/navigation', () => ({
	afterNavigate: (callback: (navigation: { type: string }) => void) => {
		kit.afterNavigate.push(callback);
	},
	preloadData: async () => {
		const answer = kit.preloaded;
		await kit.held;
		return { type: 'loaded', status: 200, data: { den: answer } };
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
	return render(DenPage, { data: { den, trail: den.trail } } as never);
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

afterEach(() => {
	kit.afterNavigate.length = 0;
	kit.held = undefined;
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
