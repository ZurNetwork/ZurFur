import { describe, expect, it } from 'vitest';
import { isRedirect } from '@sveltejs/kit';
import { actions, load } from './+page.server';

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<(typeof actions)['default']>[0];

function fetchReturning(response: () => Response): typeof globalThis.fetch {
	return (async () => response()) as typeof globalThis.fetch;
}

const anonymousFetch = fetchReturning(
	() =>
		new Response(
			JSON.stringify({
				type: 'urn:zurfur:error:not-authenticated',
				code: 'not_authenticated',
				title: 'Not authenticated',
				status: 401
			}),
			{ status: 401, headers: { 'content-type': 'application/problem+json' } }
		)
);

function loadEvent(fetch: typeof globalThis.fetch, search = ''): LoadEvent {
	return { fetch, url: new URL(`http://localhost/login${search}`) } as unknown as LoadEvent;
}

/** `load` types its return as possibly-void (it may throw a redirect); pin the data shape. */
async function runLoad(event: LoadEvent): Promise<{ callbackError: string | null }> {
	return (await load(event)) as { callbackError: string | null };
}

async function signinAction(fetch: typeof globalThis.fetch, handle: string | null) {
	const body = new URLSearchParams(handle === null ? {} : { handle });
	const request = new Request('http://localhost/login', { method: 'POST', body });
	return actions.default({ request, fetch } as unknown as ActionEvent);
}

/** Run a thunk expected to throw a SvelteKit redirect; return it. */
async function expectRedirect(thunk: () => unknown) {
	try {
		await thunk();
	} catch (thrown) {
		if (isRedirect(thrown)) return thrown;
		throw thrown;
	}
	throw new Error('expected a redirect to be thrown');
}

describe('/login load', () => {
	it('renders signed-out with no callback error by default', async () => {
		const result = await runLoad(loadEvent(anonymousFetch));
		expect(result).toEqual({ callbackError: null });
	});

	it('maps a known ?error code to its message', async () => {
		const result = await runLoad(loadEvent(anonymousFetch, '?error=denied'));
		expect(result.callbackError).toBe('Sign-in was cancelled at your PDS.');
	});

	it('falls back on an unknown ?error code', async () => {
		const result = await runLoad(loadEvent(anonymousFetch, '?error=mystery'));
		expect(result.callbackError).toBe('Sign-in failed. Try again.');
	});

	it('bounces a signed-in visitor home', async () => {
		const signedInFetch = fetchReturning(() =>
			Response.json({ did: 'did:plc:a', handle: 'a.test', display_name: null, avatar_url: null })
		);
		const redirect = await expectRedirect(() => load(loadEvent(signedInFetch)));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/');
	});

	it('renders signed-out when the backend is unreachable', async () => {
		const deadFetch = (async () => {
			throw new TypeError('fetch failed');
		}) as typeof globalThis.fetch;
		const result = await runLoad(loadEvent(deadFetch));
		expect(result).toEqual({ callbackError: null });
	});
});

describe('/login signin action', () => {
	it('rejects an empty handle locally with a problem-shaped failure', async () => {
		const neverFetch = (async () => {
			throw new Error('must not reach the backend');
		}) as typeof globalThis.fetch;
		const failure = await signinAction(neverFetch, '   ');
		expect(failure).toMatchObject({ status: 422, data: { problem: { title: 'Enter a handle.' } } });
	});

	it('relays the PDS authorize URL as a 303 navigation', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const redirectingFetch = fetchReturning(
			() => new Response(null, { status: 303, headers: { location: authorizeUrl } })
		);
		const redirect = await expectRedirect(() => signinAction(redirectingFetch, 'alice.test'));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe(authorizeUrl);
	});

	it('hands a backend problem to the page to render', async () => {
		const rejectingFetch = fetchReturning(
			() =>
				new Response(
					JSON.stringify({
						type: 'urn:zurfur:error:invalid-request',
						code: 'invalid_request',
						title: 'Invalid request',
						status: 422
					}),
					{ status: 422, headers: { 'content-type': 'application/problem+json' } }
				)
		);
		const failure = await signinAction(rejectingFetch, 'nope');
		expect(failure).toMatchObject({ status: 422, data: { problem: { code: 'invalid_request' } } });
	});
});
