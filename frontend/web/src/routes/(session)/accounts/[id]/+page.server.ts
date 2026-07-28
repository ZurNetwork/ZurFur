import { accountOutcome, deleteAccountOutcome } from '$lib/server/accounts';
import { runApi } from '$lib/server/runtime';
import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { HttpStatus } from '$lib/api/http-status';
import { ProblemCode, ProblemType, type Problem } from '$lib/api/problem';
import { superValidate } from 'sveltekit-superforms';
import { effect } from 'sveltekit-superforms/adapters';
import { deleteAccountForm } from '$lib/server/forms/delete-account';

const INCORRECT_ROLE_FOR_OPERATION_PROBLEM: Problem = {
	type: ProblemType.Forbidden,
	code: ProblemCode.Forbidden,
	title: 'You are not allowed to do this',
	detail: 'You  do not have the powers to do this',
	status: HttpStatus.Forbidden
};
/**
 * The account detail, derived from the caller's own listing (ruling F1 — no
 * GetAccount rpc in v1): an id the caller holds no role in produces the same
 * not-found problem the backend would mint, rendered in-page. Deliberately a
 * 200 + problem body, not `error(404)` — the problem seam stays uniform and
 * keeps the backend's detail copy (Engineer ruling 2026-07-28); an honest 404
 * status waits for a machine consumer that reads one.
 */
export const load: PageServerLoad = async ({ params, fetch }) => {
	return await runApi(fetch, accountOutcome(params.id));
};

export const actions: Actions = {
	/**
	 * Owner-only delete — enforced server-side; the page's role gate is only
	 * an affordance. A problem re-renders in place; success redirects to the
	 * list carrying `?deleted=<outcome>` so it can say WHICH deletion happened
	 * (soft vs hard — DD 23003138 decision 6).
	 */
	delete: async ({ params, fetch, request }) => {
		const isSameAccountOutcome = await runApi(fetch, accountOutcome(params.id));
		if ('problem' in isSameAccountOutcome) {
			return fail(isSameAccountOutcome.problem.status, { problem: isSameAccountOutcome.problem });
		}

		const { account } = isSameAccountOutcome;
		const form = await superValidate(request, effect(deleteAccountForm(account.handle)));
		if (!form.valid) {
			return fail(HttpStatus.UnprocessableContent, { form });
		}

		if (account.role !== 'owner') {
			// I know this is guarded in the UI.
			return fail(HttpStatus.Forbidden, { problem: INCORRECT_ROLE_FOR_OPERATION_PROBLEM });
		}
		const outcome = await runApi(fetch, deleteAccountOutcome(params.id));

		if ('problem' in outcome) {
			return fail(outcome.problem.status, { problem: outcome.problem });
		}

		redirect(HttpStatus.SeeOther, `/accounts?deleted=${outcome.outcome}`);
	}
};
