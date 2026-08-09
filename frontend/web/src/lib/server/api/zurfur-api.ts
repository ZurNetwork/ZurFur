/**
 * The `ZurfurApi` port (DD 39944194): every backend call the SvelteKit server
 * makes, as one service named by role with a prod Layer (real HTTP through the
 * `/api` split) and an in-memory Layer for tests — `adapter-mem` parity for
 * the frontend. Success payloads decode through Effect Schema at the boundary;
 * failures are the tagged union in {@link import('./errors')}.
 */

import {
	create,
	type DescMessage,
	fromJson,
	type JsonValue,
	type MessageShape,
	toJson
} from '@bufbuild/protobuf';
import { Context, Effect, Layer } from 'effect';
import type { AccountMembership, CreatedAccount, DeleteOutcome } from '$lib/api/account';
import { API_PREFIX, type FetchFunction } from '$lib/api/client';
import { PROBLEM_CONTENT_TYPE, type Problem, ProblemKind } from '$lib/api/problem';
import { HttpStatus } from '$lib/api/http-status';
import type { Session } from '$lib/api/session';
import { accountId, did, handleFromTrusted } from '$lib/types/brand';
import {
	CreateAccountRequestSchema,
	CreateAccountResponseSchema,
	DeleteAccountResponseSchema,
	ListAccountsResponseSchema
} from './generated/zurfur/api/v1/account_pb';
import { ProblemSchema } from './generated/zurfur/api/v1/problem_pb';
import { GetMeResponseSchema } from './generated/zurfur/api/v1/session_pb';
import {
	ApiProblem,
	ContractViolation,
	NetworkFailure,
	NotAuthenticated,
	SignoutFailed
} from './errors';

/**
 * The contract's boundary decoder (ZMVP-161; amended Decision 4 of
 * DD 39944194): `fromJson` against a GENERATED schema — produced from
 * `contract/zurfur/api/v1/*.proto`, so it structurally cannot drift from the
 * contract, which is a stronger property than the hand-written Effect Schema
 * it replaces offered. `ignoreUnknownFields: true` is MANDATORY here
 * (contract §6): protobuf-es rejects unknown fields by default, and
 * additive-only evolution is void without the tolerant reader. The
 * `contract_tolerant_reader` spec pins this exact function, so dropping the
 * option is a failing test, not a shipped incident.
 */
export function decodeContract<Desc extends DescMessage>(
	schema: Desc,
	value: JsonValue
): MessageShape<Desc> {
	return fromJson(schema, value, { ignoreUnknownFields: true });
}

/** What each `ZurfurApi` call does; failures per call are in the signature. */
export interface ZurfurApiShape {
	/** `GET /me` — who is signed in. Fails `NotAuthenticated` for anonymous/expired. */
	readonly me: Effect.Effect<
		Session,
		NotAuthenticated | ApiProblem | NetworkFailure | ContractViolation
	>;
	/**
	 * `POST /signin` — start the atproto OAuth flow; succeeds with the PDS
	 * authorize URL off the 303's `Location` (server-side `redirect: 'manual'`
	 * semantics — a browser fetch would return an opaque redirect).
	 */
	readonly startSignin: (
		handle: string
	) => Effect.Effect<string, ApiProblem | NetworkFailure | ContractViolation>;
	/**
	 * `POST /logout` — end the session backend-side; succeeds with the cookie
	 * names the backend cleared (for mirroring onto the browser's response —
	 * the SSR proxy rewrites the host, so SvelteKit won't pass `set-cookie`
	 * through on its own).
	 */
	readonly signout: Effect.Effect<readonly string[], SignoutFailed | NetworkFailure>;
	/** `GET /accounts` — every live account the caller holds a role in, role riding per row (R7 wrapped on the wire, unwrapped here). */
	readonly listAccounts: Effect.Effect<
		readonly AccountMembership[],
		ApiProblem | NetworkFailure | ContractViolation
	>;
	/** `POST /accounts` — found an account; the caller becomes its Owner. Rejects with `handle_taken` (409) or `invalid_request` (422). */
	readonly createAccount: (
		name: string,
		handle: string
	) => Effect.Effect<CreatedAccount, ApiProblem | NetworkFailure | ContractViolation>;
	/**
	 * `DELETE /accounts/{id}` — Owner-only. Succeeds with which deletion
	 * happened (⚠️ F3: an outcome value this build doesn't recognize resolves
	 * to `'unknown'`, never a decode failure). Rejects with `forbidden` (403,
	 * non-Owner) or `account_not_found` (404).
	 */
	readonly deleteAccount: (
		id: string
	) => Effect.Effect<DeleteOutcome, ApiProblem | NetworkFailure | ContractViolation>;
}

