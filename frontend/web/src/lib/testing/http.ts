/**
 * Shared spec helpers for stubbing the fetch seam — the frontend counterpart
 * of the backend's `tests/common` module: one shape for the fetch stub and
 * the problem-body builder instead of per-file variants.
 */

/**
 * A `fetch` stub answering every call with a fresh response from `respond`
 * (fresh because Response bodies are single-use), recording requested URLs.
 * Destructure only what the test needs.
 */
export function fetchStub(respond: () => Response): {
	fetch: typeof globalThis.fetch;
	calls: string[];
} {
	const calls: string[] = [];
	const fetch = (async (input: RequestInfo | URL) => {
		calls.push(String(input));
		return respond();
	}) as typeof globalThis.fetch;
	return { fetch, calls };
}

/** A `fetch` stub that fails like a dead backend (connection refused, DNS, …). */
export function unreachableFetch(message = 'fetch failed'): typeof globalThis.fetch {
	return (async () => {
		throw new TypeError(message);
	}) as typeof globalThis.fetch;
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
