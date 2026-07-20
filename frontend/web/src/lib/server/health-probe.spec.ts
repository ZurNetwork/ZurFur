import { describe, expect, it } from 'vitest';
import { probeHealth } from './health-probe';

/** A `fetch` stand-in that always resolves to the given response. */
function fetchReturning(response: Response): typeof globalThis.fetch {
	return (async () => response) as unknown as typeof globalThis.fetch;
}

/** A `fetch` stand-in that rejects — a real network/connection failure. */
function fetchThrowing(error: Error): typeof globalThis.fetch {
	return (async () => {
		throw error;
	}) as unknown as typeof globalThis.fetch;
}

/** Build a JSON response with an explicit status. */
function jsonResponse(status: number, body: unknown): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { 'content-type': 'application/json' }
	});
}

describe('probeHealth', () => {
	it('reports a healthy 2xx as reachable with no note', async () => {
		const fetch = fetchReturning(jsonResponse(200, { status: 'ok' }));

		const probe = await probeHealth(fetch);

		expect(probe.reachable).toBe(true);
		expect(probe.status).toBe(200);
		expect(probe.body).toEqual({ status: 'ok' });
		expect(probe.note).toBeNull();
	});

	it('reports a 5xx backend as REACHABLE (a response arrived) with an error note', async () => {
		// The whole point of fix 4: a 503 backend is reachable, just erroring —
		// not the same as a network failure.
		const fetch = fetchReturning(jsonResponse(503, { error: 'down for maintenance' }));

		const probe = await probeHealth(fetch);

		expect(probe.reachable).toBe(true);
		expect(probe.status).toBe(503);
		expect(probe.note).toBe('backend responded 503');
	});

	it('reports a network failure as NOT reachable with a null status', async () => {
		const fetch = fetchThrowing(new Error('connect ECONNREFUSED'));

		const probe = await probeHealth(fetch);

		expect(probe.reachable).toBe(false);
		expect(probe.status).toBeNull();
		expect(probe.note).toBe('backend unreachable: connect ECONNREFUSED');
	});
});