/** The port tag — programs ask for `ZurfurApi`, the seam decides which Layer answers. */
export class ZurfurApi extends Context.Tag('web/ZurfurApi')<ZurfurApi, ZurfurApiShape>() {}

/**
 * The per-request `fetch` the live Layer speaks through — the SvelteKit event
 * `fetch` (SSR rewrite + cookie forwarding) or the browser's. Provided at the
 * seam per request; never baked into the runtime.
 */
export class RequestFetch extends Context.Tag('web/RequestFetch')<RequestFetch, FetchFunction>() {}

/** A fetch that reaches the backend or fails `NetworkFailure` — never throws through. */
function backendFetch(
	fetch: FetchFunction,
	path: string,
	init?: RequestInit
): Effect.Effect<Response, NetworkFailure> {
	return Effect.tryPromise({
		try: () => fetch(`${API_PREFIX}${path}`, init),
		catch: (cause) => new NetworkFailure({ cause })
	});
}

/**
 * `JSON.parse`, typed by what it actually produces: every JSON text parses to a
 * `JsonValue` — a true fact the lib's `any` signature is too weak to state, so
 * it is recorded here once by annotation (no assertion involved).
 */
const parseJsonValue: (text: string) => JsonValue = JSON.parse;

/** Parse the body as JSON or fail `ContractViolation` naming the endpoint and status. */
function parsedBody(response: Response, path: string): Effect.Effect<JsonValue, ContractViolation> {
	return Effect.tryPromise({
		try: async () => parseJsonValue(await response.text()),
		catch: () => new ContractViolation({ path, status: response.status, detail: 'unparsable body' })
	});
}

/**
 * Classify a non-2xx response by the error contract: `application/problem+json`
 * with a well-formed problem becomes `NotAuthenticated` (the session branch) or
 * `ApiProblem`; anything else is a `ContractViolation`.
 */
function problemFailure(
	response: Response,
	path: string
): Effect.Effect<never, NotAuthenticated | ApiProblem | ContractViolation> {
	const violation = new ContractViolation({
		path,
		status: response.status,
		detail: 'no problem body'
	});
	const contentType = response.headers.get('content-type') ?? '';
	if (!contentType.startsWith(PROBLEM_CONTENT_TYPE)) return Effect.fail(violation);

	const classified = (
		body: JsonValue
	): Effect.Effect<never, NotAuthenticated | ApiProblem | ContractViolation> => {
		// Decode through the generated schema (ZMVP-162): the contract's one
		// Problem declaration, mapped to the plain component-facing interface.
		let problem: Problem;
		try {
			const message = decodeContract(ProblemSchema, body);
			problem = {
				type: message.type,
				code: message.code,
				title: message.title,
				detail: message.detail,
				status: message.status
			};
		} catch {
			// Each failure arm keeps its own detail — "no problem body" (wrong
			// content type), "unparsable body" (parsedBody's, propagated), and
			// this one — so a ContractViolation says which contract promise broke.
			return Effect.fail(
				new ContractViolation({ path, status: response.status, detail: 'malformed problem body' })
			);
		}
		if (problem.code === ProblemKind.NotAuthenticated.code)
			return Effect.fail(new NotAuthenticated({ problem }));
		return Effect.fail(new ApiProblem({ problem }));
	};
	return parsedBody(response, path).pipe(Effect.flatMap(classified));
}

/** The redirect-range check both signin and signout branch on. */
function isRedirectStatus(status: number): boolean {
	return status >= 300 && status < 400;
}

