import type { Problem } from '$lib/api/problem';
import type { SuperValidated } from 'sveltekit-superforms';

/**
 * One shape for hand-built superform props instead of per-file variants (the
 * `http.ts` rule, applied to forms): a `SuperValidated` the compiler actually
 * checks, with `constraints` populated the way the effect adapter emits them
 * (`required: true` per field — it derives from the ENCODED side of
 * `Schema.Trim`, so no `minlength`/`pattern` ever appears) so specs render the
 * same code path the real form takes.
 */
export function formStub<T extends Record<string, unknown>>(
	data: T,
	overrides: {
		errors?: Partial<Record<Extract<keyof T, string>, string[]>>;
		message?: Problem;
	} = {}
): SuperValidated<T, Problem> {
	const constraints = Object.fromEntries(
		Object.keys(data).map((field) => [field, { required: true }])
	);
	return {
		id: 'spec',
		valid: overrides.errors === undefined,
		posted: overrides.errors !== undefined || overrides.message !== undefined,
		data,
		errors: (overrides.errors ?? {}) as SuperValidated<T, Problem>['errors'],
		constraints: constraints as SuperValidated<T, Problem>['constraints'],
		...(overrides.message === undefined ? {} : { message: overrides.message })
	};
}
