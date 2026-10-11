import type { Handle, RequestEvent } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { handleChain } from './handle-chain';

/** A hook that records its name on the way in and stamps a header on the way out. */
function marking(name: string, seen: string[]): Handle {
	return async ({ event, resolve }) => {
		seen.push(name);
		const response = await resolve(event);
		const stamped = new Response(response.body, response);
		stamped.headers.append('x-order', name);
		return stamped;
	};
}

describe('handleChain', () => {
	it('runs the hooks first-outermost, then SvelteKit’s resolve', async () => {
		const seen: string[] = [];
		const handle = handleChain(marking('a', seen), marking('b', seen));
		const event = {} as RequestEvent;

		const response = await handle({
			event,
			resolve: () => {
				seen.push('resolve');
				return Promise.resolve(new Response('ok'));
			}
		});

		expect(seen).toEqual(['a', 'b', 'resolve']);
		expect(response.headers.get('x-order')).toBe('b, a');
	});

	it('stops when a hook doesn’t resolve', async () => {
		const short: Handle = () => new Response('stopped');
		const handle = handleChain(short);
		const response = await handle({
			event: {} as RequestEvent,
			resolve: () => Promise.reject(new Error('must not resolve'))
		});
		expect(await response.text()).toBe('stopped');
	});
});
