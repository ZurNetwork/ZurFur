import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Effect, Fiber } from 'effect';
import type { FetchFunction } from '$lib/api/client';
import { hangingFetch } from '$lib/testing/http';
import { RequestFetch, ZurfurApi, ZurfurApiLive } from './zurfur-api';
import type { ZurfurApiError } from './errors';

/** Start `program` against the live layer over `fetch`, its failure (or success) as the value. */
function settle<A>(
	fetch: FetchFunction,
	program: Effect.Effect<A, ZurfurApiError, ZurfurApi>
): Promise<A | ZurfurApiError> {
	const provided = program.pipe(
		Effect.merge,
		Effect.provide(ZurfurApiLive),
		Effect.provideService(RequestFetch, fetch)
	);
	return Effect.runPromise(provided);
}

/** Whether `promise` has settled by now. */
async function hasSettled(promise: Promise<unknown>): Promise<boolean> {
	let settled = false;
	void promise.then(
		() => (settled = true),
		() => (settled = true)
	);
	await vi.advanceTimersByTimeAsync(0);
	return settled;
}

const me = Effect.flatMap(ZurfurApi, (api) => api.me);
const signout = Effect.flatMap(ZurfurApi, (api) => api.signout);
const listAccounts = Effect.flatMap(ZurfurApi, (api) => api.listAccounts);
const startSignin = Effect.flatMap(ZurfurApi, (api) => api.startSignin('alice.test'));
const createAccount = Effect.flatMap(ZurfurApi, (api) => api.createAccount('Studio', 's.test'));
const denRoot = Effect.flatMap(ZurfurApi, (api) =>
	api.denRoot({ includeDeleted: false, pageToken: undefined })
);

/** Each backend call, with the limit it stops waiting at. */
const LIMITED_CALLS: readonly (readonly [
	string,
	number,
	Effect.Effect<unknown, ZurfurApiError, ZurfurApi>
])[] = [
	['the session check (/me)', 5_000, me],
	['sign-out', 5_000, signout],
	['the account listing', 10_000, listAccounts],
	['a sign-in start', 10_000, startSignin],
	['founding an Account', 10_000, createAccount],
	['a Den read', 10_000, denRoot]
];

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	vi.useRealTimers();
});

describe('the time limit on every backend call', () => {
	it.each(LIMITED_CALLS)(
		'%s stops waiting at its own limit, %i ms, and fails as a timeout',
		async (_, limit, program) => {
			const { fetch } = hangingFetch();
			const settled = settle(fetch, program);

			await vi.advanceTimersByTimeAsync(limit - 1);
			expect(await hasSettled(settled)).toBe(false);
			await vi.advanceTimersByTimeAsync(1);

			expect(await settled).toMatchObject({ _tag: 'NetworkFailure', reason: 'timeout' });
		}
	);

	it('fails as a timeout at the limit even when the fetch ignores its signal and never ends', async () => {
		const deaf: FetchFunction = () => new Promise<Response>(() => undefined);
		const settled = settle(deaf, me);

		await vi.advanceTimersByTimeAsync(5_000);
		expect(await hasSettled(settled)).toBe(true);

		expect(await settled).toMatchObject({ _tag: 'NetworkFailure', reason: 'timeout' });
	});

	it('bounds the body too: an answer whose body never ends fails as a timeout', async () => {
		const fetch: FetchFunction = (_input, init) => {
			const signal = init?.signal;
			const body = new ReadableStream<Uint8Array>({
				start(controller) {
					controller.enqueue(new TextEncoder().encode('{"did":'));
					signal?.addEventListener('abort', () => {
						controller.error(signal.reason);
					});
				}
			});
			return Promise.resolve(new Response(body, { status: 200 }));
		};
		const settled = settle(fetch, me);

		await vi.advanceTimersByTimeAsync(5_000);

		expect(await settled).toMatchObject({ _tag: 'NetworkFailure', reason: 'timeout' });
	});

	it('stops the clock once the call is answered, leaving no timer behind', async () => {
		const fetch: FetchFunction = () => Promise.resolve(Response.json({ did: 'did:plc:alice' }));
		await settle(fetch, me);

		expect(vi.getTimerCount()).toBe(0);
	});

	it('stops the fetch when the call is interrupted', async () => {
		const { fetch } = hangingFetch();
		const signals: AbortSignal[] = [];
		const watching: FetchFunction = (input, init) => {
			if (init?.signal) signals.push(init.signal);
			return fetch(input, init);
		};
		const provided = listAccounts.pipe(
			Effect.provide(ZurfurApiLive),
			Effect.provideService(RequestFetch, watching)
		);
		const fiber = Effect.runFork(provided);
		await vi.advanceTimersByTimeAsync(0);

		await Effect.runPromise(Fiber.interrupt(fiber));

		expect(signals.map((signal) => signal.aborted)).toEqual([true]);
		expect(vi.getTimerCount()).toBe(0);
	});
});
