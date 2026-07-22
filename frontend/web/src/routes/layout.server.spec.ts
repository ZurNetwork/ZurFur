import { describe, expect, it } from 'vitest';
import { fetchStub, problemResponse, unreachableFetch } from '$lib/testing/http';
import { load } from './+layout.server';

type LoadEvent = Parameters<typeof load>[0];

function layoutEvent(fetch: typeof globalThis.fetch): LoadEvent {
	return { fetch } as unknown as LoadEvent;
}

describe('root layout load', () => {
	it('carries the session for a signed-in visitor', async () => {
		const me = {
			did: 'did:plc:alice',
			handle: 'alice.zurfur.app',
			display_name: 'Alice',
			avatar_url: 'https://cdn.example/alice.jpg'
		};
		const { fetch } = fetchStub(() => Response.json(me));
		const result = await load(layoutEvent(fetch));
		expect(result).toEqual({ session: me });
	});

	it('carries null for an anonymous visitor (backend 401)', async () => {
		const { fetch } = fetchStub(() => problemResponse(401, 'not_authenticated'));
		const result = await load(layoutEvent(fetch));
		expect(result).toEqual({ session: null });
	});

	it('degrades to signed-out when the backend is unreachable', async () => {
		const result = await load(layoutEvent(unreachableFetch()));
		expect(result).toEqual({ session: null });
	});

	it('surfaces a broken contract instead of treating it as signed-out', async () => {
		const { fetch } = fetchStub(() => new Response('gateway timeout', { status: 504 }));
		await expect(load(layoutEvent(fetch))).rejects.toThrow(/contract violation/);
	});
});
