import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import type { FetchFunction } from '$lib/api/client';
import { fetchStub, problemResponse, unreachableFetch } from '$lib/testing/http';
import { RequestFetch, ZurfurApi, ZurfurApiLive } from './zurfur-api';
import type { ZurfurApiError } from './errors';

/** Run a program against the LIVE layer over a stubbed fetch. */
function runLive<A, E>(fetch: FetchFunction, program: Effect.Effect<A, E, ZurfurApi>): Promise<A> {
	const provided = program.pipe(
		Effect.provide(ZurfurApiLive),
		Effect.provideService(RequestFetch, fetch)
	);
	return Effect.runPromise(provided);
}

/** The program's failure, surfaced as the success value (no catch in specs). */
function failureOf<A>(
	program: Effect.Effect<A, ZurfurApiError, ZurfurApi>
): Effect.Effect<ZurfurApiError, A, ZurfurApi> {
	return Effect.flip(program);
}

const me = Effect.flatMap(ZurfurApi, (api) => api.me);
const startSignin = (handle: string) => Effect.flatMap(ZurfurApi, (api) => api.startSignin(handle));
const signout = Effect.flatMap(ZurfurApi, (api) => api.signout);
const listAccounts = Effect.flatMap(ZurfurApi, (api) => api.listAccounts);
const createAccount = (name: string, handle: string) =>
	Effect.flatMap(ZurfurApi, (api) => api.createAccount(name, handle));
const deleteAccount = (id: string) => Effect.flatMap(ZurfurApi, (api) => api.deleteAccount(id));

const aliceWire = {
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	displayName: 'Alice',
	avatarUrl: undefined
};

describe('ZurfurApi.me (live)', () => {
	it('rides the /api/v1 prefix so both split halves route it', async () => {
		const { fetch, calls } = fetchStub(() => Response.json(aliceWire));
		await runLive(fetch, me);
		expect(calls).toEqual(['/api/v1/me']);
	});

	it('decodes the session for a signed-in visitor', async () => {
		const { fetch } = fetchStub(() => Response.json(aliceWire));
		const session = await runLive(fetch, me);
		expect(session).toEqual(aliceWire);
	});

	it('fails NotAuthenticated on the 401 problem', async () => {
		const { fetch } = fetchStub(() => problemResponse(401, 'not_authenticated'));
		const failure = await runLive(fetch, failureOf(me));
		expect(failure._tag).toBe('NotAuthenticated');
	});

	it('fails ApiProblem on any other problem, carrying it whole', async () => {
		const { fetch } = fetchStub(() => problemResponse(429, 'rate_limited'));
		const failure = await runLive(fetch, failureOf(me));
		expect(failure._tag).toBe('ApiProblem');
		if (failure._tag === 'ApiProblem') expect(failure.problem.code).toBe('rate_limited');
	});

	it('fails ContractViolation on a non-problem error response', async () => {
		const { fetch } = fetchStub(() => new Response('gateway timeout', { status: 504 }));
		const failure = await runLive(fetch, failureOf(me));
		expect(failure._tag).toBe('ContractViolation');
		expect(failure.message).toMatch(/\/me responded 504/);
	});

	it('fails ContractViolation when a success body is not JSON', async () => {
		const { fetch } = fetchStub(() => new Response('not json', { status: 200 }));
		const failure = await runLive(fetch, failureOf(me));
		expect(failure.message).toMatch(/\/me responded 200 — unparsable body/);
	});

	it('fails ContractViolation when the payload does not fit the session schema', async () => {
		const { fetch } = fetchStub(() => Response.json({ did: 42 }));
		const failure = await runLive(fetch, failureOf(me));
		expect(failure.message).toMatch(/malformed session payload/);
	});

	it('fails NetworkFailure when the backend is unreachable', async () => {
		const failure = await runLive(unreachableFetch(), failureOf(me));
		expect(failure._tag).toBe('NetworkFailure');
	});
});

describe('ZurfurApi.startSignin (live)', () => {
	it('returns the PDS authorize location from the 303', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const { fetch } = fetchStub(
			() => new Response(null, { status: 303, headers: { location: authorizeUrl } })
		);
		const location = await runLive(fetch, startSignin('alice.zurfur.app'));
		expect(location).toBe(authorizeUrl);
	});

	it('fails ApiProblem when the backend rejects the handle', async () => {
		const { fetch } = fetchStub(() => problemResponse(422, 'invalid_request'));
		const failure = await runLive(fetch, failureOf(startSignin('not a handle')));
		expect(failure._tag).toBe('ApiProblem');
		if (failure._tag === 'ApiProblem') expect(failure.problem.code).toBe('invalid_request');
	});

	it('fails ContractViolation when a redirect arrives without a Location header', async () => {
		const { fetch } = fetchStub(() => new Response(null, { status: 303 }));
		const failure = await runLive(fetch, failureOf(startSignin('alice.zurfur.app')));
		expect(failure.message).toMatch(/no Location/);
	});

	it('fails ContractViolation on a problem-shaped body missing the problem content type', async () => {
		const mislabelled = () =>
			new Response(
				JSON.stringify({
					type: 'urn:zurfur:error:invalid-request',
					code: 'invalid_request',
					title: 'invalid_request',
					status: 422
				}),
				{ status: 422, headers: { 'content-type': 'application/json' } }
			);
		const { fetch } = fetchStub(mislabelled);
		const failure = await runLive(fetch, failureOf(startSignin('alice.zurfur.app')));
		expect(failure._tag).toBe('ContractViolation');
	});
});

