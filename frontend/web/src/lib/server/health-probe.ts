/**
 * The one backend-health probe behind the proof page (ZMVP-150). Kept a pure
 * function taking `fetch` (no route/event coupling) so it is unit-testable in
 * isolation, exactly like {@link import('./api-proxy').rewriteApiRequest}.
 */

/** What the proof page shows: the outcome of one `/api/health` probe. */
export interface HealthProbe {
	/**
	 * True when an HTTP response ARRIVED at all — any status, including a 5xx —
	 * and false only when the fetch itself failed (connection refused, DNS, …).
	 */
	reachable: boolean;
	/** The HTTP status when a response arrived, or `null` on a network/fetch failure. */
	status: number | null;
	/** The parsed JSON body when the response carried one, else `null`. */
	body: unknown;
	/** A human note for the non-healthy states (error status, or unreachable), else `null`. */
	note: string | null;
}

/**
 * Probe `/api/health` through the given `fetch` and classify the outcome without
 * ever throwing. `reachable` reports only whether an HTTP response arrived — a
 * 500/503 backend IS reachable, just erroring — so the caller reads `status` to
 * tell a healthy 2xx apart from a reachable-but-erroring backend, and treats
 * `reachable: false` (status `null`) as a true network failure.
 */
export async function probeHealth(fetch: typeof globalThis.fetch): Promise<HealthProbe> {
	try {
		const response = await fetch('/api/health');
		const body = await response.json().catch(() => null);
		const note = response.ok ? null : `backend responded ${response.status}`;
		// A response arrived, so the backend is reachable regardless of its status.
		return { reachable: true, status: response.status, body, note };
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		return { reachable: false, status: null, body: null, note: `backend unreachable: ${message}` };
	}
}
