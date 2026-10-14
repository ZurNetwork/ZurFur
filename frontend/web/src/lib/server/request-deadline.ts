import type { Handle } from '@sveltejs/kit';
import { countdown, TIME_LIMITS } from './time-limits';

/**
 * Starts the request's one deadline before any other hook runs and hands it
 * to `handleFetch` through `locals`, so every backend call the request makes
 * stops by then; the clock stops once the response is ready.
 */
export const requestDeadline: Handle = async ({ event, resolve }) => {
	const clock = countdown(TIME_LIMITS.request);
	event.locals.deadline = clock.signal;
	try {
		return await resolve(event);
	} finally {
		clock.stop();
	}
};

/** `request`, aborted by the earlier of its own signal and `deadline` (when there is one). */
export function withinDeadline(request: Request, deadline: AbortSignal | undefined): Request {
	if (deadline === undefined) return request;
	return new Request(request, { signal: AbortSignal.any([request.signal, deadline]) });
}
