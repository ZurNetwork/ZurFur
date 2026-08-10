import { accountsOutcome, createAccountOutcome } from '$lib/server/accounts';
import { runApi } from '$lib/server/runtime';
import { fail, redirect } from '@sveltejs/kit';
import type { PageServerLoad, Actions } from './$types';
import type { DeleteOutcome } from '$lib/api/account';
import { HttpStatus } from '$lib/api/http-status';
import { superValidate } from 'sveltekit-superforms';
import { effect } from 'sveltekit-superforms/adapters';
import { createAccountForm } from '$lib/server/forms/create-account';
import { problemMessage } from '$lib/server/forms/problem-message';

/**
 * Every outcome a completed delete can carry back via `?deleted=` — keyed as
 * `{ [K in DeleteOutcome]: K }` so adding a fourth outcome to the union is a
 * compile error here, not a silently vanished banner. The banner renders
 * only from this declared vocabulary; it is a flash HINT, not proof a
 * deletion occurred (anyone can type the query param).
 */
const DELETE_OUTCOMES = { soft: 'soft', hard: 'hard', unknown: 'unknown' } as const satisfies {
	[K in DeleteOutcome]: K;
};

/**
 * {@link DELETE_OUTCOMES} as a string-keyed lookup: `.get` with raw query
 * text PARSES it — a known outcome comes back as its `DeleteOutcome`,
 * anything else as `undefined` — with no assertion in the path. A `Map`
 * (not a bare-indexed object) for the same reason `callback-errors.ts`
 * uses one: a prototype name like `?deleted=constructor` must miss cleanly
 * instead of resolving an `Object.prototype` built-in.
 */
const DELETE_OUTCOME_BY_PARAM = new Map<string, DeleteOutcome>(Object.entries(DELETE_OUTCOMES));

/**
 * The caller's account listing, plus the `?deleted=` flash a completed delete
 * redirects back with — narrowed against {@link DeleteOutcome} so the page
 * renders from the declared vocabulary, not from raw query text — and a
 * pristine {@link createAccountForm} superform for the create form below it.
 */
export const load: PageServerLoad = async ({ fetch, url }) => {
	const outcome = await runApi(fetch, accountsOutcome);
	const form = await superValidate(effect(createAccountForm));
	const deleted = DELETE_OUTCOME_BY_PARAM.get(url.searchParams.get('deleted') ?? '');

	return { ...outcome, deleted, form };
};

// `satisfies` (not an `Actions` annotation) so the concrete action keys
// survive for specs to call directly — the annotation erases them to an
// index signature under noUncheckedIndexedAccess.
export const actions = {
	/**
	 * Found an account: validate locally against {@link createAccountForm},
	 * then POST /accounts. A local validation failure fails 422 with the
	 * superform (field errors render per-input); a backend problem rides back
	 * as the same form's `message` via superforms' `message()`; success
	 * redirects to the clean listing (PRG). One channel throughout — values,
	 * field errors, and the backend `Problem` all ride the superform.
	 */
	default: async ({ request, fetch }) => {
		const form = await superValidate(request, effect(createAccountForm));
		if (!form.valid) {
			return fail(HttpStatus.UnprocessableContent, { form });
		}

		const outcome = await runApi(fetch, createAccountOutcome(form.data.name, form.data.handle));

		if ('problem' in outcome) return problemMessage(form, outcome.problem);
		redirect(HttpStatus.SeeOther, '/accounts');
	}
} satisfies Actions;
