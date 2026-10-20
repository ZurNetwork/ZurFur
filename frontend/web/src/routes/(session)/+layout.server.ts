import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import type { TestStack } from '$lib/api/bug-report';
import { HttpStatus } from '$lib/api/http-status';
import { mockModeEnabled } from '$lib/server/api/zurfur-api-mock';
import { onDevProfileStack, webBuildId } from '$lib/server/bug-report';

/**
 * Hands the frame the signed-in visitor. The sign-in gate itself is the
 * server `handle` hook (`lib/server/session-gate.ts`), which refuses every
 * `(session)` load, data request and action without a session before any of
 * them runs. The one request it lets through is the sign-out route's
 * default action, so the redirect here only narrows the type (a backstop
 * should anything else on that route ever render). The backend still
 * checks the session on every call. The frame also gets the web's own
 * build id ("unknown" when ZURFUR_BUILD_ID is unset) and the kind of stack
 * it runs on, which the report form tells the reporter when it doesn't
 * keep reports.
 */
export const load: LayoutServerLoad = async ({ parent }) => {
	const { session } = await parent();
	if (session === undefined) redirect(HttpStatus.SeeOther, '/login');
	return { session, buildId: webBuildId(), testStack: testStack() };
};

/** Mock mode keeps nothing at all, so it wins over the dev profile's stack. */
function testStack(): TestStack {
	if (mockModeEnabled()) return 'mock';
	if (onDevProfileStack()) return 'devProfile';
	return 'none';
}
