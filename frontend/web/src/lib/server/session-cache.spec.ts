import type { RequestEvent } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { sessionNoStore } from './session-cache';

type HandleInput = Parameters<typeof sessionNoStore>[0];

/** Builds the response a stubbed `resolve` answers with (fresh per call: bodies are single-use). */
type Respond = () => Response;

/** The request a visitor sends: the route SvelteKit matched (`null` = none, a 404) and its cookie header. */
interface Visit {
	routeId: string | null;
	cookie: string | undefined;
}

/** A signed-in visitor's cookie header: the session cookie among unrelated ones. */
const SIGNED_IN_COOKIE = 'theme=dark; zurfur.sid=s3ss10n==; other=1';

/** An anonymous visitor who still carries unrelated cookies for this host. */
const ANONYMOUS_COOKIE = 'theme=dark; other=1';

/** Run the hook for `visit`, with SvelteKit's resolve answering `respond()`. */
async function handleFor(visit: Visit, respond: Respond): Promise<Response> {
	const headers = new Headers();
	if (visit.cookie !== undefined) headers.set('cookie', visit.cookie);
	const request = new Request('http://127.0.0.1:5174/', { headers });
	const event = { route: { id: visit.routeId }, request } as unknown as RequestEvent;
	const resolve = () => Promise.resolve(respond());
	const input: HandleInput = { event, resolve };
	return await sessionNoStore(input);
}

/** A signed-in visit to `routeId`. */
function signedIn(routeId: string | null): Visit {
	return { routeId, cookie: SIGNED_IN_COOKIE };
}

/** A rendered HTML page, as SvelteKit's resolve hands it back. */
function pageResponse(): Response {
	return new Response('<p>private</p>', { headers: { 'content-type': 'text/html' } });
}

/** SvelteKit's rendered 404 page. */
function notFoundResponse(): Response {
	return new Response('<p>Not found</p>', {
		status: 404,
		headers: { 'content-type': 'text/html' }
	});
}

describe('sessionNoStore', () => {
	// The root layout renders the visitor's handle, DID and avatar on every
	// page, so a signed-in response is private wherever it lands.
	it('marks a signed-in / private, no-store', async () => {
		const response = await handleFor(signedIn('/'), pageResponse);

		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	it('marks a signed-in 404 private, no-store', async () => {
		const response = await handleFor(signedIn(null), notFoundResponse);

		expect(response.status).toBe(404);
		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	it('marks a signed-in error page private, no-store', async () => {
		const serverError = () => new Response('<p>Internal Error</p>', { status: 500 });

		const response = await handleFor(signedIn('/(session)/logout'), serverError);

		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	// SvelteKit answers a `use:enhance` form action with JSON straight from the
	// action — no load runs, so a layout `setHeaders` never reaches it.
	it('marks a signed-in enhanced action JSON response', async () => {
		const actionJson = () =>
			new Response(JSON.stringify({ type: 'failure', status: 422 }), {
				status: 200,
				headers: { 'content-type': 'application/json' }
			});

		const response = await handleFor(signedIn('/(session)/accounts'), actionJson);

		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	it('marks a signed-in action redirect', async () => {
		const seeOther = () => new Response(undefined, { status: 303, headers: { location: '/' } });

		const response = await handleFor(signedIn('/(session)/logout'), seeOther);

		expect(response.status).toBe(303);
		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	it('overrides a weaker cache-control a signed-in page set', async () => {
		const cacheable = () =>
			new Response('<p>private</p>', { headers: { 'cache-control': 'public, max-age=60' } });

		const response = await handleFor(signedIn('/'), cacheable);

		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	// A `+server.ts` may return a `Response.redirect()` or a `fetch()` result
	// as-is; their headers are immutable, so the hook must stamp a copy.
	it('stamps a signed-in response whose headers are immutable', async () => {
		const immutableRedirect = () => Response.redirect('http://127.0.0.1:5174/', 303);

		const response = await handleFor(signedIn('/mock/signin'), immutableRedirect);

		expect(response.status).toBe(303);
		expect(response.headers.get('location')).toBe('http://127.0.0.1:5174/');
		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	it('keeps every Set-Cookie and the body through the stamp', async () => {
		const signOut = () => {
			const headers = new Headers({ location: '/' });
			headers.append('set-cookie', 'zurfur.sid=; Max-Age=0; Path=/');
			headers.append('set-cookie', 'other=; Max-Age=0; Path=/');
			return new Response('bye', { status: 303, headers });
		};

		const response = await handleFor(signedIn('/(session)/logout'), signOut);

		expect(response.headers.getSetCookie()).toEqual([
			'zurfur.sid=; Max-Age=0; Path=/',
			'other=; Max-Age=0; Path=/'
		]);
		expect(await response.text()).toBe('bye');
		expect(response.headers.get('cache-control')).toBe('private, no-store');
	});

	it('leaves an anonymous / alone', async () => {
		const response = await handleFor({ routeId: '/', cookie: undefined }, pageResponse);

		expect(response.headers.has('cache-control')).toBe(false);
	});

	it('leaves an anonymous visitor with unrelated cookies alone', async () => {
		const response = await handleFor({ routeId: '/login', cookie: ANONYMOUS_COOKIE }, pageResponse);

		expect(response.headers.has('cache-control')).toBe(false);
	});

	it('keeps a page’s own cache-control for an anonymous visitor', async () => {
		const cacheable = () =>
			new Response('<p>public</p>', { headers: { 'cache-control': 'public, max-age=60' } });

		const response = await handleFor({ routeId: '/', cookie: undefined }, cacheable);

		expect(response.headers.get('cache-control')).toBe('public, max-age=60');
	});

	it('reads the session cookie by name, not by prefix', async () => {
		const lookalike = { routeId: '/', cookie: 'zurfur.sidx=1; xzurfur.sid=2' };

		const response = await handleFor(lookalike, pageResponse);

		expect(response.headers.has('cache-control')).toBe(false);
	});
});
