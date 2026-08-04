import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Login from './+page.svelte';
import type { Problem } from '$lib/api/problem';
import { formStub } from '$lib/testing/superforms';

/** Thin wrapper over the shared {@link formStub}: this page's field defaults. */
function loginForm(
	overrides: {
		handle?: string;
		errors?: { handle?: string[] };
		message?: Problem;
	} = {}
) {
	return formStub({ handle: overrides.handle ?? '' }, overrides);
}

function loginData(
	form: ReturnType<typeof loginForm> = loginForm(),
	callbackError: string | null = null
) {
	return { session: null, callbackError, form };
}

describe('/login page', () => {
	it('renders the handle input and submit', async () => {
		render(Login, { data: loginData() });

		await expect.element(page.getByRole('heading', { level: 1 })).toHaveTextContent('Sign in');
		await expect.element(page.getByLabelText('Handle')).toBeInTheDocument();
		await expect.element(page.getByRole('button', { name: 'Sign in' })).toBeInTheDocument();
	});

	it('renders a callback error from the redirect contract', async () => {
		render(Login, { data: loginData(loginForm(), 'Sign-in was cancelled at your PDS.') });

		await expect
			.element(page.getByTestId('callback-error'))
			.toHaveTextContent('Sign-in was cancelled at your PDS.');
	});

	it('renders a local validation failure as a field error', async () => {
		render(Login, {
			data: loginData(loginForm({ errors: { handle: ['Please insert your handle'] } }))
		});

		await expect
			.element(page.getByTestId('handle-error'))
			.toHaveTextContent('Please insert your handle');
	});

	it('re-fills the handle from the form state after a failed submit', async () => {
		render(Login, {
			data: loginData(
				loginForm({ handle: 'kept.example', errors: { handle: ['This handle is not valid'] } })
			)
		});

		await expect.element(page.getByLabelText('Handle')).toHaveValue('kept.example');
	});

	it('renders a signin problem through the problem seam', async () => {
		const problem: Problem = {
			type: 'urn:zurfur:error:invalid-request',
			code: 'invalid_request',
			title: 'Invalid request',
			detail: 'the handle could not be used to start sign-in',
			status: 422
		};
		render(Login, { data: loginData(loginForm({ message: problem })) });

		await expect
			.element(page.getByTestId('problem'))
			.toHaveTextContent('the handle could not be used to start sign-in');
	});
});
