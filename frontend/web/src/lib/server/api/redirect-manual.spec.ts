/**
 * Pins the `redirect: 'manual'` contract `liveStartSignin`/`liveSignout`
 * depend on (GitHub #154, follow-up from the ZMVP-151 ship-gates review):
 * undici (Node's `fetch`) answers a `redirect: 'manual'` request with the
 * REAL 303 status and readable `Location` / `Set-Cookie` headers — a
 * browser `fetch` would instead see an opaque status-0 redirect with empty
 * headers, silently breaking signin (ContractViolation) and logout
 * (SignoutFailed). Every other spec in this file stubs `fetch` with
 * hand-built `Response`s, which proves the CODE reads the headers it
 * expects but never proves the RUNTIME hands them over on a real redirect.
 * This spec drives the live calls against one real local HTTP server
 * instead, over the real global `fetch`, so the assumption is tested
 * rather than merely documented.
 */

import { createServer, type Server } from 'node:http';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import type { FetchFunction } from '$lib/api/client';
import { RequestFetch, ZurfurApi, ZurfurApiLive } from './zurfur-api';

/** Run a program against the LIVE layer over a real fetch. */
function runLive<A, E>(fetch: FetchFunction, program: Effect.Effect<A, E, ZurfurApi>): Promise<A> {
	const provided = program.pipe(
		Effect.provide(ZurfurApiLive),
		Effect.provideService(RequestFetch, fetch)
	);
	return Effect.runPromise(provided);
}

const startSignin = (handle: string) => Effect.flatMap(ZurfurApi, (api) => api.startSignin(handle));
const signout = Effect.flatMap(ZurfurApi, (api) => api.signout);

/** Start a real local HTTP server answering every request with a 303. */
function listen(server: Server): Promise<string> {
	return new Promise((resolve) => {
		server.listen(0, '127.0.0.1', () => {
			const address = server.address();
			if (address === null || typeof address === 'string') {
				throw new Error('expected a TCP AddressInfo');
			}
			resolve(`http://127.0.0.1:${address.port}`);
		});
	});
}

function close(server: Server): Promise<void> {
	return new Promise((resolve, reject) => {
		server.close((error) => (error === undefined ? resolve() : reject(error)));
	});
}

describe('redirect: "manual" — real 303 + Location + Set-Cookie survive undici (#154)', () => {
	const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
	let server: Server;
	let origin: string;

	beforeAll(async () => {
		server = createServer((request, response) => {
			if (request.url?.endsWith('/signin')) {
				response.writeHead(303, { location: authorizeUrl }).end();
				return;
			}
			if (request.url?.endsWith('/logout')) {
				response
					.writeHead(303, {
						location: '/',
						'set-cookie': ['zurfur.sid=; Max-Age=0; Path=/', 'zurfur.csrf=; Max-Age=0; Path=/']
					})
					.end();
				return;
			}
			response.writeHead(404).end();
		});
		origin = await listen(server);
	});

	afterAll(() => close(server));

	// The real `fetch` (undici), talking to the real server above — API_PREFIX
	// still gets prepended by backendFetch, so the server matches on suffix.
	const realFetch: FetchFunction = ((input: RequestInfo | URL, init?: RequestInit) =>
		fetch(`${origin}${String(input).replace(/^\/api\/v1/, '')}`, init)) as FetchFunction;

	it('liveStartSignin reads the real 303 status and Location header', async () => {
		const location = await runLive(realFetch, startSignin('alice.zurfur.app'));
		expect(location).toBe(authorizeUrl);
	});

	it('liveSignout reads the real 303 status and Set-Cookie headers', async () => {
		const cleared = await runLive(realFetch, signout);
		expect(cleared).toEqual(['zurfur.sid', 'zurfur.csrf']);
	});
});
