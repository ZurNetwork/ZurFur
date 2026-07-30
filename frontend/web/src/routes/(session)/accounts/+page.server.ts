import { accountsOutcome, createAccountOutcome } from '$lib/server/accounts';
import { runApi } from '$lib/server/runtime';
import { fail, redirect } from '@sveltejs/kit';
import type { PageServerLoad, Actions } from './$types';
import { type Problem, ProblemKind, renderableStatus } from '$lib/api/problem';
import type { DeleteOutcome } from '$lib/api/account';
import { HttpStatus } from '$lib/api/http-status';

/**
 * Every outcome a completed delete can carry back via `?deleted=` — keyed as
 * a `Record<DeleteOutcome, true>` so adding a fourth outcome to the union is
 * a compile error here, not a silently vanished banner. The banner renders
 * only from this declared vocabulary; it is a flash HINT, not proof a
 * deletion occurred (anyone can type the query param).
 */
const DELETE_OUTCOMES = { soft: true, hard: true, unknown: true } as const satisfies Record<
	DeleteOutcome,
	true
>;

/**
 * The caller's account listing, plus the `?deleted=` flash a completed delete
 * redirects back with — narrowed against {@link DeleteOutcome} so the page
 * renders from the declared vocabulary, not from raw query text.
 */
export const load: PageServerLoad = async ({ fetch, url }) => {
	const outcome = await runApi(fetch, accountsOutcome);
	const deletedParam = url.searchParams.get('deleted');
	const deleted =
		deletedParam !== null && Object.hasOwn(DELETE_OUTCOMES, deletedParam)
			? (deletedParam as DeleteOutcome)
			: undefined;

	return { ...outcome, deleted };
};

/**
 * Rendered through the same problem seam as backend problems, but minted
 * locally — an empty name or handle never needs a round-trip. Same shape,
 * same rendering path ({@link import('$lib/components/ProblemNote.svelte')}).
 */
const MISSING_PARAMETERS_PROBLEM: Problem = {
	...ProblemKind.InvalidRequest,
	title: 'Invalid values.',
	detail: 'You need to enter valid values to proceed.',
	status: HttpStatus.UnprocessableContent
};

export const actions: Actions = {
	/**
	 * Found an account: validate locally, then POST /accounts. A problem
	 * re-renders the form with the typed values riding back on the fail
	 * payload (SvelteKit's repopulate-from-fail pattern — neither field is a
	 * secret); success redirects to the clean listing (PRG).
	 */
	default: async ({ request, fetch }) => {
		const form = await request.formData();
		const formName = form.get('name');
		const name = typeof formName === 'string' ? formName.trim() : '';

		const formHandle = form.get('handle');
		const handle = typeof formHandle === 'string' ? formHandle.trim() : '';

		if (name === '' || handle === '')
			return fail(HttpStatus.UnprocessableContent, {
				problem: MISSING_PARAMETERS_PROBLEM,
				name,
				handle
			});
		const outcome = await runApi(fetch, createAccountOutcome(name, handle));

		if ('problem' in outcome)
			return fail(renderableStatus(outcome.problem), { problem: outcome.problem, name, handle });
		redirect(HttpStatus.SeeOther, '/accounts');
	}
};
