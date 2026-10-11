import { redirect, type Handle, type RequestEvent } from '@sveltejs/kit';
import { HttpStatus } from '$lib/api/http-status';
import { runApi } from './runtime';
import { sessionOrAnonymous } from './session';

/** The route group every signed-in page, data request and form action lives in. */
const SESSION_GROUP = '/(session)';

/** Whether `routeId` belongs to the signed-in group. */
export function isSessionRoute(routeId: RequestEvent['route']['id']): boolean {
	return routeId === SESSION_GROUP || routeId?.startsWith(`${SESSION_GROUP}/`) === true;
}

/**
 * The sign-in gate, in one place: before any load or action of a `(session)`
 * route runs — page, `__data.json` (whatever loads it asks to skip) or form
 * action alike — it asks the backend who is signed in, and sends anyone who
 * isn't to `/login`. The session it found rides `locals` for the root
 * layout, so the request makes one whoami, not two.
 */
export const sessionGate: Handle = async ({ event, resolve }) => {
	if (!isSessionRoute(event.route.id)) return resolve(event);
	const session = await runApi(event.fetch, sessionOrAnonymous);
	if (session === undefined) redirect(HttpStatus.SeeOther, '/login');
	event.locals.session = session;
	return resolve(event);
};
