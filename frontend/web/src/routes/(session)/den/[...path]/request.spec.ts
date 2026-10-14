import { isRedirect, type RequestEvent } from '@sveltejs/kit';
import { describe, expect, it, vi } from 'vitest';
import type { DenPageData } from '$lib/api/den';
import { FRAME_ANCESTORS_POLICY, X_FRAME_OPTIONS } from '$lib/server/frame-headers';
import { SESSION_CACHE_CONTROL, SESSION_REFERRER_POLICY } from '$lib/server/session-headers';
import { fetchStub, problemResponse } from '$lib/testing/http';

/** Mock mode pinned off, so every Den read goes through the live port whatever the process env says. */
vi.mock('$lib/server/api/zurfur-api-mock', async (importActual) => {
	const actual = await importActual<typeof import('$lib/server/api/zurfur-api-mock')>();
	return { ...actual, mockModeEnabled: () => false };
});

const { handle } = await import('../../../../hooks.server');
const { load } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];

/** The live Den root as the backend answers it: Alice's own folder, empty. */
const DEN_ROOT_BODY = {
	node: { name: 'Alice', type: 'user', kind: 'directory', open: { ownLevel: 'private' } },
	listing: { entries: [] }
};

/** A backend where Alice is signed in and her Den root answers. */
function signedInBackend(url: string): Response {
	if (url.endsWith('/api/v1/me')) {
		return Response.json({ did: 'did:plc:alice', handle: 'alice.zurfur.app' });
	}
	return Response.json(DEN_ROOT_BODY);
}

/**
 * A full page load of `/den` through the real handle hook into the den load,
 * every backend call answered by `respond`; `den` is what the load handed the
 * page, when it got that far.
 */
async function requestDen(respond: (url: string) => Response) {
	const url = new URL('http://127.0.0.1:5174/den');
	const { fetch, calls } = fetchStub(respond);
	const event = {
		route: { id: '/(session)/den/[...path]' },
		url,
		request: new Request(url, { headers: { cookie: 'zurfur.sid=s3ss10n' } }),
		fetch,
		locals: {},
		isDataRequest: false
	} as unknown as RequestEvent;
	let den: DenPageData | undefined;
	const resolve = async (resolved: RequestEvent): Promise<Response> => {
		const data = (await load(resolved as unknown as LoadEvent)) as { den: DenPageData };
		den = data.den;
		return new Response('page');
	};
	const outcome: unknown = await Promise.resolve()
		.then(() => handle({ event, resolve }))
		.catch((thrown: unknown) => thrown);
	return { outcome, calls, den: () => den };
}

describe('/den on the live port, as a request', () => {
	it('signed in: the /me call, then the Den read, and the page gets its answer', async () => {
		const { outcome, calls, den } = await requestDen(signedInBackend);

		expect(outcome instanceof Response && outcome.status).toBe(200);
		const paths = calls.map((call) => new URL(call, 'http://127.0.0.1').pathname);
		expect(paths).toEqual(['/api/v1/me', '/api/v1/den']);
		expect(den()?.outcome).toMatchObject({ outcome: 'page', page: { view: 'open' } });
	});

	it('signed in: never stored, never framed, and its address never leaves the site', async () => {
		const { outcome } = await requestDen(signedInBackend);

		const headers = outcome instanceof Response ? outcome.headers : new Headers();
		expect(headers.get('cache-control')).toBe(SESSION_CACHE_CONTROL);
		expect(headers.get('referrer-policy')).toBe(SESSION_REFERRER_POLICY);
		expect(headers.get('content-security-policy')).toBe(FRAME_ANCESTORS_POLICY);
		expect(headers.get('x-frame-options')).toBe(X_FRAME_OPTIONS);
	});

	it('anonymous: sent to sign in, like every signed-in page, and no Den call', async () => {
		const { outcome, calls } = await requestDen(() => problemResponse(401, 'not_authenticated'));

		expect(isRedirect(outcome) && outcome.location).toBe('/login');
		expect(calls).toEqual(['/api/v1/me']);
	});
});
