/**
 * The `ZurfurApi` port: every backend call the SvelteKit server
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
import {
	ACCOUNT_NOT_FOUND_PROBLEM,
	NODE_NOT_FOUND_PROBLEM,
	PROBLEM_CONTENT_TYPE,
	type Problem,
	ProblemKind
} from '$lib/api/problem';
import { HttpStatus } from '$lib/api/http-status';
import type { Session } from '$lib/api/session';
import {
	accountId,
	bugReference,
	did,
	handleFromTrusted,
	type BugReference
} from '$lib/types/brand';
import type {
	BugBrowser,
	BugDescription,
	BugPage,
	BugVersion,
	ErrorStatus,
	PageType,
	WindowSide
} from '$lib/types/bug-report-rules';
import {
	CreateAccountRequestSchema,
	CreateAccountResponseSchema,
	DeleteAccountResponseSchema,
	ListAccountsResponseSchema
} from './generated/zurfur/api/v1/account_pb';
import {
	FileBugReportRequest_DisplaySchema,
	FileBugReportRequest_WindowSchema,
	FileBugReportRequestSchema,
	FileBugReportResponseSchema
} from './generated/zurfur/api/v1/bug_report_pb';
import { ProblemSchema } from './generated/zurfur/api/v1/problem_pb';
import { GetMeResponseSchema } from './generated/zurfur/api/v1/session_pb';
import {
	GetDenNodeResponseSchema,
	GetDenRootResponseSchema
} from './generated/zurfur/api/v1/den_pb';
import {
	ApiProblem,
	ContractViolation,
	NetworkFailure,
	NotAuthenticated,
	SignoutFailed
} from './errors';
import type { DenQuery, DenRead } from './den-read';
import { DEN_ROUTES, denReadFromNode, denReadFromRoot, type DenRoute } from './den-wire';
import { joinPath, segmentPath, type SegmentPath } from '../path-builder';
import { denApiPathOf } from '../den/den-path';
import { isSignInBindingCookieName, type SignInBindingCookieName } from '../cookies';
import { countdown, TIME_LIMITS } from '../time-limits';
import { firstAbortOf, readWhole, settleBy } from '../bounded-fetch';

/**
 * The contract's boundary decoder: `fromJson` against a GENERATED schema — produced from
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
	 * semantics — a browser fetch would return an opaque redirect) and the
	 * cookie binding the sign-in to this browser, for the action to relay.
	 */
	readonly startSignin: (
		handle: string
	) => Effect.Effect<StartedSignin, ApiProblem | NetworkFailure | ContractViolation>;
	/**
	 * `POST /logout` — end the session backend-side, waiting at most sign-out's
	 * own time limit (a timeout fails `NetworkFailure`); succeeds with the cookie
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
	/**
	 * `GET /den` — the viewer's own root (My Den) and one page of its entries.
	 * Fails `ApiProblem` `node_not_found` for every miss, `NotAuthenticated`
	 * for an ended session; a failure names only the route template.
	 */
	readonly denRoot: (query: DenQuery) => Effect.Effect<DenRead, DenReadError>;
	/** `GET /den/{path}` — the node at a Den path below the root; failures as {@link denRoot}. */
	readonly denNode: (path: SegmentPath, query: DenQuery) => Effect.Effect<DenRead, DenReadError>;
	/**
	 * `POST /bug-reports` — file a bug report as the signed-in visitor; succeeds
	 * with the reference to quote. Fails `NotAuthenticated` for an ended
	 * session, `ApiProblem` for any other refusal (the hourly limit among them).
	 */
	readonly fileBugReport: (
		filing: BugReportFiling
	) => Effect.Effect<
		BugReference,
		NotAuthenticated | ApiProblem | NetworkFailure | ContractViolation
	>;
}

/** The browser window's size, each side 1 to 100000. */
export interface BugWindow {
	readonly width: WindowSide;
	readonly height: WindowSide;
}

