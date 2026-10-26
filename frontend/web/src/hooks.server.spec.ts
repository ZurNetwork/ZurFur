import { isRedirect, type HandleFetch, type RequestEvent } from '@sveltejs/kit';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { hangingFetch } from '$lib/testing/http';
import { kitEventFetch } from '$lib/testing/kit-fetch';
import { DOUBLE_SLASH_REFUSAL } from '$lib/server/double-slash-paths';
import { FRAME_ANCESTORS_POLICY, X_FRAME_OPTIONS } from '$lib/server/frame-headers';
import { SESSION_CACHE_CONTROL, SESSION_REFERRER_POLICY } from '$lib/server/session-headers';
import { UNANSWERED_PAGE } from '$lib/server/unanswered';
import { handle, handleError, handleFetch } from './hooks.server';

// The run-time env starts empty, so no case reads the ZURFUR_* values the gate exports from `.env`.
vi.mock('$env/dynamic/private', () => ({ env: {} }));

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
	return kitEventFetch({ url, request, locals, route: { id: '/' } }, hook);
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
		const event = {
			route: { id: '/' },
			url: new URL(request.url),
			request,
			locals: {}
		} as unknown as RequestEvent;
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
			url: new URL(request.url),
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

describe('handle, refusing double-slash paths', () => {
	/** A request for `//evil.example/x`, carrying `cookie` when given. */
	function doubleSlashEvent(cookie?: string): RequestEvent {
		const url = 'http://127.0.0.1:5174//evil.example/x';
		const init: RequestInit = cookie === undefined ? {} : { headers: { cookie } };
		const request = new Request(url, init);
		return {
			route: { id: null },
			url: new URL(url),
			request,
			locals: {}
		} as unknown as RequestEvent;
	}

	it('is wired to the double-slash guard: //evil.example/x gets the framing-stamped 404 and never resolves', async () => {
		const event = doubleSlashEvent();
		const resolve = vi.fn(() => Promise.resolve(new Response('RESOLVED')));

		const response = await handle({ event, resolve });

		expect(response.status).toBe(404);
		expect(await response.text()).toBe(DOUBLE_SLASH_REFUSAL);
		expect(response.headers.get('x-frame-options')).toBe(X_FRAME_OPTIONS);
		expect(response.headers.get('content-security-policy')).toBe(FRAME_ANCESTORS_POLICY);
		expect(resolve).not.toHaveBeenCalled();
	});

	it('stamps the 404 to a signed-in request private, no-store and never resolves', async () => {
		const event = doubleSlashEvent('zurfur.sid=s3ss10n');
		const resolve = vi.fn(() => Promise.resolve(new Response('RESOLVED')));

		const response = await handle({ event, resolve });

		expect(response.status).toBe(404);
		expect(await response.text()).toBe(DOUBLE_SLASH_REFUSAL);
		expect(response.headers.get('x-frame-options')).toBe(X_FRAME_OPTIONS);
		expect(response.headers.get('content-security-policy')).toBe(FRAME_ANCESTORS_POLICY);
		expect(response.headers.get('cache-control')).toBe(SESSION_CACHE_CONTROL);
		expect(response.headers.get('referrer-policy')).toBe(SESSION_REFERRER_POLICY);
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

/** The two facts the origin check is configured from: the build, and ZURFUR_PUBLIC_URL. */
interface OriginEnv {
	readonly dev: boolean;
	readonly publicUrl: string | undefined;
}

/**
 * Load `hooks.server` fresh with `$app/environment` and `$env/dynamic/private`
 * stubbed to `stub`, so the origin it reads at load is the case's own, never
 * the `.env` the gate exports into the test run.
 */
async function hooksUnder(stub: OriginEnv): Promise<typeof import('./hooks.server')> {
	vi.resetModules();
	vi.doMock('$app/environment', () => ({
		dev: stub.dev,
		building: false,
		browser: false,
		version: 'test'
	}));
	vi.doMock('$env/dynamic/private', () => ({ env: { ZURFUR_PUBLIC_URL: stub.publicUrl } }));
	return await import('./hooks.server');
}

/** A form post to a page, as the browser sends it. */
interface Post {
	readonly url: string;
	readonly routeId: string;
	readonly origin: string;
	readonly cookie?: string;
}

/** What `handle` did with a post: its response, and what behind it ran. */
interface PostOutcome {
	readonly response: Response;
	readonly resolved: boolean;
	readonly fetched: boolean;
}

/** Put `post` through `hook`, with SvelteKit's resolve and `event.fetch` stubbed. */
async function postThrough(hook: typeof handle, post: Post): Promise<PostOutcome> {
	const headers = new Headers({
		'content-type': 'application/x-www-form-urlencoded',
		origin: post.origin
	});
	if (post.cookie !== undefined) headers.set('cookie', post.cookie);
	const request = new Request(post.url, { method: 'POST', headers, body: 'x=1' });
	const fetchSpy = vi.fn(() => Promise.reject(new TypeError('fetch failed')));
	const event = {
		route: { id: post.routeId },
		url: new URL(post.url),
		request,
		fetch: fetchSpy,
		locals: {},
		isDataRequest: false
	} as unknown as RequestEvent;
	const resolve = vi.fn(() => Promise.resolve(new Response('RESOLVED')));

	const response = await hook({ event, resolve });

	return {
		response,
		resolved: resolve.mock.calls.length > 0,
		fetched: fetchSpy.mock.calls.length > 0
	};
}

/** A sign-in form post arriving at `url` from `origin`. */
function loginPost(origin: string, url = 'http://127.0.0.1:5174/login'): Post {
	return { url, routeId: '/(public)/login', origin };
}

/** How long the one cold import may take on a loaded machine. */
const WARM_IMPORT_TIMEOUT_MS = 60_000;

describe('handle, refusing cross-site writes', () => {
	// Pay the cold transform of the module graph once, under its own timeout, so
	// each test's fresh import after `vi.resetModules()` is only an evaluation.
	beforeAll(async () => {
		await import('./hooks.server');
	}, WARM_IMPORT_TIMEOUT_MS);

	afterEach(() => {
		vi.doUnmock('$app/environment');
		vi.doUnmock('$env/dynamic/private');
		vi.resetModules();
		vi.restoreAllMocks();
	});

	/** Silence the dev request log a dev build writes for every request. */
	function quietDevLog(): void {
		vi.spyOn(console, 'debug').mockImplementation(() => undefined);
	}

	it('in dev with ZURFUR_PUBLIC_URL unset, takes http://127.0.0.1:8080 as the origin', async () => {
		quietDevLog();
		const hooks = await hooksUnder({ dev: true, publicUrl: undefined });

		const fromDefault = await postThrough(hooks.handle, loginPost('http://127.0.0.1:8080'));
		const fromVite = await postThrough(hooks.handle, loginPost('http://127.0.0.1:5174'));

		expect(fromDefault.resolved).toBe(true);
		expect(fromVite.response.status).toBe(403);
		expect(fromVite.resolved).toBe(false);
	});

	it('outside dev with ZURFUR_PUBLIC_URL unset, refuses every write', async () => {
		const hooks = await hooksUnder({ dev: false, publicUrl: undefined });

		const outcome = await postThrough(
			hooks.handle,
			loginPost('http://127.0.0.1:8080', 'http://127.0.0.1:8080/login')
		);

		expect(outcome.response.status).toBe(403);
		expect(outcome.resolved).toBe(false);
	});

	it('takes ZURFUR_PUBLIC_URL when it is set, not the dev default', async () => {
		quietDevLog();
		const hooks = await hooksUnder({ dev: true, publicUrl: 'http://127.0.0.1:9180' });

		const fromConfigured = await postThrough(hooks.handle, loginPost('http://127.0.0.1:9180'));
		const fromDefault = await postThrough(hooks.handle, loginPost('http://127.0.0.1:8080'));

		expect(fromConfigured.resolved).toBe(true);
		expect(fromDefault.response.status).toBe(403);
		expect(fromDefault.resolved).toBe(false);
	});

	it.each(['http://127.0.0.1:8080/?', 'http://127.0.0.1:8080/app', 'not a url', ''])(
		'with a malformed ZURFUR_PUBLIC_URL (%j), refuses every write, even in dev',
		async (malformed) => {
			quietDevLog();
			const hooks = await hooksUnder({ dev: true, publicUrl: malformed });

			const fromDefault = await postThrough(hooks.handle, loginPost('http://127.0.0.1:8080'));

			expect(fromDefault.response.status).toBe(403);
			expect(fromDefault.resolved).toBe(false);
		}
	);

	it('refuses a cross-origin sign-in post in a dev build too', async () => {
		quietDevLog();
		const hooks = await hooksUnder({ dev: true, publicUrl: 'http://127.0.0.1:8080' });

		const outcome = await postThrough(hooks.handle, loginPost('https://evil.example'));

		expect(outcome.response.status).toBe(403);
		expect(await outcome.response.text()).toBe('Cross-site POST form submissions are forbidden');
		expect(outcome.resolved).toBe(false);
	});

	it('refuses inside the framing and session stamps, before the sign-in gate asks the backend', async () => {
		quietDevLog();
		const hooks = await hooksUnder({ dev: true, publicUrl: 'http://127.0.0.1:8080' });
		const signedInPost: Post = {
			url: 'http://127.0.0.1:5174/accounts',
			routeId: '/(session)/accounts',
			origin: 'https://evil.example',
			cookie: 'zurfur.sid=s3ss10n'
		};

		const outcome = await postThrough(hooks.handle, signedInPost);

		expect(outcome.response.status).toBe(403);
		expect(outcome.response.headers.get('content-security-policy')).toBe(FRAME_ANCESTORS_POLICY);
		expect(outcome.response.headers.get('x-frame-options')).toBe(X_FRAME_OPTIONS);
		expect(outcome.response.headers.get('cache-control')).toBe(SESSION_CACHE_CONTROL);
		expect(outcome.fetched).toBe(false);
		expect(outcome.resolved).toBe(false);
	});
});
