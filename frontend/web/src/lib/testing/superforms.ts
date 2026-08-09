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
		// `| undefined` admitted on purpose: superforms clears a field error by
		// SETTING its key to undefined (never deleting it), so specs must be
		// able to build that cleared-slot shape.
		errors?: Partial<Record<Extract<keyof T, string>, string[] | undefined>>;
		message?: Problem;
	} = {}
): SuperValidated<T> {
	const constraints = Object.fromEntries(
		Object.keys(data).map((field) => [field, { required: true }])
	);
	// One assertion at the end, not per-field: `SuperValidated<T>`'s
	// `constraints`/`errors` members are a deeply conditional mapped type whose
	// generic instantiation here doesn't structurally match itself depending on
	// where it's checked (a known TS quirk) — asserting the whole built object
	// once is what actually resolves that, where per-field casts didn't.
	const stub = {
		id: 'spec',
		valid: overrides.errors === undefined,
		posted: overrides.errors !== undefined || overrides.message !== undefined,
		data,
		errors: overrides.errors ?? {},
		constraints,
		...(overrides.message === undefined ? {} : { message: overrides.message })
	};
	return stub as SuperValidated<T>;
}
