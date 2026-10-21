import '$lib/styles/tokens.css';
import '$lib/styles/base.css';
import { createRawSnippet } from 'svelte';
import { page, userEvent } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Dialog from './Dialog.svelte';

/** A body with one field to focus. */
const formBody = createRawSnippet(() => ({
	render: () => '<div><input id="bug-description" aria-label="What happened?" /></div>'
}));

/** A body far taller than a phone. */
const tallBody = createRawSnippet(() => ({
	render: () => '<div style="height: 2000px">A long form</div>'
}));

const sendFooter = createRawSnippet(() => ({
	render: () => '<button type="button">Send report</button>'
}));

/** Buttons the tests put on the page beside the dialog, removed after each test. */
const outside: HTMLElement[] = [];

function outsideButton(name: string): HTMLButtonElement {
	const button = document.createElement('button');
	button.type = 'button';
	button.textContent = name;
	document.body.append(button);
	outside.push(button);
	return button;
}

/** Leave focus on the body, as Safari does after a click on a button. */
function focusNothing(): void {
	const active = document.activeElement;
	if (active instanceof HTMLElement) active.blur();
	expect(document.activeElement).toBe(document.body);
}

function dialogElement(): Element {
	return page.getByRole('dialog').element();
}

describe('Dialog', () => {
	beforeEach(async () => {
		await page.viewport(1280, 800);
	});

	afterEach(() => {
		for (const element of outside.splice(0)) element.remove();
	});

	it('focuses its close button and gives focus back to the opener, with no new props', async () => {
		const opener = outsideButton('Open');
		opener.focus();
		render(Dialog, { open: true, label: 'My Den', children: formBody });

		await expect.element(page.getByRole('dialog', { name: 'My Den' })).toBeInTheDocument();
		await expect.element(page.getByTestId('dialog-close')).toHaveFocus();
		await userEvent.keyboard('{Escape}');
		await expect.element(page.getByRole('dialog')).not.toBeInTheDocument();
		expect(document.activeElement).toBe(opener);
	});

	it('focuses the chosen element on open', async () => {
		render(Dialog, {
			open: true,
			label: 'Report a bug',
			initialFocus: 'bug-description',
			children: formBody
		});

		await expect.element(page.getByRole('textbox', { name: 'What happened?' })).toHaveFocus();
	});

	it('falls back to its close button when the chosen element is not there', async () => {
		render(Dialog, {
			open: true,
			label: 'Report a bug',
			initialFocus: 'no-such-element',
			children: formBody
		});

		await expect.element(page.getByTestId('dialog-close')).toHaveFocus();
	});

	it('is named by its visible title, with an id of its own per dialog', async () => {
		render(Dialog, { open: true, label: 'My Den', children: formBody });
		render(Dialog, { open: false, label: 'Report a bug', children: formBody });

		const dialogs = [...document.querySelectorAll('dialog')];
		expect(dialogs).toHaveLength(2);
		const titleIds = dialogs.map((dialog) => dialog.getAttribute('aria-labelledby') ?? '');
		expect(titleIds[0]).not.toBe('');
		expect(titleIds[0]).not.toBe(titleIds[1]);
		expect(document.getElementById(titleIds[0] ?? '')?.textContent).toBe('My Den');
		expect(document.getElementById(titleIds[1] ?? '')?.textContent).toBe('Report a bug');
		expect(dialogs[0]?.hasAttribute('aria-label')).toBe(false);
		await expect.element(page.getByRole('dialog', { name: 'My Den' })).toBeVisible();
	});

	it('gives focus to the return target on Escape, even when nothing had focus at opening', async () => {
		const target = outsideButton('Report a bug');
		focusNothing();
		render(Dialog, {
			open: true,
			label: 'Report a bug',
			returnFocus: target,
			children: formBody
		});

		await expect.element(page.getByTestId('dialog-close')).toHaveFocus();
		await userEvent.keyboard('{Escape}');
		await expect.element(page.getByRole('dialog')).not.toBeInTheDocument();
		expect(document.activeElement).toBe(target);
	});

	it('prefers the return target over whatever had focus at opening', async () => {
		const opener = outsideButton('Open');
		const target = outsideButton('Menu');
		opener.focus();
		render(Dialog, { open: true, label: 'Report a bug', returnFocus: target, children: formBody });

		await page.getByTestId('dialog-close').click();
		await expect.element(page.getByRole('dialog')).not.toBeInTheDocument();
		expect(document.activeElement).toBe(target);
	});

	it('fills a phone screen, keeping the footer in view while the body scrolls', async () => {
		await page.viewport(320, 480);
		render(Dialog, {
			open: true,
			label: 'Report a bug',
			variant: 'full-screen-on-phone',
			children: tallBody,
			footer: sendFooter
		});

		const send = page.getByRole('button', { name: 'Send report' });
		await expect.element(send).toBeVisible();
		const box = dialogElement().getBoundingClientRect();
		expect(box.left).toBe(0);
		expect(box.top).toBe(0);
		expect(box.width).toBe(window.innerWidth);
		expect(box.height).toBe(window.innerHeight);

		const body = dialogElement().querySelector('.dialog__body');
		if (!(body instanceof HTMLElement)) throw new Error('the dialog has no body');
		expect(body.scrollHeight).toBeGreaterThan(body.clientHeight);
		expect(body.contains(send.element())).toBe(false);

		for (const scrollTop of [0, body.scrollHeight]) {
			body.scrollTop = scrollTop;
			const footer = send.element().getBoundingClientRect();
			expect(footer.top).toBeGreaterThanOrEqual(0);
			expect(footer.bottom).toBeLessThanOrEqual(window.innerHeight);
		}
		expect(body.scrollTop).toBeGreaterThan(0);
		expect(document.documentElement.scrollWidth).toBeLessThanOrEqual(window.innerWidth);
	});

	it('stays a centred modal on a wide screen', async () => {
		render(Dialog, {
			open: true,
			label: 'Report a bug',
			variant: 'full-screen-on-phone',
			children: formBody,
			footer: sendFooter
		});

		await expect.element(page.getByRole('button', { name: 'Send report' })).toBeVisible();
		const box = dialogElement().getBoundingClientRect();
		expect(box.width).toBeLessThan(window.innerWidth);
		expect(box.height).toBeLessThan(window.innerHeight);
		expect(box.left).toBeGreaterThan(0);
	});
});
