import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { HttpStatus } from '$lib/api/http-status';

/**
 * The session gate (ZMVP-151): every route in the `(session)` group requires
 * a signed-in visitor; anonymous visits bounce to `/login`. Future session
 * routes (`/accounts`, `/commissions`, …) join the group instead of
 * re-implementing the check. UX only — the backend still 401s on its own.
 * PRECISION: this gates page LOADS; SvelteKit runs form-action POSTs before
 * layout loads, so an action inside the group executes without this check —
 * the backend's own 401 (surfacing as the action's `{problem}`) is the guard
 * there.
 */
export const load: LayoutServerLoad = async ({ parent }) => {
	const { session } = await parent();
	if (session === undefined) redirect(HttpStatus.SeeOther, '/login');
	return {};
};
