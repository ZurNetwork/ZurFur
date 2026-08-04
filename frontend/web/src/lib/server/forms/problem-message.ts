import { message, type ErrorStatus, type SuperValidated } from 'sveltekit-superforms';
import { renderableStatus, type Problem } from '$lib/api/problem';

/**
 * The one way a backend `Problem` rides back to a page: as the superform's
 * status message, failing the request with the problem's own (renderable)
 * status. `renderableStatus` clamps to 400–599 at runtime; the cast bridges
 * its plain-number signature to superforms' literal-union `ErrorStatus` —
 * type-level only, the clamp is what actually holds. Keep every action on
 * this helper so the cast is explained (and right) exactly once.
 */
export function problemMessage<T extends Record<string, unknown>>(
	form: SuperValidated<T, Problem>,
	problem: Problem
) {
	return message(form, problem, { status: renderableStatus(problem) as ErrorStatus });
}