/** Which display preferences the browser matched. */
export interface BugDisplay {
	readonly reducedMotion: boolean;
	readonly moreContrast: boolean;
	readonly forcedColors: boolean;
}

/**
 * A bug report as the port files it: the description, and each item only
 * when the reporter chose to send it. The reporter is the session's, never
 * named here.
 */
export interface BugReportFiling {
	readonly description: BugDescription;
	readonly page: BugPage | undefined;
	readonly pageType: PageType | undefined;
	readonly webVersion: BugVersion | undefined;
	readonly browser: BugBrowser | undefined;
	readonly window: BugWindow | undefined;
	readonly display: BugDisplay | undefined;
	/** The error page's HTTP status, 400 to 599. */
	readonly errorStatus: ErrorStatus | undefined;
}

/**
 * The cookie binding a sign-in to the browser that started it, as the backend
 * set it on `/signin`'s 303. Only what varies rides here (the name, value,
 * lifetime and `Secure`); `cookieOptions` fixes the rest.
 */
export interface BrowserBindingCookie {
	readonly name: SignInBindingCookieName;
	readonly value: string;
	readonly maxAge: number;
	readonly secure: boolean;
}

/** A started sign-in: where to send the browser, and the cookie it must carry back. */
export interface StartedSignin {
	readonly location: string;
	readonly browserBinding: BrowserBindingCookie;
}

/** Every way a Den read fails. */
export type DenReadError = NotAuthenticated | ApiProblem | NetworkFailure | ContractViolation;

/** The port tag — programs ask for `ZurfurApi`, the seam decides which Layer answers. */
export class ZurfurApi extends Context.Tag('web/ZurfurApi')<ZurfurApi, ZurfurApiShape>() {}

/**
 * The per-request `fetch` the live Layer speaks through — the SvelteKit event
 * `fetch` (SSR rewrite + cookie forwarding) or the browser's. Provided at the
 * seam per request; never baked into the runtime.
 */
export class RequestFetch extends Context.Tag('web/RequestFetch')<RequestFetch, FetchFunction>() {}

/**
 * Why a fetch rejected, read from the rejection's name alone: a timeout, an
 * abort, or anything else as unreachable. The rejection is never kept.
 */
function networkFailureOf(rejection: unknown): NetworkFailure {
	if (!(rejection instanceof Error)) return new NetworkFailure({ reason: 'unreachable' });
	switch (rejection.name) {
		case 'TimeoutError':
			return new NetworkFailure({ reason: 'timeout' });
		case 'AbortError':
			return new NetworkFailure({ reason: 'aborted' });
		default:
			return new NetworkFailure({ reason: 'unreachable' });
	}
}

/** What a backend call may set on its fetch: everything but the signal, which its time limit owns. */
type BackendInit = Omit<RequestInit, 'signal'>;

/**
 * A call to the backend, answered whole (the body read too) within `limit`
 * ms, or failed `NetworkFailure`: a call that runs out is a `timeout` on time
 * even if the abort never reaches the fetch, and an interrupted one stops its
 * fetch. Never throws through.
 */
