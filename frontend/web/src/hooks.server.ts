import type { Handle, HandleFetch, HandleServerError } from '@sveltejs/kit';
import { dev } from '$app/environment';
import { env } from '$env/dynamic/private';
import { rewriteApiRequest } from '$lib/server/api-proxy';
import { mockModeMisconfigured } from '$lib/server/api/zurfur-api-mock';
import { refuseCrossSiteWrites } from '$lib/server/cross-site-writes';
import { firstParty } from '$lib/server/first-party-origin';
import { consoleSink, logRequests, logUpstreamCalls } from '$lib/server/dev-log';
import { logUnexpectedError } from '$lib/server/error-log';
import { refuseFraming } from '$lib/server/frame-headers';
import { handleChain } from '$lib/server/handle-chain';
import { requestDeadline } from '$lib/server/request-deadline';
import { firstAbortOf, readWhole, settleBy, whileHeld } from '$lib/server/bounded-fetch';
import { sessionGate } from '$lib/server/session-gate';
import { sessionHeaders } from '$lib/server/session-headers';

/**
 * PROD GUARD: mock mode must be UNREACHABLE in a real server, checked at
 * BOOT — `hooks.server.ts` loads before adapter-node starts accepting
 * requests, so a failed check here fails the process at startup rather than
 * lazily on the first request that touches the seam. See this directory's
 * node for the full rationale (why `throw`, the loopback assumption,
 * `building`-phase carve-out).
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
 * The run-time environment, where any variable may be unset. `svelte-kit sync`
 * types a variable it finds set as always present, so reads go through this view.
 */
function runtimeEnv(): Readonly<Record<string, string | undefined>> {
	return env;
}

/**
 * Server-side `fetch` rewrite so in-app `fetch('/api/...')` is ONE code path in
 * the browser and during SSR. Every request leaving the server is also bound
 * by the request's deadline, on top of the call's own time limit: it is
 * answered whole or fails by then, and both requests stay held until it ends.
 *
 * In the browser these calls ride Caddy. During SSR there is no Caddy, so we
 * point same-origin `/api/*` at the internal axum origin (prefix stripped) and
 * forward the caller's session cookie — see {@link rewriteApiRequest}. The
 * upstream is read via `$env/dynamic/private` so the build never bakes it in and
 * a workspace's own port is honored at runtime. In dev, each call is logged by
 * its page's route template, method, status and time (`logUpstreamCalls`).
 */
const boundedFetch: HandleFetch = ({ event, request, fetch }) => {
	const apiUpstream = runtimeEnv().ZURFUR_API_UPSTREAM ?? DEFAULT_API_UPSTREAM;
	const incomingCookie = event.request.headers.get('cookie') ?? undefined;

	const deadline = event.locals.deadline;
	const signal = firstAbortOf(
		deadline === undefined ? [request.signal] : [request.signal, deadline]
	);
	const proxied = rewriteApiRequest({
		request,
		eventOrigin: event.url.origin,
		incomingCookie,
		apiUpstream,
		signal
	});

	return whileHeld([request, proxied], () =>
		settleBy(signal, () => fetch(proxied).then(readWhole))
	);
};

export const handleFetch: HandleFetch = logUpstreamCalls(dev, consoleSink, boundedFetch);

/**
 * Every request gets one deadline for all its backend calls; every response
 * refuses to be framed; signed-in responses are never stored and never leak
 * their address (`private, no-store`, `Referrer-Policy: same-origin`); and no
 * `(session)` load or action runs without a session. A write (any method but
 * GET, HEAD and OPTIONS) gets a 403 unless its `Origin` is ZURFUR_PUBLIC_URL's
 * origin (`http://127.0.0.1:8080` in a dev build where it is unset; read once
 * by `first-party-origin.ts`) or it sends no `Origin` and `Sec-Fetch-Site:
 * same-origin`. In dev, each request is logged by its method, route template,
 * status and time (`logRequests`).
 */
export const handle: Handle = handleChain(
	logRequests(dev, consoleSink),
	requestDeadline,
	refuseFraming,
	sessionHeaders,
	refuseCrossSiteWrites(firstParty),
	sessionGate
);

/** An unexpected server error is logged by its route template only, and pages get a fixed message. */
export const handleError: HandleServerError = logUnexpectedError;
