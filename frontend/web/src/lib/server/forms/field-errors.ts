import {
	fail,
	setError,
	type FormPathLeavesWithErrors,
	type SuperValidated
} from 'sveltekit-superforms';
import type { Problem } from '$lib/api/problem';
import type { FieldPointer } from '$lib/types/brand';
import { formErrorStatus, problemMessage } from './problem-message';

/** The form field a pointer names, or `undefined` when no field of this form takes it. */
export type FieldOf<T extends Record<string, unknown>> = (
	pointer: FieldPointer
) => FormPathLeavesWithErrors<T> | undefined;

/**
 * A backend `Problem` back onto its form: each field error lands under the
 * field `fieldOf` maps its pointer to, failing with the problem's own status.
 * When any error has no field here (or its pointer didn't parse), the whole
 * problem is also the form message, so nothing the backend said is lost. A
 * problem with no field errors is exactly {@link problemMessage}.
 */
export function problemOnFields<T extends Record<string, unknown>>(
	form: SuperValidated<T>,
	problem: Problem,
	fieldOf: FieldOf<T>
) {
	const status = formErrorStatus(problem);
	let placedAll = true;
	for (const error of problem.errors) {
		const path = error.pointer === undefined ? undefined : fieldOf(error.pointer);
		if (path === undefined) placedAll = false;
		else setError(form, path, error.detail, { status });
	}
	const fieldsOnly = placedAll && problem.errors.length > 0;
	return fieldsOnly ? fail(status, { form }) : problemMessage(form, problem);
}