const liveMe = (fetch: FetchFunction) =>
	Effect.gen(function* () {
		const response = yield* backendFetch(fetch, '/me');
		if (!response.ok) return yield* problemFailure(response, '/me');
		const raw = yield* parsedBody(response, '/me');
		// Decode through the generated schema, then map the message to the PLAIN
		// component-facing Session — generated types stay below the seam (the
		// containment guard applies to them unchanged), components get plain data.
		return yield* Effect.try({
			try: () => {
				const message = decodeContract(GetMeResponseSchema, raw);
				const session: Session = {
					did: did(message.did),
					handle: message.handle === undefined ? undefined : handleFromTrusted(message.handle),
					displayName: message.displayName,
					avatarUrl: message.avatarUrl
				};
				return session;
			},
			catch: () =>
				new ContractViolation({
					path: '/me',
					status: response.status,
					detail: 'malformed session payload'
				})
		});
	});

const liveStartSignin = (fetch: FetchFunction, handle: string) =>
	Effect.gen(function* () {
		const form = new URLSearchParams({ handle });
		// redirect:'manual' exposing the real 303 + Location is server-only (undici)
		// behavior — a browser fetch would see an opaque status-0 redirect instead.
		const init: RequestInit = { method: 'POST', body: form, redirect: 'manual' };
		const response = yield* backendFetch(fetch, '/signin', init);
		if (isRedirectStatus(response.status)) {
			const location = response.headers.get('location') ?? undefined;
			if (location === undefined) {
				return yield* new ContractViolation({
					path: '/signin',
					status: response.status,
					detail: 'redirect carried no Location header'
				});
			}
			return location;
		}
		return yield* problemFailure(response, '/signin').pipe(
			Effect.catchTag('NotAuthenticated', ({ problem }) => new ApiProblem({ problem }))
		);
	});

const liveSignout = (fetch: FetchFunction) =>
	Effect.gen(function* () {
		// Server-only undici semantics again: the 303 + Set-Cookie stay readable.
		const init: RequestInit = { method: 'POST', redirect: 'manual' };
		const response = yield* backendFetch(fetch, '/logout', init);
		if (!isRedirectStatus(response.status)) {
			return yield* new SignoutFailed({ status: response.status });
		}
		const clearedNames = response.headers
			.getSetCookie()
			.map((setCookie) => setCookie.split('=')[0]?.trim())
			.filter((name): name is string => name !== undefined && name !== '');
		return clearedNames;
	});

/**
 * Any non-2xx account response, classified the shared way — 401
 * `not_authenticated` folds into `ApiProblem` too (unlike `me`, none of the
 * account calls has a distinct "am I signed in" branch for callers to key
 * on; the session gate above these routes already owns that question).
 */
function accountProblemFailure(
	response: Response,
	path: string
): Effect.Effect<never, ApiProblem | ContractViolation> {
	return problemFailure(response, path).pipe(
		Effect.catchTag('NotAuthenticated', ({ problem }) => new ApiProblem({ problem }))
	);
}

const liveListAccounts = (fetch: FetchFunction) =>
	Effect.gen(function* () {
		const response = yield* backendFetch(fetch, '/accounts');
		if (!response.ok) return yield* accountProblemFailure(response, '/accounts');
		const raw = yield* parsedBody(response, '/accounts');
		return yield* Effect.try({
			try: () => {
				const message = decodeContract(ListAccountsResponseSchema, raw);
				const accounts: AccountMembership[] = message.accounts.map((row) => ({
					id: accountId(row.id),
					did: did(row.did),
					handle: handleFromTrusted(row.handle),
					name: row.name,
					role: row.role
				}));
				return accounts;
			},
			catch: () =>
				new ContractViolation({
					path: '/accounts',
					status: response.status,
					detail: 'malformed account listing payload'
				})
		});
	});

