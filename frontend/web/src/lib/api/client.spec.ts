import { describe, expect, it } from 'vitest';
import { apiFetch } from './client';

/** A `fetch` stub that always returns the given response, recording the request. */
function fetchReturning(response: Response): { fetch: typeof globalThis.fetch; calls: string[] } {
	const calls: string[] = [];
	const fetch = (async (input: RequestInfo | URL) => {
		calls.push(String(input));
		return response;
	}) as typeof globalThis.fetch;
	return { fetch, calls };
}

function problemResponse(status: number, code: string): Response {
	const body = {
		type: `urn:zurfur:error:${code.replaceAll('_', '-')}`,
		code,
		title: code,
		status
	};
	const headers = { 'content-type': 'application/problem+json' };
	return new Response(JSON.stringify(body), { status, headers });
}

describe('apiFetch', () => {
	it('prefixes the path with /api so both split halves route it', async () => {
		const { fetch, calls } = fetchReturning(Response.json({}));
		await apiFetch(fetch, '/me');
		expect(calls).toEqual(['/api/me']);
	});

	it('returns the parsed body on success', async () => {
		const { fetch } = fetchReturning(Response.json({ did: 'did:plc:alice' }));
		const result = await apiFetch<{ did: string }>(fetch, '/me');
		expect(result).toEqual({ ok: true, status: 200, data: { did: 'did:plc:alice' } });
	});

	it('returns undefined data for a bodyless 204', async () => {
		const { fetch } = fetchReturning(new Response(null, { status: 204 }));
		const result = await apiFetch(fetch, '/things');
		expect(result).toEqual({ ok: true, status: 204, data: undefined });
	});

	it('returns the parsed problem for a problem+json error', async () => {
		const { fetch } = fetchReturning(problemResponse(401, 'not_authenticated'));
		const result = await apiFetch(fetch, '/me');
		expect(result.ok).toBe(false);
		if (!result.ok) {
			expect(result.problem.code).toBe('not_authenticated');
			expect(result.problem.status).toBe(401);
		}
	});

	it('throws on a non-problem error response (broken contract)', async () => {
		const { fetch } = fetchReturning(new Response('gateway timeout', { status: 504 }));
		await expect(apiFetch(fetch, '/me')).rejects.toThrow(/contract violation/);
	});
});
