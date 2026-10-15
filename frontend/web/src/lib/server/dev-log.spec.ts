import type { HandleFetch, RequestEvent } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { type LogSink, logRequests, logUpstreamCalls } from './dev-log';

type HandleInput = Parameters<ReturnType<typeof logRequests>>[0];
type FetchInput = Parameters<HandleFetch>[0];

/** A Den address holding a DID in its path and an OAuth code and state in its query. */
const PLANTED_URL =
	'http://127.0.0.1:5174/den/accounts/did:plc:plantedpathdid?code=planted-code&state=planted-state';

/** The visitor's cookie jar, its session cookie planted. */
const PLANTED_COOKIE = 'zurfur.sid=planted-session-cookie; other=planted-other';

/** Every value planted on the request, none of which may reach a log line. */
const PLANTED = [
	'plantedpathdid',
	'planted-code',
	'planted-state',
	'planted-session-cookie',
	'planted-other',
	'/den/accounts'
];

/** A sink that keeps every line, as text. */
function recordingSink(): { sink: LogSink; lines: string[] } {
	const lines: string[] = [];
	const sink: LogSink = (message, fields) => {
		lines.push(`${message} ${JSON.stringify(fields)}`);
	};
	return { sink, lines };
}

/** The request event SvelteKit hands a hook for `PLANTED_URL` at the Den's route. */
function plantedEvent(): RequestEvent {
	const request = new Request(PLANTED_URL, { headers: { cookie: PLANTED_COOKIE } });
	const event = {
		route: { id: '/(session)/den/[...path]' },
		request,
		url: new URL(PLANTED_URL),
		locals: {}
	};
	return event as unknown as RequestEvent;
}

/** Runs the request log once around a page answering `status`. */
async function runRequestLog(enabled: boolean, status: number): Promise<string[]> {
	const { sink, lines } = recordingSink();
	const hook = logRequests(enabled, sink);
	const resolve = () => Promise.resolve(new Response('<p>a page</p>', { status }));
	const input: HandleInput = { event: plantedEvent(), resolve };
	await hook(input);
	return lines;
}

/** Runs the upstream log once around an upstream call answering `status`. */
async function runUpstreamLog(enabled: boolean, status: number): Promise<string[]> {
	const { sink, lines } = recordingSink();
	const upstream: HandleFetch = () => Promise.resolve(new Response('{}', { status }));
	const hook = logUpstreamCalls(enabled, sink, upstream);
	const request = new Request(
		'http://127.0.0.1:8081/den/accounts/did:plc:plantedpathdid?pageToken=planted-code',
		{ headers: { cookie: PLANTED_COOKIE, authorization: 'Bearer planted-state' } }
	);
	const input: FetchInput = { event: plantedEvent(), request, fetch };
	await hook(input);
	return lines;
}

describe('logRequests', () => {
	it('logs one line per request: method, route template, status and time', async () => {
		const lines = await runRequestLog(true, 404);

		expect(lines).toHaveLength(1);
		const [line] = lines;
		expect(line).toContain('[web] debug request');
		expect(line).toContain('"method":"GET"');
		expect(line).toContain('"route":"/(session)/den/[...path]"');
		expect(line).toContain('"status":404');
		expect(line).toMatch(/"ms":\d+/);
	});

	it('never logs the URL, the query, a path segment or a cookie', async () => {
		const lines = await runRequestLog(true, 200);

		for (const planted of PLANTED) {
			expect(lines.join('\n')).not.toContain(planted);
		}
	});

	it('names a request that matched no route by a fixed word', async () => {
		const { sink, lines } = recordingSink();
		const event = { ...plantedEvent(), route: { id: null } } as unknown as RequestEvent;
		const resolve = () => Promise.resolve(new Response(null, { status: 404 }));

		await logRequests(true, sink)({ event, resolve });

		expect(lines.join('\n')).toContain('"route":"(no route)"');
	});

	it('logs nothing when it is off, and still answers', async () => {
		const { sink, lines } = recordingSink();
		const resolve = () => Promise.resolve(new Response('<p>a page</p>', { status: 201 }));

		const response = await logRequests(false, sink)({ event: plantedEvent(), resolve });

		expect(lines).toEqual([]);
		expect(response.status).toBe(201);
	});
});

describe('logUpstreamCalls', () => {
	it('logs one line per upstream call: route template, method, status and time', async () => {
		const lines = await runUpstreamLog(true, 503);

		expect(lines).toHaveLength(1);
		const [line] = lines;
		expect(line).toContain('[web] debug upstream');
		expect(line).toContain('"route":"/(session)/den/[...path]"');
		expect(line).toContain('"method":"GET"');
		expect(line).toContain('"status":503');
		expect(line).toMatch(/"ms":\d+/);
	});

	it('never logs the upstream URL, its query, a cookie or a header', async () => {
		const lines = await runUpstreamLog(true, 200);

		for (const planted of [...PLANTED, '8081', 'Bearer']) {
			expect(lines.join('\n')).not.toContain(planted);
		}
	});

	it('logs a call that failed, without its error', async () => {
		const { sink, lines } = recordingSink();
		const failing: HandleFetch = () =>
			Promise.reject(new TypeError('fetch failed for http://127.0.0.1:8081/planted-code'));
		const hook = logUpstreamCalls(true, sink, failing);
		const request = new Request('http://127.0.0.1:8081/me');
		const input: FetchInput = { event: plantedEvent(), request, fetch };

		await expect(hook(input)).rejects.toThrow(TypeError);

		expect(lines).toHaveLength(1);
		expect(lines[0]).toContain('"status":"failed"');
		expect(lines[0]).not.toContain('planted-code');
	});

	it('logs nothing when it is off', async () => {
		const lines = await runUpstreamLog(false, 200);

		expect(lines).toEqual([]);
	});
});
