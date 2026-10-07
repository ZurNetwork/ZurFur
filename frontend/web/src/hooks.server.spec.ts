import type { RequestEvent } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { handle } from './hooks.server';

type HandleInput = Parameters<typeof handle>[0];

describe('handle', () => {
	it('stamps a signed-in response private, no-store', async () => {
		const request = new Request('http://127.0.0.1:5174/', {
			headers: { cookie: 'zurfur.sid=s3ss10n' }
		});
		const event = { route: { id: '/' }, request } as unknown as RequestEvent;
		const resolve = () => Promise.resolve(new Response('<p>private</p>'));
		const input: HandleInput = { event, resolve };

		const response = await handle(input);

		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});
});
