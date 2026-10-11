import { page } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

/** An error message that names a real path and an internal detail. */
const LEAKY_MESSAGE = 'Not found: /secret/did:plc:secretsecret in the internal store';

/** What `$app/state`'s page holds while the public error page renders. */
const errorState = vi.hoisted(() => ({ status: 404 }));

vi.mock('$app/state', () => ({
	page: {
		get status() {
			return errorState.status;
		},
		route: { id: null },
		error: { message: LEAKY_MESSAGE },
		data: {},
		url: new URL('http://127.0.0.1:5174/secret/did:plc:secretsecret')
	}
}));

const { default: ErrorPage } = await import('./+error.svelte');

describe('public error page', () => {
	it('shows the not-found by its status, never the error’s own text', async () => {
		errorState.status = 404;
		render(ErrorPage);

		await expect.element(page.getByRole('heading', { name: 'Nothing here' })).toBeInTheDocument();
		expect(document.body.textContent).not.toContain('secret');
	});

	it('shows the generic copy for a 500, never the error’s own text', async () => {
		errorState.status = 500;
		render(ErrorPage);

		await expect
			.element(page.getByRole('heading', { name: 'Something went wrong' }))
			.toBeInTheDocument();
		expect(document.body.textContent).not.toContain('secret');
	});
});
