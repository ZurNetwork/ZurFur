import { describe, expect, it } from 'vitest';
import { fetchStub, problemResponse, unreachableFetch } from '$lib/testing/http';
import { load } from './+layout.server';

type LoadEvent = Parameters<typeof load>[0];

function layoutEvent(fetch: typeof globalThis.fetch): LoadEvent {
	return { fetch, locals: {} } as unknown as LoadEvent;
}

describe('root layout load', () => {
	it('carries the session for a signed-in visitor', async () => {
		const me = {
			did: 'did:plc:alice',
			handle: 'alice.zurfur.app',
			displayName: 'Alice',
			avatarUrl: 'https://cdn.example/alice.jpg'
		};
		const { fetch } = fetchStub(() => Response.json(me));
		const result = await load(layoutEvent(fetch));
		expect(result).toEqual({ session: me, denServed: false });
	});

	it('carries undefined for an anonymous visitor (backend 401)', async () => {
		const { fetch } = fetchStub(() => problemResponse(401, 'not_authenticated'));
		const result = await load(layoutEvent(fetch));
		expect(result).toStrictEqual({ session: undefined, denServed: false });
	});

	it('degrades to signed-out when the backend is unreachable', async () => {
		const result = await load(layoutEvent(unreachableFetch()));
		expect(result).toStrictEqual({ session: undefined, denServed: false });
	});

	it('surfaces a broken contract instead of treating it as signed-out', async () => {
		const { fetch } = fetchStub(() => new Response('gateway timeout', { status: 504 }));
		await expect(load(layoutEvent(fetch))).rejects.toThrow(/contract violation/);
	});

	it('reuses the session the sign-in gate already found, without asking again', async () => {
		const { fetch, calls } = fetchStub(() => problemResponse(401, 'not_authenticated'));
		const gated = { did: 'did:plc:alice' };
		const event = { fetch, locals: { session: gated } } as unknown as LoadEvent;
		const result = await load(event);
		expect(result).toEqual({ session: gated, denServed: false });
		expect(calls).toEqual([]);
	});
});
