/**
 * The RFC 9457 problem-details shape the backend emits for every API error —
 * the COMPONENT-FACING type (plain data above the runes seam). Since ZMVP-162
 * the shape's one declaration is `contract/zurfur/api/v1/problem.proto`; the
 * server-side boundary decodes through the GENERATED `ProblemSchema` (which
 * cannot drift from the contract) and maps into this plain interface. The
 * hand-written `isProblem` narrowing this file used to carry is retired with
 * the triplication it belonged to.
 *
 * Success bodies stay bare — a problem only ever arrives with an error status
 * and the `application/problem+json` content type. Clients branch on `code`
 * (the terse machine string), never on `type` (a non-dereferenceable
 * `urn:zurfur:error:*` URN naming the class).
 */
export interface Problem {
	type: string;
	code: string;
	title: string;
	/**
	 * Per-occurrence human detail — REQUIRED (Engineer ruling 2026-07-25; the
	 * backend has always emitted it, and its registry now debug-asserts
	 * non-emptiness). Locally-minted problems must supply one too.
	 */
	detail: string;
	status: number;
}

/** The content type every backend problem response carries. */
export const PROBLEM_CONTENT_TYPE = 'application/problem+json';
