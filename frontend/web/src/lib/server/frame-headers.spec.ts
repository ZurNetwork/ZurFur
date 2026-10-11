import type { RequestEvent } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { refuseFraming } from './frame-headers';

type HandleInput = Parameters<typeof refuseFraming>[0];

/** Run the hook with SvelteKit's resolve answering `respond()`. */
async function stamp(respond: () => Response): Promise<Response> {
	const event = {
		route: { id: '/' },
		request: new Request('http://127.0.0.1:5174/')
	} as unknown as RequestEvent;
	const input: HandleInput = { event, resolve: () => Promise.resolve(respond()) };
	return await refuseFraming(input);
}

describe('refuseFraming', () => {
	it.each([
		['an anonymous page', () => new Response('<p>hi</p>')],
		['a 404', () => new Response('nope', { status: 404 })],
		['a redirect with immutable headers', () => Response.redirect('http://127.0.0.1:5174/', 303)],
		['page data', () => Response.json({ type: 'data' })]
	])('refuses framing on %s', async (_, respond) => {
		const response = await stamp(respond);
		expect(response.headers.get('content-security-policy')).toBe("frame-ancestors 'none'");
		expect(response.headers.get('x-frame-options')).toBe('DENY');
	});
});
