import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import type { BugReportActionData, BugReportFailure, BugReportOffer } from '$lib/api/bug-report';
import { HttpStatus } from '$lib/api/http-status';
import type { Trail } from '$lib/api/trail';
import { fileBugReportOutcome, pageOfReferer, webBuildId } from '$lib/server/bug-report';
import { firstParty } from '$lib/server/first-party-origin';
import { echoOf, postedBugReport, readBugReportForm } from '$lib/server/forms/bug-report';
import { runApi } from '$lib/server/runtime';
import { parseBrowser } from '$lib/types/bug-report-rules';

/** The path bar on the report page: just "Report a bug". */
const REPORT_TRAIL: Trail = [
	{ step: 'named', label: 'Report a bug', ordinal: undefined, href: undefined }
];

/**
 * What the page offers to send: the page the reporter came from when the
 * `Referer` is exactly on this site (its path, with the archived words when
 * it showed archived items, and its page type), otherwise a "Which page?"
 * question; and the browser's user agent, cut to fit.
 */
export const load: PageServerLoad = ({ request }) => {
	const fromReferer = pageOfReferer(request.headers.get('referer') ?? undefined, firstParty);
	const browser = parseBrowser(request.headers.get('user-agent') ?? '');
	const offer: BugReportOffer = {
		page: fromReferer === undefined ? { kind: 'ask' } : { kind: 'known', page: fromReferer.page },
		pageType: fromReferer?.pageType,
		browser: browser.kind === 'accepted' ? browser.value : undefined
	};
	return { offer, trail: REPORT_TRAIL };
};

/** The posted form, or an empty one when the body isn't a form. */
async function formOf(request: Request): Promise<FormData> {
	return request.formData().catch(() => new FormData());
}

// `satisfies` (not an `Actions` annotation) so the concrete action keys
// survive for specs to call directly.
export const actions = {
	/**
	 * File a report: read the post through the form's schema (only ticked
	 * items, the server's own build, the page reduced to its path), then the
	 * API. A refusal fails 400 naming its item, the hourly limit 429, and
	 * anything that keeps it from going in 502, each with the reporter's own
	 * text; an ended session goes to sign in. The origin check is the root
	 * hook's, before this runs.
	 */
	default: async ({ request, fetch }) => {
		const posted = postedBugReport(await formOf(request));
		const echo = echoOf(posted);
		const read = readBugReportForm(posted, { firstParty, buildId: webBuildId() });
		if (read.kind === 'refused') {
			const refused: BugReportFailure = { ...read.refusal, ...echo };
			return fail(HttpStatus.BadRequest, refused);
		}
		const outcome = await runApi(fetch, fileBugReportOutcome(read.filing));
		switch (outcome.kind) {
			case 'sent': {
				const sent: BugReportActionData = { kind: 'sent', reference: outcome.reference };
				return sent;
			}
			case 'limit': {
				const limited: BugReportFailure = { kind: 'limit', ...echo };
				return fail(HttpStatus.TooManyRequests, limited);
			}
			case 'couldNotSend': {
				const unsent: BugReportFailure = { kind: 'couldNotSend', ...echo };
				return fail(HttpStatus.BadGateway, unsent);
			}
			case 'signedOut':
				redirect(HttpStatus.SeeOther, '/login');
		}
	}
} satisfies Actions;
