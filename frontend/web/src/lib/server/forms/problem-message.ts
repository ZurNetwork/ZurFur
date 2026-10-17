import { message, type ErrorStatus, type SuperValidated } from 'sveltekit-superforms';
import { isRenderableErrorStatus, renderableStatus, type Problem } from '$lib/api/problem';
import { HttpStatus } from '$lib/api/http-status';

/**
 * The type claim bridging `renderableStatus`'s plain-number signature to
 * superforms' literal-union `ErrorStatus`. The RANGE itself lives in
 * {@link isRenderableErrorStatus} — stated once; this predicate adds only
 * the superforms type, which `$lib/api` deliberately doesn't know about.
 */
function isErrorStatus(status: number): status is ErrorStatus {
	return isRenderableErrorStatus(status);
}

/**
 * The problem's own (renderable) status as superforms' `ErrorStatus`.
 * `renderableStatus` clamps to 400–599 at runtime, so the fallback arm is
 * unreachable; it exists because {@link isErrorStatus} proves the range to
 * the type system instead of asserting it.
 */
export function formErrorStatus(problem: Problem): ErrorStatus {
	const status = renderableStatus(problem);
	return isErrorStatus(status) ? status : HttpStatus.InternalServerError;
}

/**
 * The one way a backend `Problem` rides back to a page as a whole: as the
 * superform's status message, failing the request with the problem's own
 * (renderable) status. Keep every action on this helper (or on
 * `problemOnFields`, for a problem that names fields) so the bridge lives
 * (and is right) exactly once.
 */
export function problemMessage<T extends Record<string, unknown>>(
	form: SuperValidated<T>,
	problem: Problem
) {
	return message(form, problem, { status: formErrorStatus(problem) });
}
