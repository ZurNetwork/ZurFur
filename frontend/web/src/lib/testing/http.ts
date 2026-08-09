/**
 * Shared spec helpers for stubbing the fetch seam — the frontend counterpart
 * of the backend's `tests/common` module: one shape for the fetch stub and
 * the problem-body builder instead of per-file variants.
 */

import type { FetchFunction } from '$lib/api/client';

/**
 * Builds the response a stubbed fetch answers with. The URL and init are
 * offered for stubs that route on them (an action making more than one
 * backend call); a zero-arg callback is assignable for the single-call case.
 */
type ResponseFn = (url: string, init?: RequestInit) => Response;

/** What `fetchStub` hands back: the stub itself plus the URLs it saw. */
interface FetchStub {
	fetch: FetchFunction;
	calls: string[];
}

/**
 * Every `fetch` first-argument shape reduced to its URL string. Template-
 * stringifying `RequestInfo | URL` directly would silently produce
 * `"[object Request]"` for the `Request` arm (its `toString` is the
 * Object.prototype default) — `Request.url` and `URL.toString()` are the
 * arms that actually carry the URL.
 */
export function requestUrl(input: RequestInfo | URL): string {
	if (typeof input === 'string') return input;
	if (input instanceof URL) return input.toString();
	return input.url;
}

/**
 * A `fetch` stub answering every call with a fresh response from `respond`
 * (fresh because Response bodies are single-use), recording requested URLs.
 * Destructure only what the test needs.
 */
export function fetchStub(respond: ResponseFn): FetchStub {
	const calls: string[] = [];
	// resolve-then so a throwing responder REJECTS like real fetch (which
	// never throws synchronously) instead of throwing out of the stub call.
	const fetch = ((input: RequestInfo | URL, init?: RequestInit) => {
		const url = requestUrl(input);
		calls.push(url);
		return Promise.resolve().then(() => respond(url, init));
	}) as FetchFunction;
	return { fetch, calls };
}

/** A `fetch` stub that fails like a dead backend (connection refused, DNS, …). */
export function unreachableFetch(message = 'fetch failed'): FetchFunction {
	return () => Promise.reject(new TypeError(message));
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
