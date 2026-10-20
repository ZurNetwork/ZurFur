import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import type { DenPageData } from '$lib/api/den';
import type { TestStack } from '$lib/api/bug-report';
import { denHref, did, handleFromTrusted, type DenHref, type Did } from '$lib/types/brand';
import { UNKNOWN_BUILD_ID } from '$lib/types/bug-report-rules';

/** The callbacks the layout registered with `afterNavigate`, to fire as a navigation would. */
interface Nav {
	callbacks: (() => void)[];
	preloaded: string[];
	den: unknown;
	/** This test's preloads still answering, and the most there ever were at once. */
	load: { inFlight: number; most: number };
}
const nav = vi.hoisted((): Nav => ({
	callbacks: [],
	preloaded: [],
	den: undefined,
	load: { inFlight: 0, most: 0 }
}));

vi.mock('$app/navigation', () => ({
	afterNavigate: (callback: (navigation: { type: string }) => void) => {
		nav.callbacks.push(() => {
			callback({ type: 'link' });
		});
	},
	goto: () => Promise.resolve(),
	invalidateAll: () => Promise.resolve(),
	preloadData: async (href: string) => {
		nav.preloaded.push(href);
		const { load } = nav;
		load.inFlight += 1;
		load.most = Math.max(load.most, load.inFlight);
		await new Promise((settle) => setTimeout(settle, 1));
		load.inFlight -= 1;
		return { type: 'loaded', status: 500, data: {} };
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
		url: new URL('http://localhost/accounts/acct-1'),
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

function renderLayout(testStack: TestStack = 'none') {
	return render(Layout, { data: { session, buildId: UNKNOWN_BUILD_ID, testStack }, children });
}

afterEach(() => {
	nav.callbacks.length = 0;
	nav.preloaded.length = 0;
	nav.den = undefined;
	nav.load = { inFlight: 0, most: 0 };
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

	it('opens "Report a bug" from the top bar over the page, reporting it', async () => {
		await page.viewport(1280, 800);
		renderLayout();

		await page.getByTestId('report-bug-link').click();

		const dialog = page.getByRole('dialog', { name: 'Report a bug' });
		await expect.element(dialog).toBeVisible();
		await expect
			.element(dialog.getByRole('checkbox', { name: 'Page', exact: true }))
			.toHaveAccessibleDescription(
				'/accounts/acct-1 This address can contain the ids of your Accounts, characters and commissions.'
			);
		await expect
			.element(dialog.getByRole('checkbox', { name: 'Page type' }))
			.toHaveAccessibleDescription('An Account');
		await expect
			.element(
				dialog.getByText(
					'Sent as @alice.zurfur.app. Zurfur also records when it arrives, and its own version.'
				)
			)
			.toBeInTheDocument();
	});

	it('opens "Report a bug" from the drawer: the drawer shuts first, and focus goes back to ☰', async () => {
		await page.viewport(320, 640);
		renderLayout();
		const menu = page.getByTestId('open-drawer');
		await menu.click();
		const drawer = page.getByRole('dialog', { name: 'My Den' });
		const drawerLink = drawer.getByRole('link', { name: 'Report a bug' });
		await expect.element(drawerLink).toBeVisible();

		await drawerLink.click();

		const dialog = page.getByRole('dialog', { name: 'Report a bug' });
		await expect.element(dialog).toBeVisible();
		await expect.element(page.getByRole('textbox', { name: 'What happened?' })).toHaveFocus();
		await expect.element(page.getByTestId('sidebar-drawer')).not.toBeInTheDocument();
		expect(document.querySelectorAll('dialog[open]')).toHaveLength(1);

		await userEvent.keyboard('{Escape}');

		await expect.element(dialog).not.toBeInTheDocument();
		await expect.element(menu).toHaveFocus();
	});

	it('gives focus back to ☰ even when clicks focus nothing (as in Safari)', async () => {
		await page.viewport(320, 640);
		renderLayout();
		const menu = page.getByTestId('open-drawer');
		const active = document.activeElement;
		if (active instanceof HTMLElement) active.blur();
		(menu.element() as HTMLElement).click();
		const drawerLink = page.getByRole('dialog', { name: 'My Den' }).getByRole('link', {
			name: 'Report a bug'
		});
		await expect.element(drawerLink).toBeVisible();
		(drawerLink.element() as HTMLElement).click();
		await expect.element(page.getByRole('textbox', { name: 'What happened?' })).toHaveFocus();

		await userEvent.keyboard('{Escape}');

		await expect
			.element(page.getByRole('dialog', { name: 'Report a bug' }))
			.not.toBeInTheDocument();
		await expect.element(menu).toHaveFocus();
	});

	it('puts "Report a bug" in the drawer’s footer, under Accounts and above Sign out', async () => {
		await page.viewport(320, 640);
		renderLayout();
		await page.getByTestId('open-drawer').click();

		const footer = page.getByTestId('sidebar-drawer').element();
		const labels = [...footer.querySelectorAll('a, button')]
			.map((control) => control.textContent.trim())
			.filter((text) => ['Accounts', 'Report a bug', 'Sign out'].includes(text));
		expect(labels).toEqual(['Accounts', 'Report a bug', 'Sign out']);
	});

	it('hands the form the stack it runs on: mock mode says it keeps no reports', async () => {
		await page.viewport(1280, 800);
		renderLayout('mock');

		await page.getByTestId('report-bug-link').click();

		await expect
			.element(page.getByRole('dialog').getByTestId('report-test-stack'))
			.toHaveTextContent("Mock mode doesn't keep reports.");
	});

	it('says nothing about the stack on a real one', async () => {
		await page.viewport(1280, 800);
		renderLayout('none');

		await page.getByTestId('report-bug-link').click();

		await expect.element(page.getByRole('textbox', { name: 'What happened?' })).toBeVisible();
		await expect.element(page.getByTestId('report-test-stack')).not.toBeInTheDocument();
	});

	it('keeps the report’s draft for the visitor, and starts it over for another', async () => {
		await page.viewport(1280, 800);
		const bob = {
			...session,
			did: did('did:plc:bob'),
			handle: handleFromTrusted('bob.zurfur.app')
		};
		const { rerender } = renderLayout();
		const textBox = page.getByRole('textbox', { name: 'What happened?' });
		await page.getByTestId('report-bug-link').click();
		await textBox.fill("Alice's draft");
		await userEvent.keyboard('{Escape}');
		await rerender({ data: { session, buildId: UNKNOWN_BUILD_ID, testStack: 'none' }, children });
		await page.getByTestId('report-bug-link').click();
		await expect.element(textBox).toHaveValue("Alice's draft");
		await userEvent.keyboard('{Escape}');

		await rerender({
			data: { session: bob, buildId: UNKNOWN_BUILD_ID, testStack: 'none' },
			children
		});
		await page.getByTestId('report-bug-link').click();

		await expect.element(textBox).toHaveValue('');
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

	it('fetches the folders above a deep item at most four at a time', async () => {
		const segments = Array.from({ length: 16 }, (_, depth) => `d${String(depth)}`);
		const crumbs = segments.map((_, depth) => ({
			href: ['/den', ...segments.slice(0, depth)].join('/'),
			name: depth === 0 ? 'Alice' : `d${String(depth - 1)}`
		}));
		const address = ['/den', ...segments].join('/');
		nav.den = {
			kind: 'denPage',
			address,
			outcome: {
				outcome: 'page',
				page: {
					view: 'card',
					node: { view: 'card', name: 'Deep', type: 'x', mounted: false },
					crumbs
				},
				flagLinks: { on: address, off: address }
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

		await expect.poll(() => nav.preloaded.length).toBe(16);
		await expect.poll(() => nav.load.inFlight).toBe(0);
		expect(nav.load.most).toBe(4);
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
		await rerender({
			data: { session: bob, buildId: UNKNOWN_BUILD_ID, testStack: 'none' },
			children
		});
		for (const callback of nav.callbacks) callback();

		expect(trees.built.map((built) => built.owner)).toEqual([session.did]);
		expect(trees.built[0]?.navigatedAs).toEqual([session.did, bob.did]);
	});
});
