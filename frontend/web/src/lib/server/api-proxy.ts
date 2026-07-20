/**
 * The single rewrite that makes in-app `fetch('/api/...')` behave identically in
 * the browser and during SSR (ZMVP-150, AC3).
 *
 * In the browser the call rides Caddy: same-origin `/api/*` reaches axum with the
 * `/api` prefix stripped. During SSR there is no Caddy in the loop, so SvelteKit's
 * `handleFetch` hook applies this same rewrite by hand — pointing the request at
 * the internal axum origin, stripping the prefix, and forwarding the caller's
 * session cookie — so component code writes ONE `fetch('/api/...')` and never
 * branches on environment.
 *
 * Kept a pure function (no `$env`, no hook state) so it is unit-testable in
 * isolation; `hooks.server.ts` reads the env and the incoming cookie and passes
 * them in.
 */

/** The `/api` prefix, exposed once so the boundary logic can't drift. */
const API_PREFIX = '/api';

export interface RewriteApiRequestInput {
	/** The outgoing request SvelteKit's `handleFetch` handed us. */
	request: Request;
	/** `event.url.origin` — the browser-visible origin the app resolves against. */
	eventOrigin: string;
	/**
	 * The incoming request's `cookie` header (`event.request.headers.get('cookie')`),
	 * or `null` when the caller sent none. This is the ONLY cookie ever forwarded,
	 * and only to the API upstream.
	 */
	incomingCookie: string | null;
	/** The internal axum origin (`ZURFUR_API_UPSTREAM`, e.g. `http://127.0.0.1:8081`). */
	apiUpstream: string;
}

/**
 * Rewrite a same-origin `/api/*` request to the axum upstream (prefix stripped,
 * query preserved, method/body/headers preserved, session cookie forwarded), or
 * return the request untouched when it is cross-origin or not under `/api`.
 *
 * The session cookie is forwarded to the upstream and NOWHERE else: a cross-origin
 * or non-`/api` request is returned as-is, so it can never carry `zurfur.sid`.
 */
export function rewriteApiRequest(input: RewriteApiRequestInput): Request {
	const { request, eventOrigin, incomingCookie, apiUpstream } = input;

	const requestUrl = new URL(request.url);

	// Cross-origin fetches (a CDN, a third-party API) are never ours to rewrite,
	// and must never receive the session cookie — hand them back untouched.
	const isSameOrigin = requestUrl.origin === eventOrigin;
	if (!isSameOrigin) {
		return request;
	}

	// Only `/api` exactly or a path under `/api/` is an API call. `/apifoo` is a
	// different route (a prefix is not a stem) and falls through untouched — this
	// mirrors Caddy's `handle_path /api/*` + `handle /api` split.
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
	// outgoing request can't leak, then set only when the caller actually sent one.
	const rewritten = new Request(upstreamUrl, request);
	rewritten.headers.delete('cookie');
	if (incomingCookie !== null) {
		rewritten.headers.set('cookie', incomingCookie);
	}
	return rewritten;
}
