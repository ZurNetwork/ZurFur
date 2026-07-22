import { describe, expect, it } from 'vitest';
import { fetchStub, problemResponse } from '$lib/testing/http';
import { apiFetch } from './client';

describe('apiFetch', () => {
	it('prefixes the path with /api so both split halves route it', async () => {
		const { fetch, calls } = fetchStub(() => Response.json({}));
		await apiFetch(fetch, '/me');
		expect(calls).toEqual(['/api/me']);
	});

	it('returns the parsed body on success', async () => {
		const { fetch } = fetchStub(() => Response.json({ did: 'did:plc:alice' }));
		const result = await apiFetch<{ did: string }>(fetch, '/me');
		expect(result).toEqual({ ok: true, status: 200, data: { did: 'did:plc:alice' } });
	});

	it('returns undefined data for a bodyless 204', async () => {
		const { fetch } = fetchStub(() => new Response(null, { status: 204 }));
		const result = await apiFetch(fetch, '/things');
		expect(result).toEqual({ ok: true, status: 204, data: undefined });
	});

	it('returns the parsed problem for a problem+json error', async () => {
		const { fetch } = fetchStub(() => problemResponse(401, 'not_authenticated'));
		const result = await apiFetch(fetch, '/me');
		expect(result.ok).toBe(false);
		if (!result.ok) {
			expect(result.problem.code).toBe('not_authenticated');
			expect(result.problem.status).toBe(401);
		}
	});

	it('throws on a non-problem error response (broken contract)', async () => {
		const { fetch } = fetchStub(() => new Response('gateway timeout', { status: 504 }));
		await expect(apiFetch(fetch, '/me')).rejects.toThrow(/contract violation/);
	});

	it('names the endpoint and status when a success body is not JSON', async () => {
		const { fetch } = fetchStub(() => new Response('not json', { status: 200 }));
		await expect(apiFetch(fetch, '/me')).rejects.toThrow(
			/\/me responded 200 with an unparsable body/
		);
	});
});
