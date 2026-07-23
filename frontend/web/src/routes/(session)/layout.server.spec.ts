import { describe, expect, it } from 'vitest';
import { expectRedirect } from '$lib/testing/redirect';
import { load } from './+layout.server';

type LoadEvent = Parameters<typeof load>[0];

function guardEvent(session: unknown): LoadEvent {
	return { parent: async () => ({ session }) } as unknown as LoadEvent;
}

describe('(session) guard', () => {
	it('bounces an anonymous visit to /login', async () => {
		const redirect = await expectRedirect(() => load(guardEvent(null)));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/login');
	});

	it('passes a signed-in visit through', async () => {
		const signedIn = { did: 'did:plc:alice', handle: null, display_name: null, avatar_url: null };
		await expect(load(guardEvent(signedIn))).resolves.toEqual({});
	});
});
