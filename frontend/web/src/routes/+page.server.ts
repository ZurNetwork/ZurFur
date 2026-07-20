import type { PageServerLoad } from './$types';

/** What the proof page shows: the outcome of one `/api/health` probe. */
interface HealthProbe {
	reachable: boolean;
	status: number | null;
	body: unknown;
	note: string | null;
}

/**
 * Proof-of-wiring load: fetch `/api/health` through the event `fetch`, which
 * during SSR runs the {@link import('../hooks.server').handleFetch} rewrite to
 * the axum upstream. Degrades gracefully — a down backend yields a `reachable:
 * false` probe with a note, never a thrown load — so `vite build` (which does not
 * run this) and a backend-less dev boot both render fine. Not prerendered.
 */
export const load: PageServerLoad = async ({ fetch }) => {
	const health = await probeHealth(fetch);
	return { health };
};

async function probeHealth(fetch: typeof globalThis.fetch): Promise<HealthProbe> {
	try {
		const response = await fetch('/api/health');
		const body = await response.json().catch(() => null);
		return {
			reachable: response.ok,
			status: response.status,
			body,
			note: response.ok ? null : `backend responded ${response.status}`
		};
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		return { reachable: false, status: null, body: null, note: `backend unreachable: ${message}` };
	}
}
