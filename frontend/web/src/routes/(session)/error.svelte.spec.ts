import { page } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

/** An error message that names a real Den path and an internal detail. */
const LEAKY_MESSAGE =
	'API contract violation: /den/accounts/did:plc:secretsecret responded 200 — no node';

/** What `$app/state`'s page holds while this error page renders. */
const errorState = vi.hoisted(() => ({
	status: 500,
	routeId: '/(session)/accounts',
	denServed: false
}));

vi.mock('$app/state', () => ({
	page: {
		get status() {
			return errorState.status;
		},
		get route() {
			return { id: errorState.routeId };
		},
		error: { message: LEAKY_MESSAGE },
		get data() {
			return { denServed: errorState.denServed };
		},
		url: new URL('http://127.0.0.1:5174/den/accounts/did:plc:secretsecret')
	}
}));

const { default: ErrorPage } = await import('./+error.svelte');

describe('(session) error page', () => {
	it('never shows the error’s own message', async () => {
		errorState.status = 500;
		errorState.routeId = '/(session)/accounts';
		render(ErrorPage);

		await expect.element(page.getByTestId('error-view')).toBeInTheDocument();
		expect(document.body.textContent).not.toContain('secret');
		expect(document.body.textContent).not.toContain('contract violation');
	});

	it('picks its copy from the status: a 404 reads "Nothing here"', async () => {
		errorState.status = 404;
		errorState.routeId = '/(session)/accounts/[id]';
		render(ErrorPage);

		await expect.element(page.getByRole('heading', { name: 'Nothing here' })).toBeInTheDocument();
		expect(document.body.textContent).not.toContain('secret');
	});

	it('says a failed sign-out did not complete (the route, not the message, picks it)', async () => {
		errorState.status = 502;
		errorState.routeId = '/(session)/logout';
		render(ErrorPage);

		await expect
			.element(page.getByText('Sign-out did not complete. Try again.'))
			.toBeInTheDocument();
		expect(document.body.textContent).not.toContain('secret');
	});

	it('leads back to My Den only where a Den is served', async () => {
		errorState.status = 404;
		errorState.routeId = '/(session)/accounts/[id]';
		errorState.denServed = true;
		render(ErrorPage);
		await expect.element(page.getByRole('link', { name: 'Back to My Den' })).toBeInTheDocument();
	});

	it('leads back to the start on the everyday stack, never to My Den', async () => {
		errorState.status = 404;
		errorState.routeId = '/(session)/accounts/[id]';
		errorState.denServed = false;
		render(ErrorPage);
		await expect.element(page.getByRole('link', { name: 'Back to the start' })).toBeInTheDocument();
		await expect
			.element(page.getByRole('link', { name: 'Back to My Den' }))
			.not.toBeInTheDocument();
	});
});
