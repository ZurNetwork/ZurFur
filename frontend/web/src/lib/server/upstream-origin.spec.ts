/**
 * The web's writes, sent the way a page sends them during SSR (kit's real
 * `event.fetch`, this app's real `handleFetch`, real undici), arrive at a real
 * local server standing in for the API carrying `origin` equal to the page's
 * origin: the header the API's guard on state-changing requests checks.
 */

import { createServer, type Server } from 'node:http';
import type { RequestEvent } from '@sveltejs/kit';
import { Effect } from 'effect';
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { kitEventFetch } from '$lib/testing/kit-fetch';
import { commissionId } from '$lib/types/brand';
import { RequestFetch, ZurfurApi, ZurfurApiLive } from './api/zurfur-api';
import { countdown, TIME_LIMITS } from './time-limits';

/** Where the local API stand-in listens, once it does. */
const upstream = vi.hoisted(() => ({ origin: 'http://127.0.0.1:1' }));
vi.mock('$env/dynamic/private', () => ({
	env: {
		get ZURFUR_API_UPSTREAM() {
			return upstream.origin;
		}
	}
}));

const { handleFetch } = await import('../../hooks.server');

/** The origin the page was reached under: not the API stand-in's. */
const PAGE_ORIGIN = 'http://127.0.0.1:5174';

/** One request as it reached the API stand-in. */
interface Arrival {
	readonly method: string | undefined;
	readonly path: string | undefined;
	readonly origin: readonly string[] | undefined;
}

/** Every request the API stand-in has seen since the last reset. */
const arrivals: Arrival[] = [];
let server: Server;

beforeAll(async () => {
	server = createServer((request, response) => {
		const arrival: Arrival = {
			method: request.method,
			path: request.url,
			origin: request.headersDistinct.origin
		};
		arrivals.push(arrival);
		request.resume();
		request.on('end', () => {
			response.writeHead(204).end();
		});
	});
	await new Promise<void>((resolve) => {
		server.listen(0, '127.0.0.1', resolve);
	});
	const address = server.address();
	if (address === null || typeof address === 'string') throw new Error('expected a TCP address');
	upstream.origin = `http://127.0.0.1:${String(address.port)}`;
});

beforeEach(() => {
	arrivals.splice(0);
});

afterAll(
	() =>
		new Promise<void>((resolve) => {
			server.close(() => {
				resolve();
			});
		})
);

/** `event.fetch` as SvelteKit builds it for a request to `page`, with this app's `handleFetch` and a 15 s deadline. */
function requestFetch(page: string, routeId: RequestEvent['route']['id']): typeof fetch {
	const url = new URL(page, PAGE_ORIGIN);
	const request = new Request(url, { method: 'POST', headers: { cookie: 'zurfur.sid=s3ss10n' } });
	const locals: App.Locals = { deadline: countdown(TIME_LIMITS.request).signal };
	const route = { id: routeId };
	const event: Pick<RequestEvent, 'url' | 'request' | 'locals' | 'route'> = {
		url,
		request,
		locals,
		route
	};
	return kitEventFetch(event, handleFetch);
}

/** Run `program` over the live port through `eventFetch`, its failure (or success) as the value. */
function runLive(
	program: Effect.Effect<unknown, unknown, ZurfurApi>,
	eventFetch: typeof fetch
): Promise<unknown> {
	return Effect.runPromise(
		program.pipe(
			Effect.merge,
			Effect.provide(ZurfurApiLive),
			Effect.provideService(RequestFetch, eventFetch)
		)
	);
}

const ACCOUNT_ID = '0192c3a4-0000-7000-8000-000000000001';
const COMMISSION_ID = '0192f0c0-0000-7000-8000-00000000c0de';

const startSignin = () =>
	runLive(
		Effect.flatMap(ZurfurApi, (api) => api.startSignin('alice.zurfur.app')),
		requestFetch('/login', '/(public)/login')
	);
const createAccount = () =>
	runLive(
		Effect.flatMap(ZurfurApi, (api) => api.createAccount('Studio', 'studio.zurfur.app')),
		requestFetch('/accounts', '/(session)/accounts')
	);
const deleteAccount = () =>
	runLive(
		Effect.flatMap(ZurfurApi, (api) => api.deleteAccount(ACCOUNT_ID)),
		requestFetch(`/accounts/${ACCOUNT_ID}`, '/(session)/accounts/[id]')
	);
const createCommission = () =>
	runLive(
		Effect.flatMap(ZurfurApi, (api) => api.createCommission('Fox in the snow')),
		requestFetch('/den/commissions', '/(session)/den/[...path]')
	);
const renameCommission = () => {
	const id = commissionId(COMMISSION_ID);
	if (id === undefined) throw new Error('bad test id');
	return runLive(
		Effect.flatMap(ZurfurApi, (api) => api.renameCommission(id, 'Renamed')),
		requestFetch(`/den/commissions/${COMMISSION_ID}`, '/(session)/den/[...path]')
	);
};

describe("SvelteKit's server-side writes reach the API carrying the page's origin", () => {
	it('the page origin differs from the API stand-in’s own', () => {
		expect(new URL(upstream.origin).origin).not.toBe(PAGE_ORIGIN);
	});

	it.each([
		['startSignin', 'POST', '/signin', startSignin],
		['createAccount', 'POST', '/accounts', createAccount],
		['createCommission', 'POST', '/commissions', createCommission],
		['renameCommission', 'PATCH', `/commissions/${COMMISSION_ID}/title`, renameCommission],
		['deleteAccount', 'DELETE', `/accounts/${ACCOUNT_ID}`, deleteAccount]
	] as const)('%s (%s) arrives with origin set to the page’s', async (_, method, path, send) => {
		await send();

		const expected: Arrival = { method, path, origin: [PAGE_ORIGIN] };
		expect(arrivals).toEqual([expected]);
	});
});
