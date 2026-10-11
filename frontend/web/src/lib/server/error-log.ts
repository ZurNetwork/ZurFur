import type { HandleServerError } from '@sveltejs/kit';

/** The one message an unexpected server error carries to the page: fixed, so nothing internal rides it. */
export const UNEXPECTED_ERROR_MESSAGE = 'Internal Error';

/**
 * Logs an unexpected server error by its route template and status only —
 * never the error itself, the URL, a Den path, a segment or a page token,
 * since any of those can hold another person's ids — and hands the page a
 * fixed message.
 */
export const logUnexpectedError: HandleServerError = ({ event, status }) => {
	const route = event.route.id ?? '(no route)';
	console.error('[web] unexpected server error', { route, status });
	return { message: UNEXPECTED_ERROR_MESSAGE };
};
