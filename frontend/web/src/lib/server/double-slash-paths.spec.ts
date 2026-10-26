import type { RequestEvent } from '@sveltejs/kit';
import { describe, expect, it, vi } from 'vitest';
import { DOUBLE_SLASH_REFUSAL, refuseDoubleSlashPaths } from './double-slash-paths';

/** The site's origin; each path is appended to it as text, the way adapter-node builds the URL. */
const SITE = 'http://127.0.0.1:8080';

/** What the hook did with a request: its response, and whether anything behind it ran. */
interface Outcome {
	readonly response: Response;
	readonly resolved: boolean;
}

/** Put a `method` request for `path` through the hook, with SvelteKit's resolve stubbed. */
async function visit(path: string, method = 'GET'): Promise<Outcome> {
	const url = `${SITE}${path}`;
	const init: RequestInit =
		method === 'GET'
			? { method }
			: {
					method,
					headers: { 'content-type': 'application/x-www-form-urlencoded', origin: SITE },
					body: 'title=x'
				};
	const request = new Request(url, init);
	const event = { url: new URL(url), request } as unknown as RequestEvent;
	const resolve = vi.fn(() => Promise.resolve(new Response('RESOLVED', { status: 200 })));

	const response = await refuseDoubleSlashPaths({ event, resolve });

	return { response, resolved: resolve.mock.calls.length > 0 };
}

describe('refuseDoubleSlashPaths: refused', () => {
	it.each(['//x', '//den/x', '//evil.example/x', '/\\evil.example/x'])(
		'answers GET %s with a plain-text 404 and never resolves',
		async (path) => {
			const outcome = await visit(path);

			expect(outcome.response.status).toBe(404);
			expect(outcome.response.headers.get('content-type')).toBe('text/plain');
			expect(await outcome.response.text()).toBe(DOUBLE_SLASH_REFUSAL);
			expect(outcome.resolved).toBe(false);
		}
	);

	it('answers a first-party form action posted to //evil.example/den/x with the 404', async () => {
		const outcome = await visit('//evil.example/den/x?/createCommission', 'POST');

		expect(outcome.response.status).toBe(404);
		expect(await outcome.response.text()).toBe(DOUBLE_SLASH_REFUSAL);
		expect(outcome.resolved).toBe(false);
	});
});

describe('refuseDoubleSlashPaths: let through', () => {
	it.each(['/', '/den/x', '/den//x', '/den/x?/createCommission'])(
		'resolves %s untouched',
		async (path) => {
			const outcome = await visit(path);

			expect(outcome.resolved).toBe(true);
			expect(await outcome.response.text()).toBe('RESOLVED');
		}
	);
});
