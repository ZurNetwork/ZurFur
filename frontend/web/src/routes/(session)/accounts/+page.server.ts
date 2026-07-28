import { accountsOutcome, createAccountOutcome } from '$lib/server/accounts';
import { runApi } from '$lib/server/runtime';
import { fail, redirect } from '@sveltejs/kit';
import type { PageServerLoad, Actions } from './$types';
import type { Problem } from '$lib/api/problem';

export const load: PageServerLoad = async ({ fetch, url }) => {
	const outcome = await runApi(fetch, accountsOutcome);
	const deleted = url.searchParams.get('deleted');

	return { ...outcome, deleted };
};

const MISSING_PARAMETERS_PROBLEM: Problem = {
	type: 'urn:zurfur:error:invalid-request',
	code: 'invalid_request',
	title: 'Invalid values.',
	detail: 'You need to enter valid values to proceed.',
	status: 422
};

export const actions: Actions = {
	default: async ({ request, fetch }) => {
		const form = await request.formData();
		const formName = form.get('name');
		const name = typeof formName === 'string' ? formName.trim() : '';

		const formHandle = form.get('handle');
		const handle = typeof formHandle === 'string' ? formHandle.trim() : '';

		if (name === '' || handle === '') return fail(422, { problem: MISSING_PARAMETERS_PROBLEM });
		const outcome = await runApi(fetch, createAccountOutcome(name, handle));

		if ('problem' in outcome) return fail(outcome.problem.status, { problem: outcome.problem });
		redirect(303, '/accounts');
	}
};