const liveCreateAccount = (fetch: FetchFunction, name: string, handle: string) =>
	Effect.gen(function* () {
		// Encode through the generated schema, mirroring the decode side
		// (Engineer ruling 2026-07-28, extending DD 39944194 D4 to the request
		// direction): field NAMES cannot drift from the contract. Note toJson
		// omits implicit-presence zero values ('' fields drop off the wire) —
		// the backend decodes absent and empty identically, so the meaning is
		// unchanged.
		const request = create(CreateAccountRequestSchema, { name, handle });
		const init: RequestInit = {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(toJson(CreateAccountRequestSchema, request))
		};
		const response = yield* backendFetch(fetch, '/accounts', init);
		if (!response.ok) return yield* accountProblemFailure(response, '/accounts');
		const raw = yield* parsedBody(response, '/accounts');
		return yield* Effect.try({
			try: () => {
				const message = decodeContract(CreateAccountResponseSchema, raw);
				const created: CreatedAccount = {
					id: accountId(message.id),
					did: did(message.did),
					handle: handleFromTrusted(message.handle),
					name: message.name
				};
				return created;
			},
			catch: () =>
				new ContractViolation({
					path: '/accounts',
					status: response.status,
					detail: 'malformed created account payload'
				})
		});
	});

/** R8's defined fallback for `DeleteAccountResponse.outcome` (⚠️ F3): a value this client build doesn't recognize yet becomes `'unknown'` — never a thrown/decoded failure, and never read as confirming either soft or hard deletion. */
function toDeleteOutcome(outcome: string): DeleteOutcome {
	if (outcome === 'soft' || outcome === 'hard') return outcome;
	return 'unknown';
}

const liveDeleteAccount = (fetch: FetchFunction, id: string) =>
	Effect.gen(function* () {
		const path = `/accounts/${encodeURIComponent(id)}`;
		const response = yield* backendFetch(fetch, path, { method: 'DELETE' });
		if (!response.ok) return yield* accountProblemFailure(response, path);
		const raw = yield* parsedBody(response, path);
		return yield* Effect.try({
			try: () => {
				const message = decodeContract(DeleteAccountResponseSchema, raw);
				return toDeleteOutcome(message.outcome);
			},
			catch: () =>
				new ContractViolation({
					path,
					status: response.status,
					detail: 'malformed delete outcome payload'
				})
		});
	});

/** The prod Layer: real HTTP through the per-request {@link RequestFetch}. */
export const ZurfurApiLive: Layer.Layer<ZurfurApi, never, RequestFetch> = Layer.effect(
	ZurfurApi,
	Effect.gen(function* () {
		const fetch = yield* RequestFetch;
		return ZurfurApi.of({
			me: liveMe(fetch),
			startSignin: (handle) => liveStartSignin(fetch, handle),
			signout: liveSignout(fetch),
			listAccounts: liveListAccounts(fetch),
			createAccount: (name, handle) => liveCreateAccount(fetch, name, handle),
			deleteAccount: (id) => liveDeleteAccount(fetch, id)
		});
	})
);

/** The problem an unstubbed `me` fails with — the backend's anonymous 401 shape. */
const anonymousProblem: Problem = {
	...ProblemKind.NotAuthenticated,
	title: 'not_authenticated',
	detail: 'No signed-in visitor (test default).',
	status: HttpStatus.Unauthorized
};

/** Anonymous-by-default stub behaviors for {@link zurfurApiTest}. */
const anonymousDefaults: ZurfurApiShape = {
	me: Effect.fail(new NotAuthenticated({ problem: anonymousProblem })),
	startSignin: () => Effect.fail(new NetworkFailure({ cause: new TypeError('no signin stubbed') })),
	signout: Effect.fail(new SignoutFailed({ status: HttpStatus.InternalServerError })),
	listAccounts: Effect.fail(new ApiProblem({ problem: anonymousProblem })),
	createAccount: () => Effect.fail(new ApiProblem({ problem: anonymousProblem })),
	deleteAccount: () => Effect.fail(new ApiProblem({ problem: anonymousProblem }))
};

/**
 * The in-memory Layer (adapter-mem parity): tests hand in only the behaviors
 * they exercise; everything else answers like an anonymous, signin-less world.
 */
export function zurfurApiTest(overrides: Partial<ZurfurApiShape>): Layer.Layer<ZurfurApi> {
	const shape: ZurfurApiShape = { ...anonymousDefaults, ...overrides };
	return Layer.succeed(ZurfurApi, shape);
}
