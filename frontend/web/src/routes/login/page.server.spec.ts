import { describe, expect, it } from 'vitest';
import { fetchStub, problemResponse, unreachableFetch } from '$lib/testing/http';
import { expectRedirect } from '$lib/testing/redirect';
import { actions, load } from './+page.server';
import type { Session } from '$lib/api/session';

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<(typeof actions)['default']>[0];

const alice: Session = {
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	display_name: 'Alice',
	avatar_url: null
};

function loadEvent(session: Session | null, search = ''): LoadEvent {
	const event = {
		parent: async () => ({ session }),
		url: new URL(`http://localhost/login${search}`)
	};
	return event as unknown as LoadEvent;
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

describe('/login load', () => {
	it('renders signed-out with no callback error by default', async () => {
		const result = await runLoad(loadEvent(null));
		expect(result).toEqual({ callbackError: null });
	});

	it('maps a known ?error code to its message', async () => {
		const result = await runLoad(loadEvent(null, '?error=denied'));
		expect(result.callbackError).toBe('Sign-in was cancelled at your PDS.');
	});

	it('falls back on an unknown ?error code', async () => {
		const result = await runLoad(loadEvent(null, '?error=mystery'));
		expect(result.callbackError).toBe('Sign-in failed. Try again.');
	});

	it('bounces a signed-in visitor home', async () => {
		const redirect = await expectRedirect(() => load(loadEvent(alice)));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/');
	});
});

describe('/login signin action', () => {
	it('rejects an empty handle locally with a problem-shaped failure', async () => {
		const failure = await signinAction(unreachableFetch('must not reach the backend'), '   ');
		expect(failure).toMatchObject({ status: 422, data: { problem: { title: 'Enter a handle.' } } });
	});

	it('relays the PDS authorize URL as a 303 navigation', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const { fetch } = fetchStub(
			() => new Response(null, { status: 303, headers: { location: authorizeUrl } })
		);
		const redirect = await expectRedirect(() => signinAction(fetch, 'alice.test'));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe(authorizeUrl);
	});

	it('hands a backend problem to the page to render', async () => {
		const { fetch } = fetchStub(() => problemResponse(422, 'invalid_request'));
		const failure = await signinAction(fetch, 'nope');
		expect(failure).toMatchObject({ status: 422, data: { problem: { code: 'invalid_request' } } });
	});
});
