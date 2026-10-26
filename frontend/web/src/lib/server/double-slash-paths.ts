import type { Handle } from '@sveltejs/kit';
import { HttpStatus } from '$lib/api/http-status';

/** The prefix no Zurfur page starts with: a pathname that opens with it reads as another host. */
const DOUBLE_SLASH = '//';

/** The fixed body of the refusal. */
export const DOUBLE_SLASH_REFUSAL = 'Not found';

/** Whether `pathname` starts with `//`. */
function startsWithDoubleSlash(pathname: string): boolean {
	return pathname.startsWith(DOUBLE_SLASH);
}

/** The plain-text 404 a refused path gets. */
function refusal(): Response {
	return new Response(DOUBLE_SLASH_REFUSAL, {
		status: HttpStatus.NotFound,
		headers: { 'content-type': 'text/plain' }
	});
}

/**
 * Refuses every request whose pathname starts with `//` with a plain-text 404,
 * before anything behind it runs, since a redirect built from such a path
 * leaves the site. Every other request goes on untouched.
 */
export const refuseDoubleSlashPaths: Handle = ({ event, resolve }) =>
	startsWithDoubleSlash(event.url.pathname) ? refusal() : resolve(event);
