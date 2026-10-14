import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import type { DenPageData } from '$lib/api/den';
import { denHref, did, handleFromTrusted, type DenHref, type Did } from '$lib/types/brand';

/** The callbacks the layout registered with `afterNavigate`, to fire as a navigation would. */
interface Nav {
	callbacks: (() => void)[];
	preloaded: string[];
	den: unknown;
}
const nav = vi.hoisted((): Nav => ({ callbacks: [], preloaded: [], den: undefined }));

vi.mock('$app/navigation', () => ({
	afterNavigate: (callback: (navigation: { type: string }) => void) => {
		nav.callbacks.push(() => {
			callback({ type: 'link' });
		});
	},
	goto: () => Promise.resolve(),
	preloadData: (href: string) => {
		nav.preloaded.push(href);
		return Promise.resolve({ type: 'loaded', status: 500, data: {} });
	}
}));

vi.mock('$app/state', () => ({
	page: {
		get data() {
			return {
				trail: [
					{ step: 'named', label: 'Accounts', href: '/accounts' },
					{ step: 'named', label: 'alice-studio.zurfur.app', href: undefined }
				],
				den: nav.den
			};
		},
		route: { id: '/(session)/accounts/[id]' },
		status: 200
	}
}));

/** Every tree memory the layout built, with the owner it was built for and each owner it was told on a navigation. */
interface Built {
	memory: { restart: (fallbackRoot: DenHref) => void };
	owner: Did;
	navigatedAs: Did[];
}
const trees = vi.hoisted((): { built: Built[] } => ({ built: [] }));

