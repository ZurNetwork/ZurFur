import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import type { FetchFunction } from '$lib/api/client';
import { NODE_NOT_FOUND_PROBLEM } from '$lib/api/problem';
import { fetchStub, problemResponse, requestUrl, unreachableFetch } from '$lib/testing/http';
import { pageToken } from '$lib/types/brand';
import { RequestFetch, ZurfurApi, ZurfurApiLive, parseBrowserBindingCookie } from './zurfur-api';
import type { DenQuery } from './den-read';
import { ApiProblem, type ZurfurApiError } from './errors';
import { segmentPath, type SegmentPath } from '../path-builder';

/** A problem-typed response carrying `body` as it is. */
function problemResponseWith(status: number, body: object): Response {
	const headers = { 'content-type': 'application/problem+json' };
	return new Response(JSON.stringify(body), { status, headers });
}

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
	it('returns the PDS authorize location and the browser-binding cookie from the 303', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const headers = new Headers({ location: authorizeUrl });
		headers.append('set-cookie', 'other=1; Path=/');
		headers.append(
			'set-cookie',
			'zurfur.signin=tok_en-1; HttpOnly; SameSite=Lax; Path=/; Max-Age=600'
		);
		const { fetch } = fetchStub(() => new Response(null, { status: 303, headers }));
		const started = await runLive(fetch, startSignin('alice.zurfur.app'));
		expect(started).toEqual({
			location: authorizeUrl,
			browserBinding: { name: 'zurfur.signin', value: 'tok_en-1', maxAge: 600, secure: false }
		});
	});

	it('fails ContractViolation when the redirect carries no browser-binding cookie', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const { fetch } = fetchStub(
			() => new Response(null, { status: 303, headers: { location: authorizeUrl } })
		);
		const failure = await runLive(fetch, failureOf(startSignin('alice.zurfur.app')));
		expect(failure._tag).toBe('ContractViolation');
		expect(failure.message).toMatch(/browser-binding cookie/);
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

const aliceRow = {
	id: 'acct-alice',
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	name: 'Alice Studio',
	role: 'owner'
};

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
		const created = {
			id: 'acct-new',
			did: 'did:plc:new',
			handle: 'new.zurfur.app',
			name: 'New Studio'
		};
		const { fetch, calls } = fetchStub(() => Response.json(created, { status: 201 }));
		const spyingFetch: typeof fetch = async (input, init) => {
			// `liveCreateAccount` always sends a JSON string body — narrow before
			// parsing rather than stringifying the whole `BodyInit` union (whose
			// other arms, e.g. `ReadableStream`, have no meaningful `toString`).
			sentBody = typeof init?.body === 'string' ? (JSON.parse(init.body) as unknown) : undefined;
			return fetch(input, init);
		};
		const account = await runLive(spyingFetch, createAccount('New Studio', 'new.zurfur.app'));
		expect(calls).toEqual(['/api/v1/accounts']);
		expect(sentBody).toEqual({ name: 'New Studio', handle: 'new.zurfur.app' });
		expect(account).toEqual(created);
	});

	it('fails ApiProblem carrying handle_taken on the 409', async () => {
		const { fetch } = fetchStub(() => problemResponse(409, 'handle_taken'));
		const failure = await runLive(
			fetch,
			failureOf(createAccount('New Studio', 'taken.zurfur.app'))
		);
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

describe('ZurfurApi.deleteAccount (live): the path builder guards the id', () => {
	it.each([
		['an empty id', ''],
		['a dot segment', '.'],
		['a dot-dot segment', '..'],
		['a slash that would reach another endpoint', '../me'],
		['a backslash', 'a\\b'],
		['a query', 'acct?x=1'],
		['a fragment', 'acct#x'],
		['a space', 'acct 1'],
		['non-ASCII', 'é']
	])('refuses %s without ever calling fetch, as account_not_found', async (_, id) => {
		const { fetch, calls } = fetchStub(() => Response.json({ outcome: 'hard' }));
		const failure = await runLive(fetch, failureOf(deleteAccount(id)));
		expect(calls).toEqual([]);
		expect(failure._tag).toBe('ApiProblem');
		if (failure._tag === 'ApiProblem') expect(failure.problem.code).toBe('account_not_found');
	});

	it('escapes a % so the backend’s one decode gives back the literal id, never a slash', async () => {
		const { fetch, calls } = fetchStub(() => Response.json({ outcome: 'hard' }));
		await runLive(fetch, deleteAccount('a%2Fb'));
		expect(calls).toEqual(['/api/v1/accounts/a%252Fb']);
	});

	it('sends the id %2e%2e escaped, so it can never climb', async () => {
		const { fetch, calls } = fetchStub(() => Response.json({ outcome: 'hard' }));
		await runLive(fetch, deleteAccount('%2e%2e'));
		expect(calls).toEqual(['/api/v1/accounts/%252e%252e']);
	});

	it('sends a DID id as one plain segment', async () => {
		const { fetch, calls } = fetchStub(() => Response.json({ outcome: 'hard' }));
		await runLive(fetch, deleteAccount('did:plc:mockalicestudioaaaaaaaa'));
		expect(calls).toEqual(['/api/v1/accounts/did:plc:mockalicestudioaaaaaaaa']);
	});

	it('sends a UUID id as one plain segment', async () => {
		const { fetch, calls } = fetchStub(() => Response.json({ outcome: 'hard' }));
		await runLive(fetch, deleteAccount('0192d00d-0b16-7000-8000-0000000b16b1'));
		expect(calls).toEqual(['/api/v1/accounts/0192d00d-0b16-7000-8000-0000000b16b1']);
	});
});

describe('ZurfurApi den reads (live)', () => {
	const plain = { includeDeleted: false, pageToken: undefined };
	const flagged = { includeDeleted: true, pageToken: pageToken('p1.50') };
	const privatePath = segmentPath(['accounts', 'did:plc:secretstudio']) ?? [];
	const denRoot = (query: DenQuery = plain) =>
		Effect.flatMap(ZurfurApi, (api) => api.denRoot(query));
	const denNode = (path: SegmentPath, query: DenQuery = plain) =>
		Effect.flatMap(ZurfurApi, (api) => api.denNode(path, query));
	const rootBody = {
		node: { name: 'Alice', type: 'user', kind: 'directory', open: { ownLevel: 'private' } },
		listing: { entries: [] }
	};

	it('reads the root at /den and a node at /den/{path}, with only the allowlisted query', async () => {
		const { fetch, calls } = fetchStub(() => Response.json(rootBody));
		await runLive(fetch, denRoot());
		await runLive(fetch, denRoot(flagged));
		await runLive(fetch, denNode(privatePath, flagged));
		expect(calls).toEqual([
			'/api/v1/den',
			'/api/v1/den?includeDeleted=true&pageToken=p1.50',
			'/api/v1/den/accounts/did:plc:secretstudio?includeDeleted=true&pageToken=p1.50'
		]);
	});

	it('answers a path the builder refuses with the one not-found, without calling the backend', async () => {
		const { fetch, calls } = fetchStub(() => Response.json(rootBody));
		// Forged past the mint: no real segment path reaches here refused.
		const refused = ['..'] as unknown as SegmentPath;
		const failure = await runLive(fetch, failureOf(denNode(refused)));
		expect(failure).toEqual(new ApiProblem({ problem: NODE_NOT_FOUND_PROBLEM }));
		expect(calls).toEqual([]);
	});

	it('fails NotAuthenticated on the 401 problem', async () => {
		const { fetch } = fetchStub(() => problemResponse(401, 'not_authenticated'));
		const failure = await runLive(fetch, failureOf(denNode(privatePath)));
		expect(failure._tag).toBe('NotAuthenticated');
	});

	it('fails ApiProblem on any other problem, carrying it whole', async () => {
		const { fetch } = fetchStub(() => problemResponse(404, 'node_not_found'));
		const failure = await runLive(fetch, failureOf(denNode(privatePath)));
		expect(failure._tag).toBe('ApiProblem');
		if (failure._tag === 'ApiProblem') expect(failure.problem.code).toBe('node_not_found');
	});

	it('fails NetworkFailure when the backend is unreachable', async () => {
		const failure = await runLive(unreachableFetch(), failureOf(denRoot()));
		expect(failure._tag).toBe('NetworkFailure');
	});

	it('keeps no part of a rejection that names the Den path, in the failure or its rejection', async () => {
		const leaky: FetchFunction = (input) =>
			Promise.reject(
				new TypeError(`fetch failed: ${requestUrl(input)}`, {
					cause: new Error(`connect ECONNREFUSED ${requestUrl(input)}`)
				})
			);
		const failure = await runLive(leaky, failureOf(denNode(privatePath)));
		const rejection = await runLive(leaky, denNode(privatePath)).then(
			() => 'resolved',
			(rejected: unknown) => `${String(rejected)} ${JSON.stringify(rejected)}`
		);

		expect(failure).toMatchObject({ _tag: 'NetworkFailure', reason: 'unreachable' });
		expect(`${String(failure)} ${JSON.stringify(failure)}`).not.toContain('secretstudio');
		expect(rejection).toContain('NetworkFailure');
		expect(rejection).not.toContain('secretstudio');
	});

	it.each([
		['TimeoutError', 'timeout'],
		['AbortError', 'aborted'],
		['SyntaxError', 'unreachable']
	])('reads a %s rejection as %s', async (name, reason) => {
		const rejecting: FetchFunction = () => Promise.reject(new DOMException('gone', name));
		const failure = await runLive(rejecting, failureOf(denRoot()));
		expect(failure).toMatchObject({ _tag: 'NetworkFailure', reason });
	});

	it.each([
		['a non-problem error', () => new Response('bad gateway', { status: 502 })],
		['a malformed problem', () => problemResponseWith(500, { code: 7 })],
		['a body that is not JSON', () => new Response('not json', { status: 200 })],
		['a payload that does not decode', () => Response.json({ node: { name: 7 } })],
		['an answer with no node', () => Response.json({ crumbs: [] })]
	])('fails ContractViolation on %s, naming only the route template', async (_, respond) => {
		const { fetch } = fetchStub(respond);
		const nodeFailure = await runLive(fetch, failureOf(denNode(privatePath, flagged)));
		const rootFailure = await runLive(fetch, failureOf(denRoot(flagged)));
		expect(nodeFailure._tag).toBe('ContractViolation');
		expect(rootFailure._tag).toBe('ContractViolation');
		if (nodeFailure._tag === 'ContractViolation') expect(nodeFailure.path).toBe('/den/{path}');
		if (rootFailure._tag === 'ContractViolation') expect(rootFailure.path).toBe('/den');
		for (const failure of [nodeFailure, rootFailure]) {
			const logged = `${failure.message} ${JSON.stringify(failure)}`;
			expect(logged).not.toContain('secretstudio');
			expect(logged).not.toContain('accounts');
			expect(logged).not.toContain('p1.50');
			expect(logged).not.toContain('includeDeleted');
		}
	});
});

describe('parseBrowserBindingCookie', () => {
	it('reads the host-locked cookie where cookies are Secure', () => {
		const setCookie =
			'__Host-zurfur.signin=abc; HttpOnly; SameSite=Lax; Secure; Path=/; Max-Age=600';
		expect(parseBrowserBindingCookie(setCookie)).toEqual({
			name: '__Host-zurfur.signin',
			value: 'abc',
			maxAge: 600,
			secure: true
		});
	});

	it('ignores every other cookie', () => {
		expect(parseBrowserBindingCookie('zurfur.sid=abc; Path=/; Max-Age=600')).toBeUndefined();
		expect(parseBrowserBindingCookie('zurfur.signinx=abc; Max-Age=600')).toBeUndefined();
	});

	it('refuses a cookie with no token or no positive Max-Age', () => {
		expect(parseBrowserBindingCookie('zurfur.signin=; Path=/; Max-Age=600')).toBeUndefined();
		expect(parseBrowserBindingCookie('zurfur.signin=abc; Path=/')).toBeUndefined();
		expect(parseBrowserBindingCookie('zurfur.signin=abc; Max-Age=0')).toBeUndefined();
		expect(parseBrowserBindingCookie('zurfur.signin=abc; Max-Age=ten')).toBeUndefined();
	});
});
