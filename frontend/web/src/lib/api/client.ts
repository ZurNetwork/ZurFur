/**
 * The one typed fetch seam every screen talks to the backend through
 * (ZMVP-151). Callers hand in the ambient `fetch` — the SvelteKit event
 * `fetch` during SSR (where `handleFetch` rewrites `/api` to the axum
 * upstream and forwards the session cookie) or the browser's `fetch` (where
 * Caddy does the same split) — so one code path serves both sides, exactly
 * like {@link import('../server/api-proxy').rewriteApiRequest} promises.
 */

import { isProblem, PROBLEM_CONTENT_TYPE, type Problem } from './problem';

/**
 * The `fetch` signature every seam function accepts — the SvelteKit event
 * `fetch` during SSR or the browser's own; named so no signature spells out
 * the global's type inline.
 */
export type FetchFunction = typeof fetch;

/**
 * Every API call resolves to exactly one of the two shapes the backend
 * contract allows: a bare success body, or an RFC 9457 problem. Anything
 * else (a non-problem error, an unparsable body) is a broken contract and
 * throws instead of masquerading as either.
 */
export type ApiResult<T> = { ok: true; status: number; data: T } | { ok: false; problem: Problem };

/** The prefix the origin split routes to axum; kept in lockstep with the proxy seam. */
export const API_PREFIX = '/api';

/**
 * Fetch `path` (backend-relative, e.g. `/me`) through the `/api` split and
 * classify the response by the contract: 2xx → `{ok: true}` with the parsed
 * body (`undefined` for a bodyless 204), `application/problem+json` → `{ok:
 * false}` with the parsed {@link Problem}. A response that is neither is a
 * contract violation and throws.
 */
export async function apiFetch<T>(
	fetch: FetchFunction,
	path: string,
	init?: RequestInit
): Promise<ApiResult<T>> {
	const response = await fetch(`${API_PREFIX}${path}`, init);

	if (response.ok) {
		const data = response.status === 204 ? undefined : await parsedBody(response, path);
		return { ok: true, status: response.status, data: data as T };
	}

	const contentType = response.headers.get('content-type') ?? '';
	if (contentType.startsWith(PROBLEM_CONTENT_TYPE)) {
		const body = await parsedBody(response, path);
		if (isProblem(body)) {
			return { ok: false, problem: body };
		}
	}
	throw new Error(
		`API contract violation: ${path} responded ${response.status} without a problem body`
	);
}

/**
 * Parse the body as JSON, naming the endpoint and status on failure — a bare
 * `SyntaxError` out of the seam would not say which call broke the contract.
 */
async function parsedBody(response: Response, path: string): Promise<unknown> {
	try {
		return await response.json();
	} catch (cause) {
		throw new Error(
			`API contract violation: ${path} responded ${response.status} with an unparsable body`,
			{ cause }
		);
	}
}
