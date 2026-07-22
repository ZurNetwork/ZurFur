/**
 * The tagged error union the Effect seam speaks (DD 39944194): every way a
 * backend call fails, as a distinct tag the seam can `catchTags` on. The RFC
 * 9457 problem (DD 23592962) rides inside the tags that carry one — the wire
 * contract is not re-modeled, just lifted into the error channel.
 */

import { Data } from 'effect';
import type { Problem } from '$lib/api/problem';

/** The backend's 401 `not_authenticated` — the one problem session flows branch on. */
export class NotAuthenticated extends Data.TaggedError('NotAuthenticated')<{
	readonly problem: Problem;
}> {}

/** Any other RFC 9457 problem, carried whole for `fail()` / rendering. */
export class ApiProblem extends Data.TaggedError('ApiProblem')<{
	readonly problem: Problem;
}> {}

/** The backend could not be reached at all (the fetch itself rejected). */
export class NetworkFailure extends Data.TaggedError('NetworkFailure')<{
	readonly cause: unknown;
}> {}

/** A response that fits neither the success shape nor `application/problem+json`. */
export class ContractViolation extends Data.TaggedError('ContractViolation')<{
	readonly path: string;
	readonly status: number;
	readonly detail: string;
}> {
	override get message(): string {
		return `API contract violation: ${this.path} responded ${this.status} — ${this.detail}`;
	}
}

/** `POST /logout` ended without the redirect the backend contract promises. */
export class SignoutFailed extends Data.TaggedError('SignoutFailed')<{
	readonly status: number;
}> {}

/** Everything a {@link import('./zurfur-api').ZurfurApi} call can fail with. */
export type ZurfurApiError =
	NotAuthenticated | ApiProblem | NetworkFailure | ContractViolation | SignoutFailed;
