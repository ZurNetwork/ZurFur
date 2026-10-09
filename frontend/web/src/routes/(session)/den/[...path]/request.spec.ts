import { isHttpError, isRedirect, type RequestEvent } from '@sveltejs/kit';
import { describe, expect, it, vi } from 'vitest';
import { fetchStub, problemResponse } from '$lib/testing/http';

/** The den program, replaced so the spec can see whether the Den port is ever asked. */
const den = vi.hoisted(() => ({ denPage: vi.fn() }));

vi.mock('$lib/server/den', () => den);

const { handle } = await import('../../../../hooks.server');
const { load } = await import('./+page.server');

type LoadEvent = Parameters<typeof load>[0];

/** A request for `/den` on the everyday stack, through the real handle hook into the den load. */
async function requestDen(me: () => Response) {
	const url = new URL('http://127.0.0.1:5174/den');
	const { fetch, calls } = fetchStub(me);
	const event = {
		route: { id: '/(session)/den/[...path]' },
		url,
		request: new Request(url, { headers: { cookie: 'zurfur.sid=s3ss10n' } }),
		fetch,
		locals: {},
		isDataRequest: false
	} as unknown as RequestEvent;
	const resolve = async (resolved: RequestEvent): Promise<Response> => {
		try {
			await load(resolved as unknown as LoadEvent);
			return new Response('page');
		} catch (thrown) {
			if (isHttpError(thrown)) return new Response(null, { status: thrown.status });
			throw thrown;
		}
	};
	const outcome: unknown = await Promise.resolve()
		.then(() => handle({ event, resolve }))
		.catch((thrown: unknown) => thrown);
	return { outcome, calls };
}

describe('/den on the everyday stack, as a request', () => {
	it('signed in: the one /me call every signed-in page makes, then the 404, and no Den call', async () => {
		den.denPage.mockClear();
		const { outcome, calls } = await requestDen(() =>
			Response.json({ did: 'did:plc:alice', handle: 'alice.zurfur.app' })
		);

		expect(outcome).toBeInstanceOf(Response);
		expect(outcome instanceof Response && outcome.status).toBe(404);
		expect(calls).toEqual(['/api/v1/me']);
		expect(den.denPage).not.toHaveBeenCalled();
	});

	it('anonymous: sent to sign in, like every signed-in page, and no Den call', async () => {
		den.denPage.mockClear();
		const { outcome } = await requestDen(() => problemResponse(401, 'not_authenticated'));

		expect(isRedirect(outcome) && outcome.location).toBe('/login');
		expect(den.denPage).not.toHaveBeenCalled();
	});
});
