import type { HandleFetch } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';
import { rewriteApiRequest } from '$lib/server/api-proxy';
import { mockModeMisconfigured } from '$lib/server/api/zurfur-api-mock';

/**
 * PROD GUARD (ZMVP-198): mock mode must be UNREACHABLE in a real server,
 * checked at BOOT. `hooks.server.ts` is the module adapter-node's server
 * entry point loads before it starts accepting requests, so a throw here
 * fails the PROCESS at startup — not lazily, on whichever request happens to
 * first touch the seam (`runtime.ts`, where this guard used to live and only
 * ran on that module's own first import).
 *
 * {@link mockModeMisconfigured} (`zurfur-api-mock.ts`) is `ZURFUR_WEB_MOCK`
 * requested AND NOT a dev build AND NOT `vite build`'s own postbuild
 * `building` phase — that last term keeps `ZURFUR_WEB_MOCK=1 yarn build`
 * itself green when the flag leaks in from a dotenv-loaded `.env` (`just`'s
 * `dotenv-load` applies to every recipe, not only `dev-mock`); only an
 * actually booted, non-dev SERVER process with the flag set is the real
 * misconfiguration this exists to catch.
 *
 * Loopback assumption, stated once: mock mode is safe ONLY because `vite
 * dev` binds `127.0.0.1` by default — never pair `just dev-mock` with
 * `--host` (that would expose the unauthenticated, in-memory fixture world
 * to the LAN instead of just this machine).
 *
 * The review rulebook bans `throw` in production code (errors are values,
 * so a caller can branch on them) — but there is no request in flight yet
 * at boot to carry an error value through, so this is the one channel
 * available.
 */
if (mockModeMisconfigured()) {
	// eslint-disable-next-line no-restricted-syntax -- boot-time guard only, see the comment above: no request exists yet to carry an error value through.
	throw new Error(
		'ZURFUR_WEB_MOCK is set outside a dev build — mock mode must never reach production.'
	);
}

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
	const incomingCookie = event.request.headers.get('cookie') ?? undefined;

	const proxied = rewriteApiRequest({
		request,
		eventOrigin: event.url.origin,
		incomingCookie,
		apiUpstream
	});

	return fetch(proxied);
};
