import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import { did, handleFromTrusted } from '$lib/types/brand';

/** The callbacks the layout registered with `afterNavigate`, to fire as a navigation would. */
const nav = vi.hoisted(() => ({ callbacks: [] as (() => void)[] }));

vi.mock('$app/navigation', () => ({
	afterNavigate: (callback: () => void) => {
		nav.callbacks.push(callback);
	}
}));

vi.mock('$app/state', () => ({
	page: {
		data: {
			trail: [
				{ step: 'named', label: 'Accounts', href: '/accounts' },
				{ step: 'named', label: 'alice-studio.zurfur.app', href: undefined }
			]
		},
		route: { id: '/(session)/accounts/[id]' },
		status: 200
	}
}));

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

	it('never scrolls sideways at 320 px', async () => {
		await page.viewport(320, 640);
		renderLayout();
		await expect.element(page.getByRole('banner')).toBeInTheDocument();
		expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(window.innerWidth);
	});
});
