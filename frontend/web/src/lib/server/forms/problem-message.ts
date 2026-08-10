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
 * The one way a backend `Problem` rides back to a page: as the superform's
 * status message, failing the request with the problem's own (renderable)
 * status. `renderableStatus` clamps to 400–599 at runtime, so the fallback
 * arm is unreachable; it exists because {@link isErrorStatus} proves the
 * range to the type system instead of asserting it. Keep every action on
 * this helper so the bridge lives (and is right) exactly once.
 */
export function problemMessage<T extends Record<string, unknown>>(
	form: SuperValidated<T>,
	problem: Problem
) {
	const status = renderableStatus(problem);
	return message(form, problem, {
		status: isErrorStatus(status) ? status : HttpStatus.InternalServerError
	});
}
