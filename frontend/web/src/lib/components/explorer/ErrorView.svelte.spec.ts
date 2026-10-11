import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import ErrorView from './ErrorView.svelte';

describe('ErrorView', () => {
	it('shows the one not-found copy for a 404', async () => {
		render(ErrorView, { status: 404, routeId: '/(session)/accounts/[id]' });

		await expect.element(page.getByRole('heading', { name: 'Nothing here' })).toBeInTheDocument();
		await expect
			.element(page.getByText("This doesn't exist, or you can't open it."))
			.toBeInTheDocument();
	});

	it('shows a calm generic copy for any other status', async () => {
		render(ErrorView, { status: 502, routeId: '/(session)/accounts' });

		await expect
			.element(page.getByRole('heading', { name: 'Something went wrong' }))
			.toBeInTheDocument();
	});

	it('says a failed sign-out did not complete', async () => {
		render(ErrorView, { status: 502, routeId: '/(session)/logout' });

		await expect
			.element(page.getByRole('heading', { name: "You're still signed in" }))
			.toBeInTheDocument();
		await expect
			.element(page.getByText('Sign-out did not complete. Try again.'))
			.toBeInTheDocument();
	});

	it('offers a way back', async () => {
		render(ErrorView, { status: 500, routeId: undefined });

		await expect.element(page.getByRole('link', { name: 'Back to the start' })).toBeInTheDocument();
	});
});
