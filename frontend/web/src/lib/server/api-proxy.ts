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

import { SESSION_COOKIE } from './cookies';

/** The `/api` prefix, exposed once so the boundary logic can't drift. */
// The FULL versioned prefix is stripped (Caddy does the same): the backend's
// route table stays unprefixed, and the weld between path-major and proto
// package is held by the contract's router test rather than by axum nesting.
const API_PREFIX = '/api/v1';

export interface RewriteApiRequestInput {
	/** The outgoing request SvelteKit's `handleFetch` handed us. */
	request: Request;
	/** `event.url.origin` — the browser-visible origin the app resolves against. */
	eventOrigin: string;
	/**
	 * The incoming request's raw `cookie` header (`event.request.headers.get('cookie')`,
	 * platform null converted at contact), or `undefined` when the caller sent none.
	 * Only a sole `zurfur.sid` pair is extracted from it and forwarded, and only
	 * to the API upstream — never the whole header, and none when it holds two.
	 */
	incomingCookie: string | undefined;
	/** The internal axum origin (`ZURFUR_API_UPSTREAM`, e.g. `http://127.0.0.1:8081`). */
	apiUpstream: string;
	/**
	 * The signal a request leaving the server carries, set on it directly
	 * rather than followed through the rebuilt requests in between; the
	 * request's own signal when absent.
	 */
	signal?: AbortSignal | undefined;
}

/**
 * Every `zurfur.sid` value in a raw `cookie` header, in the order sent. Pairs
 * are `;`-separated with optional surrounding whitespace, and a value may
 * itself contain `=` (e.g. base64 padding), so each pair is split on its FIRST
 * `=` only. Names are compared exactly, never decoded.
 */
function sessionCookieValues(incomingCookie: string | undefined): readonly string[] {
	if (incomingCookie === undefined) {
		return [];
	}

	const values: string[] = [];
	for (const pair of incomingCookie.split(';')) {
		const trimmed = pair.trim();
		const equalsIndex = trimmed.indexOf('=');
		if (equalsIndex === -1) {
			continue;
		}
		const name = trimmed.slice(0, equalsIndex);
		if (name === SESSION_COOKIE) {
			values.push(trimmed.slice(equalsIndex + 1));
		}
	}
	return values;
}

/**
 * The `zurfur.sid` pair to forward from a raw `cookie` header: the one pair, or
 * `undefined` when there is none or more than one, since a planted second pair
 * can't be told from the visitor's. Never the whole header: cookies are scoped
 * by hostname, not port, so every other local dev tool's cookies share the jar.
 */
export function extractSessionCookie(incomingCookie: string | undefined): string | undefined {
	const [value, ...others] = sessionCookieValues(incomingCookie);
	if (value === undefined || others.length > 0) {
		return undefined;
	}
	// The ONLY cookie ever forwarded: never the sign-in's browser-binding cookie,
	// which reaches the backend only on `/signin-callback`, through Caddy.
	return `${SESSION_COOKIE}=${value}`;
}

/**
 * Whether a raw `cookie` header carries any `zurfur.sid` pair, one or several.
 * Exported so the session headers hook treats a request as signed in by the
 * cookie this rewrite reads, even when two pairs keep it from forwarding one.
 */
export function carriesSessionCookie(incomingCookie: string | undefined): boolean {
	return sessionCookieValues(incomingCookie).length > 0;
}

/**
 * Rewrite a same-origin `/api/v1/*` request to the axum upstream (prefix stripped,
 * query preserved, method/body/headers preserved, only a sole `zurfur.sid` forwarded).
 * A cross-origin request passes through with credentials omitted, so SvelteKit
 * adds no cookies to it; a same-origin non-`/api` request is returned as-is.
 * Neither passthrough has its own headers added to or stripped.
 */
export function rewriteApiRequest(input: RewriteApiRequestInput): Request {
	const { request, eventOrigin, incomingCookie, apiUpstream } = input;
	const signal = input.signal ?? request.signal;

	const requestUrl = new URL(request.url);

	// Cross-origin fetches (a CDN, a third-party API) are never ours to rewrite,
	// and must never receive the session cookie. `credentials: 'omit'` keeps
	// SvelteKit from attaching the visitor's cookie jar to one whose hostname is
	// the app's or a subdomain of it.
	const isSameOrigin = requestUrl.origin === eventOrigin;
	if (!isSameOrigin) {
		return new Request(request, { credentials: 'omit', signal });
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
	// outgoing request can't leak, then forward a sole `zurfur.sid` pair — never
	// the caller's other host-scoped cookies — and only when exactly one is present.
	// `credentials: 'omit'` stops SvelteKit's server fetch from re-attaching the
	// whole incoming cookie jar when the upstream's hostname matches the app's
	// (both 127.0.0.1 in dev); the explicit header below still goes out.
	const rewritten = new Request(new Request(upstreamUrl, request), { credentials: 'omit', signal });
	rewritten.headers.delete('cookie');
	const sessionCookie = extractSessionCookie(incomingCookie);
	if (sessionCookie !== undefined) {
		rewritten.headers.set('cookie', sessionCookie);
	}
	return rewritten;
}