function backendFetch(
	fetch: FetchFunction,
	path: string,
	init: BackendInit = {},
	limit: number = TIME_LIMITS.call
): Effect.Effect<Response, NetworkFailure> {
	return Effect.acquireUseRelease(
		Effect.sync(() => countdown(limit)),
		(clock) =>
			Effect.tryPromise({
				try: (interrupted) => {
					const signal = firstAbortOf([clock.signal, interrupted]);
					const url = `${API_PREFIX}${path}`;
					return settleBy(signal, () => fetch(url, { ...init, signal }).then(readWhole));
				},
				catch: networkFailureOf
			}),
		(clock) => Effect.sync(clock.stop)
	);
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
		const response = yield* backendFetch(fetch, '/me', {}, TIME_LIMITS.session);
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

/**
 * The browser-binding cookie in one `Set-Cookie` value, or `undefined` when the
 * value is another cookie, has no token, or carries no `Max-Age`.
 */
export function parseBrowserBindingCookie(setCookie: string): BrowserBindingCookie | undefined {
	const [pair = '', ...attributes] = setCookie.split(';');
	const equalsIndex = pair.indexOf('=');
	if (equalsIndex === -1) return undefined;
	const name = pair.slice(0, equalsIndex).trim();
	const value = pair.slice(equalsIndex + 1).trim();
	if (!isSignInBindingCookieName(name) || value === '') return undefined;
	const lowered = attributes.map((attribute) => attribute.trim().toLowerCase());
	const maxAgeText = lowered.find((attribute) => attribute.startsWith('max-age='))?.slice(8);
	const maxAge = maxAgeText === undefined ? Number.NaN : Number(maxAgeText);
	if (!Number.isInteger(maxAge) || maxAge <= 0) return undefined;
	const secure = lowered.includes('secure');
	return { name, value, maxAge, secure };
}

const liveStartSignin = (fetch: FetchFunction, handle: string) =>
	Effect.gen(function* () {
		const form = new URLSearchParams({ handle });
		// redirect:'manual' exposing the real 303 + Location is server-only (undici)
		// behavior — a browser fetch would see an opaque status-0 redirect instead.
		const init: BackendInit = { method: 'POST', body: form, redirect: 'manual' };
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
			const browserBinding = response.headers
				.getSetCookie()
				.map(parseBrowserBindingCookie)
				.find((cookie) => cookie !== undefined);
			if (browserBinding === undefined) {
				return yield* new ContractViolation({
					path: '/signin',
					status: response.status,
					detail: 'redirect carried no browser-binding cookie'
				});
			}
			const started: StartedSignin = { location, browserBinding };
			return started;
		}
		return yield* problemFailure(response, '/signin').pipe(
			Effect.catchTag('NotAuthenticated', ({ problem }) => new ApiProblem({ problem }))
		);
	});

