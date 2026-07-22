/**
 * The shared vocabulary of the backend seam. Since DD 39944194 the actual
 * calls live server-side behind the `ZurfurApi` port
 * ({@link import('../server/api/zurfur-api')}); what remains here is the
 * client-safe surface both sides of the split share.
 */

/** The prefix the origin split routes to axum; kept in lockstep with the proxy seam. */
export const API_PREFIX = '/api';

/**
 * The `fetch` signature every seam function accepts — the SvelteKit event
 * `fetch` during SSR (where `handleFetch` rewrites `/api` to the axum
 * upstream and forwards the session cookie) or the browser's own (where
 * Caddy does the same split) — so one code path serves both sides.
 */
export type FetchFunction = typeof fetch;
