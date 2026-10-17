import { fail, redirect } from '@sveltejs/kit';
import { superValidate } from 'sveltekit-superforms';
import { effect } from 'sveltekit-superforms/adapters';
import type { Actions, PageServerLoad } from './$types';
import { HttpStatus } from '$lib/api/http-status';
import { NODE_NOT_FOUND_PROBLEM } from '$lib/api/problem';
import {
	createCommissionOutcome,
	RENAME_COMMISSION_NOT_FOUND_PROBLEM,
	RENAME_NODE_NOT_FOUND_PROBLEM,
	renameCommissionOutcome
} from '$lib/server/commissions';
import { denPage } from '$lib/server/den';
import {
	denHrefOf,
	denPathFromPathname,
	denQueryFromSearch,
	PLAIN_QUERY
} from '$lib/server/den/den-path';
import { createCommissionField, createCommissionForm } from '$lib/server/forms/create-commission';
import {
	renameCommissionField,
	renameCommissionForm,
	renameTarget
} from '$lib/server/forms/rename-commission';
import { problemOnFields } from '$lib/server/forms/field-errors';
import { problemMessage } from '$lib/server/forms/problem-message';
import { runApi } from '$lib/server/runtime';
import { runRead } from '$lib/server/unanswered';

/**
 * The node at a Den address — the only code that reads the Den. One route
 * serves the root (`/den`) and everything below it. On a full page load it
 * also reads the folders above the node, so the first paint shows the tree
 * opened down to it; an in-app navigation reads only the node. A refused
 * address, and every miss, is the one not-found, answered 200 with the
 * not-found in the page. A stale page token reloads the folder from its
 * first page, once; an ended session goes to sign in; a backend that
 * doesn't answer fails to the error view as "isn't answering". The page
 * also gets pristine "New commission" and "Rename" forms.
 */
export const load: PageServerLoad = async ({ fetch, url, isDataRequest }) => {
	const result = await runRead(fetch, denPage(url.pathname, url.searchParams, !isDataRequest));
	if (result.result === 'reload') redirect(HttpStatus.SeeOther, result.location);
	if (result.result === 'signedOut') redirect(HttpStatus.SeeOther, '/login');
	const createCommission = await superValidate(effect(createCommissionForm));
	const renameCommission = await superValidate(effect(renameCommissionForm));
	return { den: result.data, trail: result.data.trail, createCommission, renameCommission };
};

// `satisfies` (not an `Actions` annotation) so the concrete action keys
// survive for specs to call directly.
export const actions = {
	/**
	 * Create a commission from the folder at this address, then land on its
	 * entry there (303), or on the folder when the entry isn't found. A
	 * refused title shows under its field; any other problem, and a refused
	 * address, shows as the form's message.
	 */
	createCommission: async ({ request, fetch, url }) => {
		const form = await superValidate(request, effect(createCommissionForm));
		if (!form.valid) return fail(HttpStatus.UnprocessableContent, { form });
		const folder = denPathFromPathname(url.pathname);
		if (folder === undefined) return problemMessage(form, NODE_NOT_FOUND_PROBLEM);

		const outcome = await runApi(fetch, createCommissionOutcome(folder, form.data.title));

		if ('problem' in outcome) return problemOnFields(form, outcome.problem, createCommissionField);
		redirect(HttpStatus.SeeOther, outcome.landing);
	},

	/**
	 * Rename the commission the hidden field names, then go back to this
	 * address (303), keeping only the post's "Show archived" flag. An id that
	 * doesn't mint is the commission not-found and calls nothing. A refused
	 * title shows under its field; any other problem, and a refused address,
	 * shows as the form's message.
	 */
	renameCommission: async ({ request, fetch, url }) => {
		const form = await superValidate(request, effect(renameCommissionForm));
		if (!form.valid) return fail(HttpStatus.UnprocessableContent, { form });
		const id = renameTarget(form.data.id);
		if (id === undefined) return problemMessage(form, RENAME_COMMISSION_NOT_FOUND_PROBLEM);
		const here = denPathFromPathname(url.pathname);
		const { includeDeleted } = denQueryFromSearch(url.searchParams);
		const back =
			here === undefined ? undefined : denHrefOf(here, { ...PLAIN_QUERY, includeDeleted });
		if (back === undefined) return problemMessage(form, RENAME_NODE_NOT_FOUND_PROBLEM);

		const outcome = await runApi(fetch, renameCommissionOutcome(id, form.data.title));

		if ('problem' in outcome) return problemOnFields(form, outcome.problem, renameCommissionField);
		redirect(HttpStatus.SeeOther, back);
	}
} satisfies Actions;
