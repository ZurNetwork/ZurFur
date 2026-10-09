import { redirect, type Handle, type RequestEvent } from '@sveltejs/kit';
import { HttpStatus } from '$lib/api/http-status';
import { runApi } from './runtime';
import { sessionOrAnonymous } from './session';

/** The route group every signed-in page, data request and form action lives in. */
const SESSION_GROUP = '/(session)';

/** The sign-out action's route: it reads nothing, so it runs without a session. */
const SIGN_OUT_ROUTE = '/(session)/logout';

/**
 * Whether this request is the sign-out action, which must run even when the
 * backend can't confirm a session, so it can always clear the browser's.
 */
function isSignOutAction(event: RequestEvent): boolean {
	return event.route.id === SIGN_OUT_ROUTE && event.request.method === 'POST';
}

/** Whether `routeId` belongs to the signed-in group. */
export function isSessionRoute(routeId: RequestEvent['route']['id']): boolean {
	return routeId === SESSION_GROUP || routeId?.startsWith(`${SESSION_GROUP}/`) === true;
}

/**
 * The sign-in gate, in one place: before any load or action of a `(session)`
 * route runs — page, `__data.json` (whatever loads it asks to skip) or form
 * action alike — it asks the backend who is signed in, and sends anyone who
 * isn't to `/login`. The session it found rides `locals` for the root
 * layout, so the request makes one whoami, not two. The one exception is
 * the sign-out action: it reads nothing, and must clear the browser's
 * session even when the backend is down.
 */
export const sessionGate: Handle = async ({ event, resolve }) => {
	if (!isSessionRoute(event.route.id) || isSignOutAction(event)) return resolve(event);
	const session = await runApi(event.fetch, sessionOrAnonymous);
	if (session === undefined) redirect(HttpStatus.SeeOther, '/login');
	event.locals.session = session;
	return resolve(event);
};
