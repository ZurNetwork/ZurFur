import type { Handle } from '@sveltejs/kit';

/** Refuses every frame: no other site may embed a Zurfur page. */
export const FRAME_ANCESTORS_POLICY = "frame-ancestors 'none'";

/** The older header for the same refusal, for browsers that predate `frame-ancestors`. */
export const X_FRAME_OPTIONS = 'DENY';

/**
 * Stamps every response with `Content-Security-Policy: frame-ancestors 'none'`
 * and `X-Frame-Options: DENY`, so no site can frame Zurfur and trick a click.
 * It stamps a copy, since a `fetch()` or `Response.redirect()` result has
 * immutable headers.
 */
export const refuseFraming: Handle = async ({ event, resolve }) => {
	const response = await resolve(event);
	const stamped = new Response(response.body, response);
	stamped.headers.set('content-security-policy', FRAME_ANCESTORS_POLICY);
	stamped.headers.set('x-frame-options', X_FRAME_OPTIONS);
	return stamped;
};
