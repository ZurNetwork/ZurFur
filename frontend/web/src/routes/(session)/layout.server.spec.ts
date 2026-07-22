import { describe, expect, it } from 'vitest';
import { isRedirect } from '@sveltejs/kit';
import { load } from './+layout.server';

type LoadEvent = Parameters<typeof load>[0];

function guardEvent(session: unknown): LoadEvent {
	return { parent: async () => ({ session }) } as unknown as LoadEvent;
}

describe('(session) guard', () => {
	it('bounces an anonymous visit to /login', async () => {
		try {
			await load(guardEvent(null));
		} catch (thrown) {
			if (!isRedirect(thrown)) throw thrown;
			expect(thrown.status).toBe(303);
			expect(thrown.location).toBe('/login');
			return;
		}
		throw new Error('expected the guard to redirect');
	});

	it('passes a signed-in visit through', async () => {
		const signedIn = { did: 'did:plc:alice', handle: null, display_name: null, avatar_url: null };
		await expect(load(guardEvent(signedIn))).resolves.toEqual({});
	});
});
