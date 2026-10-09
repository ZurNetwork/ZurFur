import { page, userEvent } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import type { Session } from '$lib/api/session';
import type { Trail } from '$lib/api/trail';
import { did, handleFromTrusted } from '$lib/types/brand';
import AppShell from './AppShell.svelte';

const alice: Session = {
	did: did('did:plc:alice'),
	handle: handleFromTrusted('alice.zurfur.app'),
	displayName: 'Alice',
	avatarUrl: undefined
};

const trail: Trail = [{ step: 'named', label: 'Accounts', href: undefined }];

/** The sidebar's content: one link, so the drawer has something to hold. */
const navigation = createRawSnippet(() => ({
	render: () => '<nav aria-label="Test navigation"><a href="/accounts">Somewhere</a></nav>'
}));

/** The pane's content. */
const children = createRawSnippet(() => ({ render: () => '<h1>Pane heading</h1>' }));

function renderShell(denServed = true) {
	return render(AppShell, {
		session: alice,
		trail,
		section: 'accounts',
		denServed,
		navigation,
		children
	});
}

describe('AppShell', () => {
	it('puts "Skip to content" first, pointing at the main pane', async () => {
		renderShell();

		const skip = page.getByTestId('skip-link');
		await expect.element(skip).toHaveAttribute('href', '#main');
		const main = page.getByRole('main');
		await expect.element(main).toHaveAttribute('id', 'main');
		await expect.element(main.getByRole('heading', { name: 'Pane heading' })).toBeInTheDocument();
	});

	it('shows the top bar with the Accounts link marked current and who is signed in', async () => {
		renderShell();

		await expect.element(page.getByRole('banner')).toBeInTheDocument();
		await expect.element(page.getByTestId('accounts-link')).toHaveAttribute('aria-current', 'page');
		await expect
			.element(page.getByRole('banner').getByTestId('session-handle'))
			.toHaveTextContent('alice.zurfur.app');
	});

	it('keeps the drawer closed and its content unrendered until ☰ is pressed', async () => {
		renderShell();

		await expect.element(page.getByTestId('sidebar-drawer')).not.toBeInTheDocument();
		await expect.element(page.getByTestId('open-drawer')).toHaveAttribute('aria-expanded', 'false');
	});

	it('opens the drawer as a modal from ☰, focusing its close button, with the footer inside', async () => {
		renderShell();

		await page.getByTestId('open-drawer').click();

		const drawer = page.getByRole('dialog', { name: 'My Den' });
		await expect.element(drawer).toBeInTheDocument();
		await expect.element(page.getByTestId('dialog-close')).toHaveFocus();
		await expect.element(page.getByTestId('sidebar-drawer')).toBeInTheDocument();
		await expect.element(page.getByTestId('drawer-accounts-link')).toBeInTheDocument();
		await expect.element(drawer.getByRole('button', { name: 'Sign out' })).toBeInTheDocument();
		await expect.element(page.getByTestId('open-drawer')).toHaveAttribute('aria-expanded', 'true');
	});

	it('closes the drawer with Escape and gives focus back to ☰', async () => {
		renderShell();
		const opener = page.getByTestId('open-drawer');

		await opener.click();
		await expect.element(page.getByTestId('dialog-close')).toHaveFocus();
		await userEvent.keyboard('{Escape}');

		await expect.element(page.getByTestId('sidebar-drawer')).not.toBeInTheDocument();
		await expect.element(opener).toHaveFocus();
	});

	it('closes the drawer with its close button and gives focus back to ☰', async () => {
		renderShell();
		const opener = page.getByTestId('open-drawer');

		await opener.click();
		await page.getByTestId('dialog-close').click();

		await expect.element(page.getByTestId('sidebar-drawer')).not.toBeInTheDocument();
		await expect.element(opener).toHaveFocus();
	});

	it('shows no sidebar, no drawer and no ☰ where this run serves no Den', async () => {
		renderShell(false);

		await expect.element(page.getByRole('main')).toBeInTheDocument();
		await expect.element(page.getByTestId('sidebar')).not.toBeInTheDocument();
		await expect.element(page.getByTestId('open-drawer')).not.toBeInTheDocument();
		await expect.element(page.getByRole('dialog', { includeHidden: true })).not.toBeInTheDocument();
		await expect.element(page.getByTestId('accounts-link')).toBeInTheDocument();
		await expect.element(page.getByRole('button', { name: 'Sign out' })).toBeInTheDocument();
	});

	it('keeps the sections and Sign out in the row on a phone when there is no drawer', async () => {
		renderShell(false);
		await expect.element(page.getByTestId('accounts-link')).toBeInTheDocument();

		expect(page.getByTestId('accounts-link').element().closest('.desktop-only')).toBeNull();
		expect(page.getByTestId('session-handle').element().closest('.desktop-only')).toBeNull();
	});

	it('links to My Den in the top bar only where a Den is served', async () => {
		renderShell(true);
		await expect.element(page.getByTestId('den-link')).toHaveAttribute('href', '/den');
	});

	it('has no My Den link on the everyday stack', async () => {
		renderShell(false);
		await expect.element(page.getByTestId('accounts-link')).toBeInTheDocument();
		await expect.element(page.getByTestId('den-link')).not.toBeInTheDocument();
	});
});
