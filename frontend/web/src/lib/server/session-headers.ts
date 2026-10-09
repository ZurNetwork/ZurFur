import type { Handle } from '@sveltejs/kit';
import { extractSessionCookie } from './api-proxy';

/**
 * The `Cache-Control` every signed-in response carries: the API's `no-store`,
 * plus `private` as defence in depth. No `Pragma`: it is deprecated
 * and was never defined for responses. No `Vary: Cookie`: Vary only governs
 * reuse of a stored response, and `no-store` means nothing is stored.
 */
export const SESSION_CACHE_CONTROL = 'private, no-store';

/**
 * The `Referrer-Policy` every signed-in response carries: a link to another
 * site sends no `Referer`, so a signed-in address (a Den path holds DIDs and
 * commission ids) never leaves this origin. Never tighten it to
 * `no-referrer`: that makes form posts send `Origin: null`, and the origin
 * checks would refuse every form action.
 */
export const SESSION_REFERRER_POLICY = 'same-origin';

/**
 * Whether the request carries the session cookie. It is the only credential
 * `handleFetch` forwards to the backend, so every signed-in render had it; a
 * stale one is treated as signed in too.
 */
function carriesSession(request: Request): boolean {
	const cookieHeader = request.headers.get('cookie') ?? undefined;
	return extractSessionCookie(cookieHeader) !== undefined;
}

/**
 * Marks every response to a signed-in request `private, no-store` and
 * `Referrer-Policy: same-origin`, overriding anything a page set — pages,
 * `__data.json`, form-action JSON, redirects, 404 and error pages alike. It
 * stamps a copy, since a `fetch()` or `Response.redirect()` result has
 * immutable headers. Anonymous responses pass through untouched.
 */
export const sessionHeaders: Handle = async ({ event, resolve }) => {
	const response = await resolve(event);
	if (!carriesSession(event.request)) return response;

	const stamped = new Response(response.body, response);
	stamped.headers.set('cache-control', SESSION_CACHE_CONTROL);
	stamped.headers.set('referrer-policy', SESSION_REFERRER_POLICY);
	return stamped;
};
