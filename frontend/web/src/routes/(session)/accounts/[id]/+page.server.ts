import { accountOutcome, deleteAccountOutcome } from '$lib/server/accounts';
import { runApi } from '$lib/server/runtime';
import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, fetch }) => {
	return await runApi(fetch, accountOutcome(params.id));
};

export const actions: Actions = {
	delete: async ({ params, fetch }) => {
		const outcome = await runApi(fetch, deleteAccountOutcome(params.id));

		if ('problem' in outcome) {
			return fail(outcome.problem.status, { problem: outcome.problem });
		}

		redirect(303, `/accounts?deleted=${outcome.outcome}`);
	}
};
