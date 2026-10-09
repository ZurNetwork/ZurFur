import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { resolve } from '$app/paths';
import type { Trail } from '$lib/api/trail';
import PathBar from './PathBar.svelte';

/** `~ / accounts / Alice's Studio`, linked down to the parent. */
const deepTrail: Trail = [
	{ step: 'root', href: resolve('/') },
	{ step: 'named', label: 'accounts', href: resolve('/accounts') },
	{ step: 'named', label: "Alice's Studio", href: undefined }
];

/** The text of the step marked `aria-current="page"`, if any. */
function currentStep(): string | undefined {
	const bar = page.getByTestId('path-bar').element();
	return bar.querySelector('[aria-current="page"]')?.textContent ?? undefined;
}

describe('PathBar', () => {
	it('lists every step, the last marked as the current page', async () => {
		render(PathBar, { trail: deepTrail });

		const path = page.getByTestId('path-bar');
		await expect.element(path.getByRole('link', { name: 'accounts' })).toBeInTheDocument();
		await expect.element(path.getByText("Alice's Studio")).toBeInTheDocument();
		expect(currentStep()).toBe("Alice's Studio");
	});

	it('shows the root as ~, named "My Den" for screen readers', async () => {
		render(PathBar, { trail: deepTrail });

		const root = page.getByTestId('path-bar').getByRole('link', { name: 'My Den' });
		await expect.element(root).toHaveTextContent('~');
	});

	it('offers the parent as the back link on a phone', async () => {
		render(PathBar, { trail: deepTrail });

		const back = page.getByTestId('path-bar-back').getByRole('link');
		await expect.element(back).toHaveAttribute('href', '/accounts');
		await expect.element(back).toHaveTextContent('accounts');
	});

	it('has no back link when the page has no parent', async () => {
		const single: Trail = [{ step: 'named', label: 'Accounts', href: undefined }];
		render(PathBar, { trail: single });

		await expect.element(page.getByTestId('path-bar-back')).not.toBeInTheDocument();
		await expect.element(page.getByText('Accounts')).toBeInTheDocument();
		expect(currentStep()).toBe('Accounts');
	});

	it('isolates a right-to-left name so it cannot reorder the bar', async () => {
		const rtl: Trail = [
			{ step: 'root', href: resolve('/') },
			{ step: 'named', label: 'طلب رسم', href: undefined }
		];
		render(PathBar, { trail: rtl });

		const name = page.getByTestId('path-bar').getByText('طلب رسم');
		await expect.element(name).toBeInTheDocument();
		expect(name.element().tagName).toBe('BDI');
	});
});
