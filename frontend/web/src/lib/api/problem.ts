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

/**
 * The `urn:zurfur:error:*` URNs the frontend mints locally, named — the
 * hyphenated half of the type/code pair. Entry names match {@link ProblemCode}
 * one-to-one; a locally-built Problem must take BOTH fields from the same
 * entry name. Not a mirror of the backend's full registry — wire problems
 * arrive already paired; an entry lands here when the frontend mints it.
 */
export const ProblemType = {
	InvalidRequest: 'urn:zurfur:error:invalid-request',
	AccountNotFound: 'urn:zurfur:error:account-not-found',
	NotAuthenticated: 'urn:zurfur:error:not-authenticated',
	Forbidden: 'urn:zurfur:error:forbidden'
} as const;

/** The union of every locally-minted problem URN. */
export type ProblemType = (typeof ProblemType)[keyof typeof ProblemType];

/**
 * The machine codes matching {@link ProblemType} entry-for-entry — the
 * snake_case half of the pair (clients branch on this, never on the URN).
 */
export const ProblemCode = {
	InvalidRequest: 'invalid_request',
	AccountNotFound: 'account_not_found',
	NotAuthenticated: 'not_authenticated',
	Forbidden: 'forbidden'
} as const;

/** The union of every locally-minted problem code. */
export type ProblemCode = (typeof ProblemCode)[keyof typeof ProblemCode];
