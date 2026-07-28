/**
 * Shared spec helpers for stubbing the fetch seam — the frontend counterpart
 * of the backend's `tests/common` module: one shape for the fetch stub and
 * the problem-body builder instead of per-file variants.
 */

import type { FetchFunction } from '$lib/api/client';

/** Builds the response a stubbed fetch answers with. */
type ResponseFn = () => Response;

/** What `fetchStub` hands back: the stub itself plus the URLs it saw. */
type FetchStub = {
	fetch: FetchFunction;
	calls: string[];
};

/**
 * A `fetch` stub answering every call with a fresh response from `respond`
 * (fresh because Response bodies are single-use), recording requested URLs.
 * Destructure only what the test needs.
 */
export function fetchStub(respond: ResponseFn): FetchStub {
	const calls: string[] = [];
	const fetch = (async (input: RequestInfo | URL) => {
		calls.push(String(input));
		return respond();
	}) as FetchFunction;
	return { fetch, calls };
}

/** Builds the response a routed stub answers with, given the requested URL and init. */
type RoutedResponseFn = (url: string, init?: RequestInit) => Response;

/**
 * A `fetch` stub for actions that make MORE THAN ONE backend call (e.g. read
 * the listing, then delete): `respond` sees each call's URL and init and
 * routes on them. Same recording contract as {@link fetchStub}.
 */
export function routedFetchStub(respond: RoutedResponseFn): FetchStub {
	const calls: string[] = [];
	const fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
		calls.push(String(input));
		return respond(String(input), init);
	}) as FetchFunction;
	return { fetch, calls };
}

/** A `fetch` stub that fails like a dead backend (connection refused, DNS, …). */
export function unreachableFetch(message = 'fetch failed'): FetchFunction {
	return (async () => {
		throw new TypeError(message);
	}) as FetchFunction;
}

/** A registry-shaped `application/problem+json` response for `code` at `status`. */
export function problemResponse(status: number, code: string, detail?: string): Response {
	const body = {
		type: `urn:zurfur:error:${code.replaceAll('_', '-')}`,
		code,
		title: code,
		...(detail === undefined ? {} : { detail }),
		status
	};
	const headers = { 'content-type': 'application/problem+json' };
	return new Response(JSON.stringify(body), { status, headers });
}
