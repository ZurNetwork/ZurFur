/**
 * The single rewrite that makes in-app `fetch('/api/v1/...')` behave identically in
 * the browser and during SSR.
 *
 * In the browser the call rides Caddy: same-origin `/api/v1/*` reaches axum with the
 * `/api` prefix stripped. During SSR there is no Caddy in the loop, so SvelteKit's
 * `handleFetch` hook applies this same rewrite by hand — pointing the request at
 * the internal axum origin, stripping the prefix, and forwarding the caller's
 * session cookie — so component code writes ONE `fetch('/api/v1/...')` and never
 * branches on environment.
 *
 * Kept a pure function (no `$env`, no hook state) so it is unit-testable in
 * isolation; `hooks.server.ts` reads the env and the incoming cookie and passes
 * them in.
 */

/** The `/api` prefix, exposed once so the boundary logic can't drift. */
// The FULL versioned prefix is stripped (Caddy does the same): the backend's
// route table stays unprefixed, and the weld between path-major and proto
// package is held by the contract's router test rather than by axum nesting.
const API_PREFIX = '/api/v1';

/**
 * The ONLY cookie ever forwarded to the upstream: the host-only session cookie.
 * Never the sign-in's browser-binding cookie, which reaches the backend only on
 * `/signin-callback`, a browser navigation Caddy proxies to axum directly.
 * Named once so the filter below and any future caller can't drift apart —
 * exported so the mock `ZurfurApi` Layer (`zurfur-api-mock.ts`)
 * reports the same name `signout` clears, rather than a hand-copied literal.
 */
export const SESSION_COOKIE_NAME = 'zurfur.sid';

export interface RewriteApiRequestInput {
	/** The outgoing request SvelteKit's `handleFetch` handed us. */
	request: Request;
	/** `event.url.origin` — the browser-visible origin the app resolves against. */
	eventOrigin: string;
	/**
	 * The incoming request's raw `cookie` header (`event.request.headers.get('cookie')`,
	 * platform null converted at contact), or `undefined` when the caller sent none.
	 * Only the `zurfur.sid` pair is extracted from it and forwarded, and only to
	 * the API upstream — never the whole header.
	 */
	incomingCookie: string | undefined;
	/** The internal axum origin (`ZURFUR_API_UPSTREAM`, e.g. `http://127.0.0.1:8081`). */
	apiUpstream: string;
}

/**
 * Extract ONLY the `zurfur.sid` pair from a raw `cookie` header, or `undefined`
 * when it is absent. Cookies are host-scoped by hostname and NOT by port, so on
 * `127.0.0.1` every other local dev tool shares one cookie jar — forwarding the
 * whole header would leak those unrelated cookies straight to axum. We parse
 * defensively: pairs are `;`-separated with optional surrounding whitespace, and
 * a value may itself contain `=` (e.g. base64 padding), so each pair is split on
 * its FIRST `=` only. Exported so the session cache hook recognizes a
 * signed-in request by the same cookie this rewrite forwards.
 */
export function extractSessionCookie(incomingCookie: string | undefined): string | undefined {
	if (incomingCookie === undefined) {
		return undefined;
	}

	const pairs = incomingCookie.split(';');
	for (const pair of pairs) {
		const trimmed = pair.trim();
		const equalsIndex = trimmed.indexOf('=');
		if (equalsIndex === -1) {
			continue;
		}

		const name = trimmed.slice(0, equalsIndex);
		if (name === SESSION_COOKIE_NAME) {
			const value = trimmed.slice(equalsIndex + 1);
			return `${SESSION_COOKIE_NAME}=${value}`;
		}
	}
	return undefined;
}

/**
 * Rewrite a same-origin `/api/v1/*` request to the axum upstream (prefix stripped,
 * query preserved, method/body/headers preserved, only `zurfur.sid` forwarded).
 * A cross-origin request passes through with credentials omitted, so SvelteKit
 * adds no cookies to it; a same-origin non-`/api` request is returned as-is.
 * Neither passthrough has its own headers added to or stripped.
 */
export function rewriteApiRequest(input: RewriteApiRequestInput): Request {
	const { request, eventOrigin, incomingCookie, apiUpstream } = input;

	const requestUrl = new URL(request.url);

	// Cross-origin fetches (a CDN, a third-party API) are never ours to rewrite,
	// and must never receive the session cookie. `credentials: 'omit'` keeps
	// SvelteKit from attaching the visitor's cookie jar to one whose hostname is
	// the app's or a subdomain of it.
	const isSameOrigin = requestUrl.origin === eventOrigin;
	if (!isSameOrigin) {
		return new Request(request, { credentials: 'omit' });
	}

	// Only `/api` exactly or a path under `/api/` is an API call. `/apifoo` is a
	// different route (a prefix is not a stem) and falls through untouched — this
	// mirrors Caddy's `handle_path /api/v1/*` + `handle /api` split.
	const path = requestUrl.pathname;
	const isApiCall = path === API_PREFIX || path.startsWith(`${API_PREFIX}/`);
	if (!isApiCall) {
		return request;
	}

	// Strip the `/api` prefix. A bare `/api` maps to the upstream root `/`.
	const strippedPath = path === API_PREFIX ? '/' : path.slice(API_PREFIX.length);

	const upstreamUrl = new URL(apiUpstream);
	upstreamUrl.pathname = strippedPath;
	upstreamUrl.search = requestUrl.search;

	// Clone method, body, and headers onto the new target URL, then set the cookie
	// solely from the incoming request: delete first so a stray cookie on the
	// outgoing request can't leak, then forward only the `zurfur.sid` pair — never
	// the caller's other host-scoped cookies — and only when it is actually present.
	// `credentials: 'omit'` stops SvelteKit's server fetch from re-attaching the
	// whole incoming cookie jar when the upstream's hostname matches the app's
	// (both 127.0.0.1 in dev); the explicit header below still goes out.
	const rewritten = new Request(new Request(upstreamUrl, request), { credentials: 'omit' });
	rewritten.headers.delete('cookie');
	const sessionCookie = extractSessionCookie(incomingCookie);
	if (sessionCookie !== undefined) {
		rewritten.headers.set('cookie', sessionCookie);
	}
	return rewritten;
}
