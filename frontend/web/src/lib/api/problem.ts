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
import { HttpStatus } from './http-status';

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
 * The problem kinds the frontend mints locally, each entry the type/code PAIR
 * (the hyphenated URN and the snake_case code clients branch on) — one entry
 * per kind, spread at the mint site, so a mismatched pairing is unwritable
 * rather than doc-enforced (Engineer ruling 2026-07-28, revised from split
 * enums on the round-2 gate evidence). Not a mirror of the backend's full
 * registry — wire problems arrive already paired; an entry lands here when
 * the frontend mints that kind itself.
 */
export const ProblemKind = {
	InvalidRequest: { type: 'urn:zurfur:error:invalid-request', code: 'invalid_request' },
	AccountNotFound: { type: 'urn:zurfur:error:account-not-found', code: 'account_not_found' },
	NotAuthenticated: { type: 'urn:zurfur:error:not-authenticated', code: 'not_authenticated' },
	Forbidden: { type: 'urn:zurfur:error:forbidden', code: 'forbidden' }
} as const satisfies Record<string, Pick<Problem, 'type' | 'code'>>;

/** The union of every locally-mintable kind (a `{type, code}` pair). */
export type ProblemKind = (typeof ProblemKind)[keyof typeof ProblemKind];

/**
 * Field-for-field the backend's own `Problem::forbidden()` — reused verbatim
 * rather than invented (same convention as the derived not-found in
 * $lib/server/accounts.ts): a local authorization pre-check answers the SAME
 * condition the backend would 403, so it must say the same thing. It only
 * ever DENIES — the backend stays authoritative for every request that gets
 * through — but if an Owner-only rule ever widens (e.g. Admins), a gate
 * minting this must move in lockstep or it denies what the backend allows.
 */
export const FORBIDDEN_PROBLEM: Problem = {
	...ProblemKind.Forbidden,
	title: 'Forbidden',
	detail: "You don't have permission to perform this action.",
	status: HttpStatus.Forbidden
};

/**
 * The status a decoded problem can safely put on an HTTP response. Wire
 * `status` is a proto3 implicit-presence int32 — a body that omits it decodes
 * to 0, and `fail()` forwards whatever it's given into a `Response`, which
 * throws a RangeError outside 200–599. Anything outside the error range
 * collapses to 500 so a malformed problem degrades to a plain server error
 * instead of crashing the render of a handled one.
 */
export function renderableStatus(problem: Problem): number {
	const { status } = problem;
	return status >= 400 && status <= 599 ? status : HttpStatus.InternalServerError;
}
