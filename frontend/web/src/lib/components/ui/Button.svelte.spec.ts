import { page } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import { resolve } from '$app/paths';
import Button from './Button.svelte';

const label = createRawSnippet(() => ({ render: () => '<span>Go</span>' }));

describe('Button', () => {
	it('is a link with href', async () => {
		render(Button, { href: resolve('/accounts'), children: label });
		await expect
			.element(page.getByRole('link', { name: 'Go' }))
			.toHaveAttribute('href', '/accounts');
	});

	it('is a real button otherwise, and runs its action', async () => {
		const onclick = vi.fn();
		render(Button, { onclick, children: label });
		await page.getByRole('button', { name: 'Go' }).click();
		expect(onclick).toHaveBeenCalledOnce();
	});

	it('marks itself busy and ignores presses while busy', async () => {
		const onclick = vi.fn();
		render(Button, { onclick, busy: true, children: label });
		const button = page.getByRole('button', { name: 'Go' });
		await expect.element(button).toHaveAttribute('aria-busy', 'true');
		button.element().dispatchEvent(new MouseEvent('click', { bubbles: true }));
		expect(onclick).not.toHaveBeenCalled();
	});
});
