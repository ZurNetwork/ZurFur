import { isRedirect, type HandleFetch, type RequestEvent } from '@sveltejs/kit';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { hangingFetch } from '$lib/testing/http';
import { kitEventFetch } from '$lib/testing/kit-fetch';
import { FRAME_ANCESTORS_POLICY, X_FRAME_OPTIONS } from '$lib/server/frame-headers';
import { SESSION_CACHE_CONTROL, SESSION_REFERRER_POLICY } from '$lib/server/session-headers';
import { UNANSWERED_PAGE } from '$lib/server/unanswered';
import { handle, handleError, handleFetch } from './hooks.server';

type HandleInput = Parameters<typeof handle>[0];

/** The app's origin; the API upstream (`127.0.0.1:8081`) shares its hostname, as in dev. */
const APP_ORIGIN = 'http://127.0.0.1:8080';

/** The visitor's whole cookie jar: the session cookie among unrelated ones. */
const VISITOR_COOKIE = 'foo=bar; zurfur.sid=abc; other=secret';

/** A `handleFetch` that passes every request on as it is: kit's own behaviour, unhooked. */
const passThrough: HandleFetch = ({ request, fetch }) => fetch(request);

/** `event.fetch` as SvelteKit builds it for a visitor carrying `VISITOR_COOKIE`. */
function eventFetch(hook: HandleFetch, locals: App.Locals = {}): typeof fetch {
	const url = new URL(`${APP_ORIGIN}/`);
	const request = new Request(url, { headers: { cookie: VISITOR_COOKIE } });
	return kitEventFetch({ url, request, locals }, hook);
}

/** Fetch `target` through `hook`; resolves to the request that left the server. */
async function requestLeaving(
	hook: HandleFetch,
	target: string,
	init?: RequestInit,
	locals: App.Locals = {}
): Promise<Request> {
	const sent: Request[] = [];
	vi.stubGlobal('fetch', (request: Request) => {
		sent.push(request);
		return Promise.resolve(new Response('{}'));
	});
	await eventFetch(hook, locals)(target, init);
	const [leaving] = sent;
	if (leaving === undefined) throw new Error('no request left the server');
	return leaving;
}

afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe('handle', () => {
	it('stamps a signed-in response private, no-store', async () => {
		const request = new Request('http://127.0.0.1:5174/', {
			headers: { cookie: 'zurfur.sid=s3ss10n' }
		});
		const event = { route: { id: '/' }, request, locals: {} } as unknown as RequestEvent;
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
		const event = {
			route: { id: '/(public)/login' },
			request,
			locals: {}
		} as unknown as RequestEvent;
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

describe('handle, when Zurfur doesn’t answer', () => {
	it('answers a signed-in page whose /me never comes with the gate’s page, stamped as signed in', async () => {
		vi.useFakeTimers();
		const request = new Request('http://127.0.0.1:5174/den', {
			headers: { cookie: 'zurfur.sid=s3ss10n' }
		});
		const event = {
			route: { id: '/(session)/den/[...path]' },
			url: new URL(request.url),
			request,
			fetch: hangingFetch().fetch,
			locals: {},
			isDataRequest: false
		} as unknown as RequestEvent;
		const resolve = vi.fn(() => Promise.resolve(new Response('PRIVATE-PAGE')));

		const answered = Promise.resolve(handle({ event, resolve }));
		await vi.advanceTimersByTimeAsync(5_000);
		const response = await answered;

		expect(response.status).toBe(503);
		expect(await response.text()).toBe(UNANSWERED_PAGE);
		expect(response.headers.get('cache-control')).toBe(SESSION_CACHE_CONTROL);
		expect(response.headers.get('referrer-policy')).toBe(SESSION_REFERRER_POLICY);
		expect(response.headers.get('content-security-policy')).toBe(FRAME_ANCESTORS_POLICY);
		expect(response.headers.get('x-frame-options')).toBe(X_FRAME_OPTIONS);
		expect(response.headers.has('set-cookie')).toBe(false);
		expect(resolve).not.toHaveBeenCalled();
	});
});

describe('handle, when Zurfur doesn’t answer a request with no session cookie', () => {
	it('still answers the gate’s page never stored, and sending no Referer off the site', async () => {
		const request = new Request('http://127.0.0.1:5174/accounts');
		const event = {
			route: { id: '/(session)/accounts' },
			url: new URL(request.url),
			request,
			fetch: () => Promise.reject(new TypeError('fetch failed')),
			locals: {},
			isDataRequest: false
		} as unknown as RequestEvent;
		const resolve = vi.fn(() => Promise.resolve(new Response('PRIVATE-PAGE')));

		const response = await handle({ event, resolve });

		expect(response.status).toBe(503);
		expect(response.headers.get('cache-control')).toBe('no-store');
		expect(response.headers.get('referrer-policy')).toBe(SESSION_REFERRER_POLICY);
		expect(response.headers.get('x-frame-options')).toBe(X_FRAME_OPTIONS);
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

	it('stops the request leaving the server when the call’s own limit runs out', async () => {
		const limit = new AbortController();
		const deadline = new AbortController();

		const leaving = await requestLeaving(
			handleFetch,
			'/api/v1/me',
			{ signal: limit.signal },
			{ deadline: deadline.signal }
		);
		limit.abort(new DOMException('limit', 'TimeoutError'));

		expect(leaving.signal.aborted).toBe(true);
		expect((leaving.signal.reason as DOMException).name).toBe('TimeoutError');
	});

	it('stops the request leaving the server when the request’s deadline passes', async () => {
		const deadline = new AbortController();

		const leaving = await requestLeaving(handleFetch, '/api/v1/den', undefined, {
			deadline: deadline.signal
		});
		deadline.abort(new DOMException('deadline', 'TimeoutError'));

		expect(leaving.signal.aborted).toBe(true);
		expect((leaving.signal.reason as DOMException).message).toBe('deadline');
	});
});
