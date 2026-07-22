import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';

/**
 * The session gate (ZMVP-151): every route in the `(session)` group requires
 * a signed-in visitor; anonymous visits bounce to `/login`. Future session
 * routes (`/accounts`, `/commissions`, …) join the group instead of
 * re-implementing the check. UX only — the backend still 401s on its own.
 */
export const load: LayoutServerLoad = async ({ parent }) => {
	const { session } = await parent();
	if (session === null) redirect(303, '/login');
	return {};
};
