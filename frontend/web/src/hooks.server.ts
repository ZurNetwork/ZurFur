import type { HandleFetch } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';
import { rewriteApiRequest } from '$lib/server/api-proxy';

/**
 * Fallback axum origin when ZURFUR_API_UPSTREAM is unset — matches the Caddyfile
 * and .env.example defaults (the internal axum bind, ZMVP-150).
 */
const DEFAULT_API_UPSTREAM = 'http://127.0.0.1:8081';

/**
 * Server-side `fetch` rewrite so in-app `fetch('/api/...')` is ONE code path in
 * the browser and during SSR (ZMVP-150, AC3).
 *
 * In the browser these calls ride Caddy. During SSR there is no Caddy, so we
 * point same-origin `/api/*` at the internal axum origin (prefix stripped) and
 * forward the caller's session cookie — see {@link rewriteApiRequest}. The
 * upstream is read via `$env/dynamic/private` so the build never bakes it in and
 * a worktree's own port is honored at runtime.
 */
export const handleFetch: HandleFetch = ({ event, request, fetch }) => {
	const apiUpstream = env.ZURFUR_API_UPSTREAM ?? DEFAULT_API_UPSTREAM;
	const incomingCookie = event.request.headers.get('cookie');

	const proxied = rewriteApiRequest({
		request,
		eventOrigin: event.url.origin,
		incomingCookie,
		apiUpstream
	});

	return fetch(proxied);
};
