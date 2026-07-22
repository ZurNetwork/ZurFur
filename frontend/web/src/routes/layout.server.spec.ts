import { describe, expect, it } from 'vitest';
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
		const fetch = (async () => Response.json(me)) as typeof globalThis.fetch;
		const result = await load(layoutEvent(fetch));
		expect(result).toEqual({ session: me });
	});

	it('carries null for an anonymous visitor (backend 401)', async () => {
		const problem = {
			type: 'urn:zurfur:error:not-authenticated',
			code: 'not_authenticated',
			title: 'Not authenticated',
			status: 401
		};
		const fetch = (async () =>
			new Response(JSON.stringify(problem), {
				status: 401,
				headers: { 'content-type': 'application/problem+json' }
			})) as typeof globalThis.fetch;
		const result = await load(layoutEvent(fetch));
		expect(result).toEqual({ session: null });
	});

	it('degrades to signed-out when the backend is unreachable', async () => {
		const deadFetch = (async () => {
			throw new TypeError('fetch failed');
		}) as typeof globalThis.fetch;
		const result = await load(layoutEvent(deadFetch));
		expect(result).toEqual({ session: null });
	});
});
