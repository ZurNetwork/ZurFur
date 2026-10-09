import { isRedirect, type HandleFetch, type RequestEvent } from '@sveltejs/kit';
import { afterEach, describe, expect, it, vi } from 'vitest';
// SvelteKit's real server-side `event.fetch`, reached through its internals on
// purpose: which cookies it re-attaches is source-only behaviour, so a kit bump
// that changes `create_fetch` must break this spec loudly.
// @ts-expect-error -- kit internal, untyped
import { create_fetch } from '../node_modules/@sveltejs/kit/src/runtime/server/fetch.js';
// @ts-expect-error -- kit internal, untyped
import { get_cookies } from '../node_modules/@sveltejs/kit/src/runtime/server/cookie.js';
import { handle, handleError, handleFetch } from './hooks.server';

type HandleInput = Parameters<typeof handle>[0];

/** The app's origin; the API upstream (`127.0.0.1:8081`) shares its hostname, as in dev. */
const APP_ORIGIN = 'http://127.0.0.1:8080';

/** The visitor's whole cookie jar: the session cookie among unrelated ones. */
const VISITOR_COOKIE = 'foo=bar; zurfur.sid=abc; other=secret';

/** A `handleFetch` that passes every request on as it is: kit's own behaviour, unhooked. */
const passThrough: HandleFetch = ({ request, fetch }) => fetch(request);

/** `event.fetch` as SvelteKit builds it for a visitor carrying `VISITOR_COOKIE`. */
function eventFetch(hook: HandleFetch): typeof fetch {
	const url = new URL(`${APP_ORIGIN}/`);
	const request = new Request(url, { headers: { cookie: VISITOR_COOKIE } });
	/* eslint-disable @typescript-eslint/no-unsafe-assignment, @typescript-eslint/no-unsafe-call, @typescript-eslint/no-unsafe-return -- kit internals are untyped */
	const { get_cookie_header, set_internal, set_trailing_slash } = get_cookies(request, url);
	set_trailing_slash('never');
	return create_fetch({
		event: { url, request },
		options: { hooks: { handleFetch: hook } },
		manifest: { assets: new Set(), _: { server_assets: {} }, mimeTypes: {} },
		state: { depth: 0 },
		get_cookie_header,
		set_internal
	});
	/* eslint-enable @typescript-eslint/no-unsafe-assignment, @typescript-eslint/no-unsafe-call, @typescript-eslint/no-unsafe-return */
}

/** Fetch `target` through `hook`; resolves to the request that left the server. */
async function requestLeaving(hook: HandleFetch, target: string): Promise<Request> {
	const sent: Request[] = [];
	vi.stubGlobal('fetch', (request: Request) => {
		sent.push(request);
		return Promise.resolve(new Response('{}'));
	});
	await eventFetch(hook)(target);
	const [leaving] = sent;
	if (leaving === undefined) throw new Error('no request left the server');
	return leaving;
}

afterEach(() => {
	vi.unstubAllGlobals();
});

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
		expect(response.headers.get('referrer-policy')).toBe('same-origin');
		expect(response.headers.get('x-frame-options')).toBe('DENY');
		expect(response.headers.get('content-security-policy')).toBe("frame-ancestors 'none'");
	});

	it('refuses framing on an anonymous response too', async () => {
		const request = new Request('http://127.0.0.1:5174/login');
		const event = { route: { id: '/(public)/login' }, request } as unknown as RequestEvent;
		const resolve = () => Promise.resolve(new Response('<p>sign in</p>'));
		const response = await handle({ event, resolve });

		expect(response.headers.get('x-frame-options')).toBe('DENY');
		expect(response.headers.has('cache-control')).toBe(false);
	});

	it('is wired to the sign-in gate: a crafted (session) data request never resolves', async () => {
		const request = new Request(
			'http://127.0.0.1:5174/accounts/__data.json?x-sveltekit-invalidated=001'
		);
		const anonymousMe = () =>
			Promise.resolve(
				new Response(
					JSON.stringify({
						type: 'urn:zurfur:error:not-authenticated',
						code: 'not_authenticated',
						title: 'x',
						detail: 'x',
						status: 401
					}),
					{ status: 401, headers: { 'content-type': 'application/problem+json' } }
				)
			);
		const event = {
			route: { id: '/(session)/accounts' },
			url: new URL(request.url),
			request,
			fetch: anonymousMe,
			locals: {}
		} as unknown as RequestEvent;
		const resolve = vi.fn(() => Promise.resolve(new Response('UNGUARDED-DATA')));

		const outcome: unknown = await Promise.resolve()
			.then(() => handle({ event, resolve }))
			.catch((thrown: unknown) => thrown);

		expect(isRedirect(outcome) && outcome.location).toBe('/login');
		expect(resolve).not.toHaveBeenCalled();
	});
});

describe('handleError', () => {
	it('is wired to log the route template only and return a fixed message', async () => {
		const errorLog = vi.spyOn(console, 'error').mockImplementation(() => undefined);
		const event = {
			route: { id: '/(session)/den/[...path]' },
			url: new URL('http://127.0.0.1:5174/den/accounts/did:plc:leak')
		} as unknown as RequestEvent;
		const input: Parameters<typeof handleError>[0] = {
			error: new Error('did:plc:leak'),
			event,
			status: 500,
			message: 'Internal Error'
		};

		const result = await handleError(input);
		const logged = JSON.stringify(errorLog.mock.calls);
		errorLog.mockRestore();

		expect(result).toEqual({ message: 'Internal Error' });
		expect(logged).not.toContain('did:plc:leak');
		expect(logged).toContain('/(session)/den/[...path]');
	});
});

describe('handleFetch under SvelteKit’s server fetch', () => {
	// Control: proves the harness reproduces kit's re-attach, so the cases below
	// pass because of handleFetch and not because kit stopped doing it.
	it('kit re-attaches the whole jar to an unhooked request on the app’s hostname', async () => {
		const leaving = await requestLeaving(passThrough, 'http://127.0.0.1:8081/me');

		expect(leaving.headers.get('cookie')).toBe(VISITOR_COOKIE);
	});

	it('sends only zurfur.sid with an /api/v1 call', async () => {
		const leaving = await requestLeaving(handleFetch, '/api/v1/me');

		expect(leaving.url).toBe('http://127.0.0.1:8081/me');
		expect(leaving.headers.get('cookie')).toBe('zurfur.sid=abc');
	});

	it('sends no cookie with a cross-origin request on the app’s hostname', async () => {
		const leaving = await requestLeaving(handleFetch, 'http://127.0.0.1:8081/me');

		expect(leaving.url).toBe('http://127.0.0.1:8081/me');
		expect(leaving.headers.get('cookie')).toBeNull();
	});
});
