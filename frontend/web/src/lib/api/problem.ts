/**
 * The RFC 9457 problem-details shape the backend emits for every API error
 * (DD 23592962, ZMVP-35; registry in `backend/crates/api/src/problem.rs`).
 * Success bodies stay bare — a problem only ever arrives with an error status
 * and the `application/problem+json` content type. Clients branch on `code`
 * (the terse machine string), never on `type` (a non-dereferenceable
 * `urn:zurfur:error:*` URN naming the class).
 */
export interface Problem {
	type: string;
	code: string;
	title: string;
	/** Per-occurrence human detail; the registry leaves it off some problems. */
	detail?: string;
	status: number;
}

/** The content type every backend problem response carries. */
export const PROBLEM_CONTENT_TYPE = 'application/problem+json';

/**
 * Narrow an unknown parsed body to a {@link Problem}. Checks the four required
 * members by type — enough to trust the shape without re-validating the URN,
 * which the backend owns.
 */
export function isProblem(value: unknown): value is Problem {
	if (typeof value !== 'object' || value === null) return false;
	const candidate = value as Record<string, unknown>;
	return (
		typeof candidate.type === 'string' &&
		typeof candidate.code === 'string' &&
		typeof candidate.title === 'string' &&
		typeof candidate.status === 'number'
	);
}
