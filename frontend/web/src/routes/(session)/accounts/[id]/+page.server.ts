import { accountOutcome, deleteAccountOutcome } from '$lib/server/accounts';
import { runApi } from '$lib/server/runtime';
import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { HttpStatus } from '$lib/api/http-status';
import { FORBIDDEN_PROBLEM, renderableStatus } from '$lib/api/problem';
import { superValidate } from 'sveltekit-superforms';
import { effect } from 'sveltekit-superforms/adapters';
import { deleteAccountForm } from '$lib/server/forms/delete-account';

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
	 * Owner-only delete, guarded in fail-closed order: role membership first
	 * (authorization precedes validation), then the type-the-handle confirm
	 * schema, and only then the DELETE — enforced server-side; the page's
	 * role gate is only an affordance. A problem re-renders in place; success
	 * redirects to the list carrying `?deleted=<outcome>` so it can say WHICH
	 * deletion happened (soft vs hard — DD 23003138 decision 6).
	 */
	delete: async ({ params, fetch, request }) => {
		const callerAccountOutcome = await runApi(fetch, accountOutcome(params.id));
		if ('problem' in callerAccountOutcome) {
			const { problem } = callerAccountOutcome;
			return fail(renderableStatus(problem), { problem });
		}

		const { account } = callerAccountOutcome;
		if (account.role !== 'owner') {
			return fail(HttpStatus.Forbidden, { problem: FORBIDDEN_PROBLEM });
		}

		const form = await superValidate(request, effect(deleteAccountForm(account.handle)));
		if (!form.valid) {
			return fail(HttpStatus.UnprocessableContent, { form });
		}

		const outcome = await runApi(fetch, deleteAccountOutcome(params.id));
		if ('problem' in outcome) {
			return fail(renderableStatus(outcome.problem), { problem: outcome.problem });
		}

		redirect(HttpStatus.SeeOther, `/accounts?deleted=${outcome.outcome}`);
	}
};