describe('ZurfurApi.signout (live)', () => {
	it('returns the cookie names the backend cleared on the 303', async () => {
		const headers = new Headers({ location: '/' });
		headers.append('set-cookie', 'zurfur.sid=; Max-Age=0; Path=/');
		headers.append('set-cookie', 'zurfur.csrf=; Max-Age=0; Path=/');
		const { fetch } = fetchStub(() => new Response(null, { status: 303, headers }));
		const cleared = await runLive(fetch, signout);
		expect(cleared).toEqual(['zurfur.sid', 'zurfur.csrf']);
	});

	it('fails SignoutFailed when the backend does not answer with a redirect', async () => {
		const { fetch } = fetchStub(() => new Response(null, { status: 200 }));
		const failure = await runLive(fetch, failureOf(signout));
		expect(failure._tag).toBe('SignoutFailed');
		if (failure._tag === 'SignoutFailed') expect(failure.status).toBe(200);
	});
});

const aliceRow = { id: 'acct-alice', did: 'did:plc:alice', handle: 'alice.zurfur.app', name: 'Alice Studio', role: 'owner' };

describe('ZurfurApi.listAccounts (live)', () => {
	it('decodes a wrapped listing into plain rows', async () => {
		const { fetch, calls } = fetchStub(() => Response.json({ accounts: [aliceRow] }));
		const accounts = await runLive(fetch, listAccounts);
		expect(calls).toEqual(['/api/v1/accounts']);
		expect(accounts).toEqual([aliceRow]);
	});

	it('tolerates an unknown role value and an unknown extra field (R8)', async () => {
		const futureRow = { ...aliceRow, role: 'steward', futureField: 'not in the contract yet' };
		const { fetch } = fetchStub(() => Response.json({ accounts: [futureRow] }));
		const accounts = await runLive(fetch, listAccounts);
		expect(accounts).toEqual([{ ...aliceRow, role: 'steward' }]);
	});

	it('fails ContractViolation naming /accounts on a non-JSON body', async () => {
		const { fetch } = fetchStub(() => new Response('not json', { status: 200 }));
		const failure = await runLive(fetch, failureOf(listAccounts));
		expect(failure._tag).toBe('ContractViolation');
		expect(failure.message).toMatch(/\/accounts responded 200 — unparsable body/);
	});
});

describe('ZurfurApi.createAccount (live)', () => {
	it('posts {name, handle} and decodes the 201 into the founded account', async () => {
		let sentBody: unknown;
		const created = { id: 'acct-new', did: 'did:plc:new', handle: 'new.zurfur.app', name: 'New Studio' };
		const { fetch, calls } = fetchStub(() => Response.json(created, { status: 201 }));
		const spyingFetch: typeof fetch = async (input, init) => {
			sentBody = init?.body === undefined ? undefined : JSON.parse(String(init.body));
			return fetch(input, init);
		};
		const account = await runLive(spyingFetch, createAccount('New Studio', 'new.zurfur.app'));
		expect(calls).toEqual(['/api/v1/accounts']);
		expect(sentBody).toEqual({ name: 'New Studio', handle: 'new.zurfur.app' });
		expect(account).toEqual(created);
	});

	it('fails ApiProblem carrying handle_taken on the 409', async () => {
		const { fetch } = fetchStub(() => problemResponse(409, 'handle_taken'));
		const failure = await runLive(fetch, failureOf(createAccount('New Studio', 'taken.zurfur.app')));
		expect(failure._tag).toBe('ApiProblem');
		if (failure._tag === 'ApiProblem') expect(failure.problem.code).toBe('handle_taken');
	});

	it('fails ApiProblem carrying invalid_request on the 422 (reserved label / punycode share this code)', async () => {
		const { fetch } = fetchStub(() => problemResponse(422, 'invalid_request'));
		const failure = await runLive(fetch, failureOf(createAccount('', 'xn--bad')));
		expect(failure._tag).toBe('ApiProblem');
		if (failure._tag === 'ApiProblem') expect(failure.problem.code).toBe('invalid_request');
	});
});

describe('ZurfurApi.deleteAccount (live)', () => {
	it('decodes a hard outcome', async () => {
		const { fetch, calls } = fetchStub(() => Response.json({ outcome: 'hard' }));
		const outcome = await runLive(fetch, deleteAccount('acct-1'));
		expect(calls).toEqual(['/api/v1/accounts/acct-1']);
		expect(outcome).toBe('hard');
	});

	it('decodes a soft outcome', async () => {
		const { fetch } = fetchStub(() => Response.json({ outcome: 'soft' }));
		const outcome = await runLive(fetch, deleteAccount('acct-1'));
		expect(outcome).toBe('soft');
	});

	it('fails ApiProblem carrying forbidden on the 403 (non-Owner)', async () => {
		const { fetch } = fetchStub(() => problemResponse(403, 'forbidden'));
		const failure = await runLive(fetch, failureOf(deleteAccount('acct-1')));
		expect(failure._tag).toBe('ApiProblem');
		if (failure._tag === 'ApiProblem') expect(failure.problem.code).toBe('forbidden');
	});

	it('fails ApiProblem carrying account_not_found (not not_found) on the 404', async () => {
		const { fetch } = fetchStub(() => problemResponse(404, 'account_not_found'));
		const failure = await runLive(fetch, failureOf(deleteAccount('acct-1')));
		expect(failure._tag).toBe('ApiProblem');
		if (failure._tag === 'ApiProblem') expect(failure.problem.code).toBe('account_not_found');
	});

	it("resolves an unknown outcome string to the 'unknown' fallback, never throwing (R8)", async () => {
		const { fetch } = fetchStub(() => Response.json({ outcome: 'quarantined' }));
		const outcome = await runLive(fetch, deleteAccount('acct-1'));
		expect(outcome).toBe('unknown');
	});
});
