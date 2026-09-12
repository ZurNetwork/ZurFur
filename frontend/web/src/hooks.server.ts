import type { HandleFetch } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';
import { rewriteApiRequest } from '$lib/server/api-proxy';
import { mockModeMisconfigured } from '$lib/server/api/zurfur-api-mock';

/**
 * PROD GUARD: mock mode must be UNREACHABLE in a real server, checked at
 * BOOT — `hooks.server.ts` loads before adapter-node starts accepting
 * requests, so a failed check here fails the process at startup rather than
 * lazily on the first request that touches the seam. See NODE.md for the
 * full rationale (why `throw`, the loopback assumption, `building`-phase
 * carve-out).
 */
if (mockModeMisconfigured()) {
	// eslint-disable-next-line no-restricted-syntax -- boot-time guard only, see the comment above: no request exists yet to carry an error value through.
	throw new Error(
		'ZURFUR_WEB_MOCK is set outside a dev build — mock mode must never reach production.'
	);
}

/**
 * Fallback axum origin when ZURFUR_API_UPSTREAM is unset — matches the Caddyfile
 * and .env.example defaults (the internal axum bind).
 */
const DEFAULT_API_UPSTREAM = 'http://127.0.0.1:8081';

/**
 * Server-side `fetch` rewrite so in-app `fetch('/api/...')` is ONE code path in
 * the browser and during SSR.
 *
 * In the browser these calls ride Caddy. During SSR there is no Caddy, so we
 * point same-origin `/api/*` at the internal axum origin (prefix stripped) and
 * forward the caller's session cookie — see {@link rewriteApiRequest}. The
 * upstream is read via `$env/dynamic/private` so the build never bakes it in and
 * a worktree's own port is honored at runtime.
 */
export const handleFetch: HandleFetch = ({ event, request, fetch }) => {
	const apiUpstream = env.ZURFUR_API_UPSTREAM ?? DEFAULT_API_UPSTREAM;
	const incomingCookie = event.request.headers.get('cookie') ?? undefined;

	const proxied = rewriteApiRequest({
		request,
		eventOrigin: event.url.origin,
		incomingCookie,
		apiUpstream
	});

	return fetch(proxied);
};
