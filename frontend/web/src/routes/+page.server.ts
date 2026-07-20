import type { PageServerLoad } from './$types';
import { probeHealth } from '$lib/server/health-probe';

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