const liveSignout = (fetch: FetchFunction) =>
	Effect.gen(function* () {
		// Server-only undici semantics again: the 303 + Set-Cookie stay readable.
		const init: BackendInit = { method: 'POST', redirect: 'manual' };
		const response = yield* backendFetch(fetch, '/logout', init, TIME_LIMITS.signout);
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
		const init: BackendInit = {
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

/**
 * `DELETE /accounts/{id}`, its path built by the shared path builder: an id
 * the builder refuses (empty, a dot segment, a slash, anything off the
 * segment allowlist) never reaches `fetch` and answers the same not-found
 * an unknown id gets.
 */
const liveDeleteAccount = (fetch: FetchFunction, id: string) =>
	Effect.gen(function* () {
		const accountPath = segmentPath([id]);
		const path = accountPath === undefined ? undefined : joinPath('/accounts', accountPath);
		if (path === undefined) return yield* new ApiProblem({ problem: ACCOUNT_NOT_FOUND_PROBLEM });
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

/**
 * A Den read over HTTP: `path` is the built API path, `route` the template
 * every failure names instead. A 401 stays `NotAuthenticated`; an answer
 * with no node, or one that doesn't decode, is a `ContractViolation`.
 */
function liveDenRead(
	fetch: FetchFunction,
	path: string,
	route: DenRoute,
	decode: (raw: JsonValue) => DenRead | undefined
): Effect.Effect<DenRead, DenReadError> {
	return Effect.gen(function* () {
		const response = yield* backendFetch(fetch, path);
		if (!response.ok) return yield* problemFailure(response, route);
		const raw = yield* parsedBody(response, route);
		const violation = (detail: string) =>
			new ContractViolation({ path: route, status: response.status, detail });
		const read = yield* Effect.try({
			try: () => decode(raw),
			catch: () => violation('malformed den payload')
		});
		if (read === undefined) return yield* violation('an answer with no node');
		return read;
	});
}

/** `GET /den`: the viewer's own root. */
const liveDenRoot = (fetch: FetchFunction, query: DenQuery) => {
	const path = denApiPathOf([], query);
	if (path === undefined) return Effect.fail(new ApiProblem({ problem: NODE_NOT_FOUND_PROBLEM }));
	return liveDenRead(fetch, path, DEN_ROUTES.root, (raw) =>
		denReadFromRoot(decodeContract(GetDenRootResponseSchema, raw))
	);
};

/** `GET /den/{path}`: a path the builder refuses is the one not-found, with no fetch. */
const liveDenNode = (fetch: FetchFunction, denPath: SegmentPath, query: DenQuery) => {
	const path = denApiPathOf(denPath, query);
	if (path === undefined) return Effect.fail(new ApiProblem({ problem: NODE_NOT_FOUND_PROBLEM }));
	return liveDenRead(fetch, path, DEN_ROUTES.node, (raw) =>
		denReadFromNode(decodeContract(GetDenNodeResponseSchema, raw))
	);
};

/** Where bug reports are filed. */
const BUG_REPORTS_PATH = '/bug-reports';

/** `filing` as the contract's request message: an item left out is absent on the wire. */
function fileBugReportRequest(filing: BugReportFiling) {
	const { window, display } = filing;
	return create(FileBugReportRequestSchema, {
		description: filing.description,
		page: filing.page,
		pageKind: filing.pageType,
		webVersion: filing.webVersion,
		browser: filing.browser,
		window: window === undefined ? undefined : create(FileBugReportRequest_WindowSchema, window),
		display:
			display === undefined ? undefined : create(FileBugReportRequest_DisplaySchema, display),
		errorStatus: filing.errorStatus
	});
}

/**
 * `POST /bug-reports`, encoded through the generated schema; a 2xx answer
 * decodes to the reference, which must be a well-formed one.
 */
const liveFileBugReport = (fetch: FetchFunction, filing: BugReportFiling) =>
	Effect.gen(function* () {
		const request = fileBugReportRequest(filing);
		const init: BackendInit = {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(toJson(FileBugReportRequestSchema, request))
		};
		const response = yield* backendFetch(fetch, BUG_REPORTS_PATH, init);
		if (!response.ok) return yield* problemFailure(response, BUG_REPORTS_PATH);
		const raw = yield* parsedBody(response, BUG_REPORTS_PATH);
		const violation = new ContractViolation({
			path: BUG_REPORTS_PATH,
			status: response.status,
			detail: 'malformed bug report answer'
		});
		const reference = yield* Effect.try({
			try: () => bugReference(decodeContract(FileBugReportResponseSchema, raw).reference),
			catch: () => violation
		});
		if (reference === undefined) return yield* violation;
		return reference;
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
			deleteAccount: (id) => liveDeleteAccount(fetch, id),
			denRoot: (query) => liveDenRoot(fetch, query),
			denNode: (path, query) => liveDenNode(fetch, path, query),
			fileBugReport: (filing) => liveFileBugReport(fetch, filing)
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
	startSignin: () => Effect.fail(new NetworkFailure({ reason: 'unreachable' })),
	signout: Effect.fail(new SignoutFailed({ status: HttpStatus.InternalServerError })),
	listAccounts: Effect.fail(new ApiProblem({ problem: anonymousProblem })),
	createAccount: () => Effect.fail(new ApiProblem({ problem: anonymousProblem })),
	deleteAccount: () => Effect.fail(new ApiProblem({ problem: anonymousProblem })),
	denRoot: () => Effect.fail(new NotAuthenticated({ problem: anonymousProblem })),
	denNode: () => Effect.fail(new NotAuthenticated({ problem: anonymousProblem })),
	fileBugReport: () => Effect.fail(new NotAuthenticated({ problem: anonymousProblem }))
};

/**
 * The in-memory Layer (adapter-mem parity): tests hand in only the behaviors
 * they exercise; everything else answers like an anonymous, signin-less world.
 */
export function zurfurApiTest(overrides: Partial<ZurfurApiShape>): Layer.Layer<ZurfurApi> {
	const shape: ZurfurApiShape = { ...anonymousDefaults, ...overrides };
	return Layer.succeed(ZurfurApi, shape);
}
