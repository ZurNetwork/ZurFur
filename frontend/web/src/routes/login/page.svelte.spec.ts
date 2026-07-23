import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Login from './+page.svelte';

const signedOut = { session: null, callbackError: null };

describe('/login page', () => {
	it('renders the handle input and submit', async () => {
		render(Login, { data: signedOut, form: null });

		await expect.element(page.getByRole('heading', { level: 1 })).toHaveTextContent('Sign in');
		await expect.element(page.getByLabelText('Handle')).toBeInTheDocument();
		await expect.element(page.getByRole('button', { name: 'Sign in' })).toBeInTheDocument();
	});

	it('renders a callback error from the redirect contract', async () => {
		render(Login, {
			data: { session: null, callbackError: 'Sign-in was cancelled at your PDS.' },
			form: null
		});

		await expect
			.element(page.getByTestId('callback-error'))
			.toHaveTextContent('Sign-in was cancelled at your PDS.');
	});

	it('renders a signin problem through the problem seam', async () => {
		const problem = {
			type: 'urn:zurfur:error:invalid-request',
			code: 'invalid_request',
			title: 'Invalid request',
			detail: 'the handle could not be used to start sign-in',
			status: 422
		};
		render(Login, { data: signedOut, form: { problem } });

		await expect
			.element(page.getByTestId('problem'))
			.toHaveTextContent('the handle could not be used to start sign-in');
	});
});
