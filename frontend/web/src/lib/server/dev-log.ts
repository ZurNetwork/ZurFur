import type { Handle, HandleFetch } from '@sveltejs/kit';

/** Where a dev log line goes: the console in the app, a recorder in a test. */
export type LogSink = (message: string, fields: Readonly<Record<string, string | number>>) => void;

/** The route a request that matched none is logged under. */
export const NO_ROUTE = '(no route)';

/** The sink the app logs to: `console.debug`, which the dev log file keeps. */
export const consoleSink: LogSink = (message, fields) => {
	console.debug(message, fields);
};

/** Whole milliseconds since `start`, a `performance.now()` reading. */
function since(start: number): number {
	return Math.round(performance.now() - start);
}

/**
 * When `enabled`, logs one line per request to `sink`: its method, route
 * template, status and time. Never the URL, the query, a path segment, a
 * cookie or a header, any of which can carry a code, a token or a DID.
 */
export function logRequests(enabled: boolean, sink: LogSink): Handle {
	if (!enabled) return ({ event, resolve }) => resolve(event);
	return async ({ event, resolve }) => {
		const start = performance.now();
		const response = await resolve(event);
		sink('[web] debug request', {
			method: event.request.method,
			route: event.route.id ?? NO_ROUTE,
			status: response.status,
			ms: since(start)
		});
		return response;
	};
}

/**
 * `upstream`, logging one line per call to `sink` when `enabled`: the page's
 * route template, the call's method, its status (`failed` when it never
 * answered) and its time. Never the URL, its path or query, a cookie or a header.
 */
export function logUpstreamCalls(
	enabled: boolean,
	sink: LogSink,
	upstream: HandleFetch
): HandleFetch {
	if (!enabled) return upstream;
	return (input) => {
		const start = performance.now();
		const route = input.event.route.id ?? NO_ROUTE;
		const method = input.request.method;
		const called = Promise.resolve(upstream(input));
		void called.then(
			(response) => {
				sink('[web] debug upstream', { route, method, status: response.status, ms: since(start) });
			},
			() => {
				sink('[web] debug upstream', { route, method, status: 'failed', ms: since(start) });
			}
		);
		return called;
	};
}
