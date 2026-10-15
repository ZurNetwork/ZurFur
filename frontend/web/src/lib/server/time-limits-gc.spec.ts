/**
 * The time limits against real undici and a real local server that never
 * answers, with garbage collection forced while each call waits: the call
 * must still fail on time, and its connection must close. Only `setTimeout`
 * is faked, so the sockets run on real time.
 */

import { createServer, type Server, type ServerResponse } from 'node:http';
import type { Socket } from 'node:net';
import { setFlagsFromString } from 'node:v8';
import { runInNewContext } from 'node:vm';
import type { RequestEvent } from '@sveltejs/kit';
import { Effect } from 'effect';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { kitEventFetch } from '$lib/testing/kit-fetch';
import { RequestFetch, ZurfurApi, ZurfurApiLive } from './api/zurfur-api';
import type { ZurfurApiError } from './api/errors';
import { countdown, TIME_LIMITS } from './time-limits';

/** Where the local backend listens, once it does. */
const upstream = vi.hoisted(() => ({ origin: 'http://127.0.0.1:1' }));
vi.mock('$env/dynamic/private', () => ({
	env: {
		get ZURFUR_API_UPSTREAM() {
			return upstream.origin;
		}
	}
}));

const { handleFetch } = await import('../../hooks.server');

/** V8's own collector, reached the way Node allows at run time. */
const collectGarbage = ((): (() => void) => {
	setFlagsFromString('--expose_gc');
	const gc: unknown = runInNewContext('gc');
	if (typeof gc !== 'function') throw new Error('gc is not exposed');
	return () => {
		Reflect.apply(gc, undefined, []);
	};
})();

/** How the backend treats a request: never answer, or send headers and part of a body, then stall. */
let behaviour: 'hang' | 'stall' = 'hang';
/** Every request the backend has seen, resolved as each arrives. */
let arrived: Promise<void> = Promise.resolve();
let markArrived: () => void = () => undefined;
const sockets = new Set<Socket>();
/** Responses the backend holds open, ended in cleanup. */
const held: ServerResponse[] = [];
let server: Server;

function expectArrival(): void {
	arrived = new Promise((resolve) => {
		markArrived = resolve;
	});
}

beforeAll(async () => {
	server = createServer((_request, response) => {
		held.push(response);
		if (behaviour === 'stall') {
			response.writeHead(200, { 'content-type': 'application/json' });
			response.write('{"did":');
		}
		markArrived();
	});
	server.on('connection', (socket) => {
		sockets.add(socket);
		socket.on('close', () => sockets.delete(socket));
	});
	await new Promise<void>((resolve) => {
		server.listen(0, '127.0.0.1', resolve);
	});
	const address = server.address();
	if (address === null || typeof address === 'string') throw new Error('expected a TCP address');
	upstream.origin = `http://127.0.0.1:${String(address.port)}`;
});

afterEach(() => {
	vi.useRealTimers();
	for (const response of held.splice(0)) response.destroy();
	for (const socket of sockets) socket.destroy();
	behaviour = 'hang';
});

afterAll(
	() =>
		new Promise<void>((resolve) => {
			server.close(() => {
				resolve();
			});
		})
);

/** `event.fetch` as SvelteKit builds it for one request, with this app's `handleFetch` and a 15 s deadline. */
function requestFetch(): typeof fetch {
	const url = new URL('http://127.0.0.1:5174/den');
	const request = new Request(url, { headers: { cookie: 'zurfur.sid=s3ss10n' } });
	const locals: App.Locals = { deadline: countdown(TIME_LIMITS.request).signal };
	const event: Pick<RequestEvent, 'url' | 'request' | 'locals'> = { url, request, locals };
	return kitEventFetch(event, handleFetch);
}

/** Wait `ms` of fake time, collecting garbage every second of it. */
async function waitCollecting(ms: number): Promise<void> {
	for (let waited = 0; waited < ms; waited += 1_000) {
		collectGarbage();
		await vi.advanceTimersByTimeAsync(Math.min(1_000, ms - waited));
	}
}

/** Whether every connection to the backend has closed, given real time to do so. */
async function connectionsClosed(): Promise<boolean> {
	for (let tries = 0; tries < 100 && sockets.size > 0; tries += 1) {
		await new Promise((resolve) => setImmediate(resolve));
	}
	return sockets.size === 0;
}

/** Start `program` over the live port through SvelteKit's fetch, its failure (or success) as the value. */
function startCall(program: Effect.Effect<unknown, ZurfurApiError, ZurfurApi>) {
	let settled = false;
	const call = Effect.runPromise(
		program.pipe(
			Effect.merge,
			Effect.provide(ZurfurApiLive),
			Effect.provideService(RequestFetch, requestFetch())
		)
	).finally(() => {
		settled = true;
	});
	return { call, settled: () => settled };
}

const me = Effect.flatMap(ZurfurApi, (api) => api.me);
const listAccounts = Effect.flatMap(ZurfurApi, (api) => api.listAccounts);

describe('the time limits with garbage collection forced while the call waits', () => {
	it.each([
		['the session check (/me) that never answers', 'hang', 5_000, me],
		['a call that never answers', 'hang', 10_000, listAccounts],
		['the session check whose body stalls', 'stall', 5_000, me]
	] as const)(
		'%s stops at its own limit and closes its connection',
		async (_, mode, limit, program) => {
			vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
			behaviour = mode;
			expectArrival();
			const { call, settled } = startCall(program);
			await arrived;

			await waitCollecting(limit - 1);
			expect(settled()).toBe(false);
			await waitCollecting(1);

			expect(await call).toMatchObject({ _tag: 'NetworkFailure', reason: 'timeout' });
			expect(await connectionsClosed()).toBe(true);
		}
	);

	it('stops a call with no limit of its own at the request’s 15 s deadline', async () => {
		vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
		expectArrival();
		let settled = false;
		const call = requestFetch()('/api/v1/raw').then(
			() => 'answered',
			(failure: unknown) => (failure instanceof Error ? failure.name : 'failed')
		);
		void call.finally(() => (settled = true));
		await arrived;

		await waitCollecting(TIME_LIMITS.request - 1);
		expect(settled).toBe(false);
		await waitCollecting(1);

		expect(await call).toBe('TimeoutError');
		expect(await connectionsClosed()).toBe(true);
	});
});
