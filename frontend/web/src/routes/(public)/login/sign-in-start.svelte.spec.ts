import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { formStub } from '$lib/testing/superforms';

/** Every full page load the page asks for, instead of the window leaving the spec. */
const navigation = vi.hoisted(() => ({
	loadFullPage: vi.fn<(location: string) => 'left' | 'refused'>(() => 'left')
}));
vi.mock('$lib/navigation', () => navigation);

/** The options the page handed superForm, captured so the spec can play a result through them. */
const captured = vi.hoisted((): { onResult: unknown } => ({ onResult: undefined }));
vi.mock('sveltekit-superforms', async () => {
	const client = await import('sveltekit-superforms/client');
	return {
		superForm: (...args: Parameters<typeof client.superForm>) => {
			captured.onResult = args[1]?.onResult;
			return client.superForm(...args);
		}
	};
});

const { default: Login } = await import('./+page.svelte');

/** The sign-in start's answer on the dev profile: straight to the callback only the API serves. */
const CALLBACK = '/signin-callback?code=dev&state=dev';

const data = {
	session: undefined,
	callbackError: undefined,
	form: formStub({ handle: '' }),
	signoutUnconfirmed: false
};

/** The action's answer to the enhanced submit, as SvelteKit serializes a redirect. */
function redirectAnswer(location: string): Response {
	const body = JSON.stringify({ type: 'redirect', status: 303, location });
	return new Response(body, { headers: { 'content-type': 'application/json' } });
}

/** Play `result` through the page's own onResult; true when the page cancelled superforms' handling. */
async function playResult(result: object): Promise<boolean> {
	const onResult = captured.onResult;
	if (typeof onResult !== 'function') throw new Error('the page handed superForm no onResult');
	let cancelled = false;
	const cancel = (): void => {
		cancelled = true;
	};
	await Reflect.apply(onResult, undefined, [{ result, cancel }]);
	return cancelled;
}

afterEach(() => {
	vi.restoreAllMocks();
	navigation.loadFullPage.mockClear();
});

describe('/login: the sign-in start', () => {
	it('leaves by a full page load to the redirect’s location, with no client-router error', async () => {
		const fetch = vi.spyOn(window, 'fetch').mockResolvedValue(redirectAnswer(CALLBACK));
		const consoleError = vi.spyOn(console, 'error');
		render(Login, { data });

		await page.getByLabelText('Handle').fill('alice.test');
		await page.getByRole('button', { name: 'Sign in' }).click();

		await expect.poll(() => navigation.loadFullPage.mock.calls).toEqual([[CALLBACK]]);
		expect(fetch).toHaveBeenCalledOnce();
		expect(consoleError).not.toHaveBeenCalled();
	});

	it.each([
		['redirect', true, { type: 'redirect', status: 303, location: CALLBACK }],
		['failure', false, { type: 'failure', status: 422, data: { form: data.form } }],
		['success', false, { type: 'success', status: 200, data: { form: data.form } }],
		['error', false, { type: 'error', status: 500, error: new Error('boom') }]
	])('the %s result: superforms’ handling cancelled is %s', async (type, cancels, result) => {
		render(Login, { data });

		const cancelled = await playResult(result);

		expect(cancelled).toBe(cancels);
		const leaves = type === 'redirect' ? [[CALLBACK]] : [];
		expect(navigation.loadFullPage.mock.calls).toEqual(leaves);
	});
});