vi.mock('$lib/components/explorer/tree-memory.svelte', async (importActual) => {
	const actual = await importActual<typeof import('$lib/components/explorer/tree-memory.svelte')>();
	class RecordedTreeMemory extends actual.TreeMemory {
		readonly #record: Built;
		constructor(owner: Did) {
			super(owner);
			this.#record = { memory: this, owner, navigatedAs: [] };
			trees.built.push(this.#record);
		}
		override navigated(owner: Did, data: DenPageData | undefined): readonly DenHref[] {
			this.#record.navigatedAs.push(owner);
			return super.navigated(owner, data);
		}
	}
	return { ...actual, TreeMemory: RecordedTreeMemory };
});

const { default: Layout } = await import('./+layout.svelte');

const session = {
	did: did('did:plc:alice'),
	handle: handleFromTrusted('alice.zurfur.app'),
	displayName: 'Alice',
	avatarUrl: undefined
};

const children = createRawSnippet(() => ({ render: () => '<h1>Pane heading</h1>' }));

function renderLayout() {
	return render(Layout, { data: { session }, children });
}

afterEach(() => {
	nav.callbacks.length = 0;
	nav.preloaded.length = 0;
	nav.den = undefined;
	trees.built.length = 0;
});

describe('(session) layout: the frame around every signed-in page', () => {
	it('puts the page inside the frame: top bar, main pane, the page’s own path', async () => {
		await page.viewport(1280, 800);
		renderLayout();
		await expect.element(page.getByRole('banner')).toBeInTheDocument();
		await expect
			.element(page.getByRole('main').getByRole('heading', { name: 'Pane heading' }))
			.toBeInTheDocument();
		await expect
			.element(page.getByTestId('path-bar').getByRole('link', { name: 'Accounts' }))
			.toBeInTheDocument();
	});

	it('marks Accounts as the current section and keeps Sign out in the top bar', async () => {
		await page.viewport(1280, 800);
		renderLayout();
		await expect.element(page.getByTestId('accounts-link')).toHaveAttribute('aria-current', 'page');
		await expect
			.element(page.getByRole('banner').getByRole('button', { name: 'Sign out' }))
			.toBeVisible();
	});

	it('puts "Skip to content" first in the Tab order, and it moves focus to the pane', async () => {
		await page.viewport(1280, 800);
		renderLayout();
		const skip = page.getByTestId('skip-link');
		await expect.element(skip).toBeInTheDocument();

		const focusable = document.querySelectorAll<HTMLElement>(
			'a[href], button, input, select, textarea, [tabindex]:not([tabindex="-1"])'
		);
		expect(focusable[0]).toBe(skip.element());
		focusable[0]?.focus();
		await expect.element(skip).toBeVisible();
		await userEvent.keyboard('{Enter}');
		await expect.element(page.getByRole('main')).toHaveFocus();
	});

	it('closes the drawer after a navigation', async () => {
		await page.viewport(320, 640);
		renderLayout();
		await page.getByTestId('open-drawer').click();
		await expect.element(page.getByTestId('sidebar-drawer')).toBeInTheDocument();

		for (const callback of nav.callbacks) callback();

		await expect.element(page.getByTestId('sidebar-drawer')).not.toBeInTheDocument();
	});

	it('shows Accounts once in the drawer, in its footer', async () => {
		await page.viewport(320, 640);
		renderLayout();
		await page.getByTestId('open-drawer').click();
		await expect.element(page.getByTestId('sidebar-drawer')).toBeInTheDocument();

		const drawer = page.getByRole('dialog').element();
		const accountsLinks = [...drawer.querySelectorAll('a')].filter(
			(link) => link.textContent.trim() === 'Accounts'
		);
		expect(accountsLinks).toHaveLength(1);
	});

	it('shows Sign out in the drawer at 320 px', async () => {
		await page.viewport(320, 640);
		renderLayout();
		await page.getByTestId('open-drawer').click();

		await expect
			.element(page.getByRole('dialog').getByRole('button', { name: 'Sign out' }))
			.toBeVisible();
	});

	it('never scrolls sideways at 320 px', async () => {
		await page.viewport(320, 640);
		renderLayout();
		await expect.element(page.getByRole('banner')).toBeInTheDocument();
		expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(window.innerWidth);
	});

	it('fetches every folder above the open item that the tree lacks, after an in-app navigation', async () => {
		const crumb = (href: string, name: string) => ({ href, name });
		nav.den = {
			kind: 'denPage',
			address: '/den/commissions/c9',
			outcome: {
				outcome: 'page',
				page: {
					view: 'card',
					node: { view: 'card', name: 'Deep', type: 'x', mounted: false },
					crumbs: [crumb('/den', 'Alice'), crumb('/den/commissions', 'commissions')]
				},
				flagLinks: { on: '/den/commissions/c9', off: '/den/commissions/c9' }
			},
			rootHref: '/den',
			includeDeleted: false,
			continued: false,
			ancestors: [],
			title: 'My Den · Zurfur',
			trail: []
		};
		renderLayout();
		await expect.element(page.getByRole('banner')).toBeInTheDocument();

		for (const callback of nav.callbacks) callback();

		await expect.poll(() => [...nav.preloaded].sort()).toEqual(['/den', '/den/commissions']);
	});

	it('fetches the folders the tree lacks after a navigation to a page with no Den data', async () => {
		nav.den = undefined;
		renderLayout();
		await expect.element(page.getByRole('banner')).toBeInTheDocument();

		trees.built[0]?.memory.restart(denHref('/den'));
		for (const callback of nav.callbacks) callback();

		await expect.poll(() => [...nav.preloaded]).toEqual(['/den']);
	});

	it('keys the tree memory by the signed-in visitor’s DID, on every navigation', async () => {
		const bob = {
			...session,
			did: did('did:plc:bob'),
			handle: handleFromTrusted('bob.zurfur.app')
		};
		const { rerender } = renderLayout();
		await expect.element(page.getByRole('banner')).toBeInTheDocument();

		for (const callback of nav.callbacks) callback();
		await rerender({ data: { session: bob }, children });
		for (const callback of nav.callbacks) callback();

		expect(trees.built.map((built) => built.owner)).toEqual([session.did]);
		expect(trees.built[0]?.navigatedAs).toEqual([session.did, bob.did]);
	});
});
