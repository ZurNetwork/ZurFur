import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { HttpStatus } from '$lib/api/http-status';

/**
 * Hands the frame the signed-in visitor. The sign-in gate itself is the
 * server `handle` hook (`lib/server/session-gate.ts`), which refuses every
 * `(session)` load, data request and action without a session before any of
 * them runs; the redirect here only narrows the type and never fires. The
 * backend still checks the session on every call.
 */
export const load: LayoutServerLoad = async ({ parent }) => {
	const { session } = await parent();
	if (session === undefined) redirect(HttpStatus.SeeOther, '/login');
	return { session };
};
