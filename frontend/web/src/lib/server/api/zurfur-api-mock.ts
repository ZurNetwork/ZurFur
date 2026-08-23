/**
 * The mock `ZurfurApi` Layer (ZMVP-198): a full, in-memory stand-in for
 * {@link ZurfurApiLive} so `yarn dev` renders a working, signed-in app with
 * no backend, no Docker, and no Caddy. Every fact about whether mock mode is
 * live is read exactly once, here — {@link mockModeRequested},
 * {@link mockModeEnabled}, {@link mockModeMisconfigured} — so
 * `runtime.ts`'s seam branch, the `/mock/signin` route's fail-closed 404,
 * and `hooks.server.ts`'s boot-time prod guard all read the SAME flag the
 * SAME way (feedback_make_unsoundness_unreachable: one shared enforced
 * path). Implements the FULL {@link ZurfurApiShape}, not a `Partial` — when
 * a future ticket grows the shape, this module fails to compile until it
 * grows too (honesty by typecheck; {@link zurfurApiTest}'s anonymous-default
 * stub exists for the opposite reason — tests that only care about ONE call
 * at a time).
 *
 * State is one {@link MockStore}, seeded signed-in with a fixture visitor and
 * one owner membership. The dev process runs on a single module-scope
 * instance (`sharedStore`) so a request's `ZurfurApi` Layer and the
 * `/mock/signin` route see the same world; it resets on server restart or
 * HMR of this module — acceptable and never load-bearing for anything real.
 * A test builds its own store with {@link createMockStore} instead of
 * touching the shared one.
 *
 * Loopback assumption: mock mode is safe only because `vite dev` binds
 * `127.0.0.1` by default — it is unauthenticated, in-memory fixture data,
 * so `just dev-mock` must never be paired with `--host`.
 */

import { building, dev } from '$app/environment';
import { env } from '$env/dynamic/private';
import { Effect, Layer } from 'effect';
import type { AccountMembership, CreatedAccount, DeleteOutcome } from '$lib/api/account';
import {
	ACCOUNT_NOT_FOUND_PROBLEM,
	invalidRequestProblem,
	NOT_AUTHENTICATED_PROBLEM
} from '$lib/api/problem';
import type { Session } from '$lib/api/session';
import { SESSION_COOKIE_NAME } from '$lib/server/api-proxy';
import { accountId, did, handle, handleFromTrusted, type Did } from '$lib/types/brand';
import { ApiProblem, NotAuthenticated } from './errors';
import { ZurfurApi, type ZurfurApiShape } from './zurfur-api';

/** The mock world's mutable state: who is signed in, and which accounts they hold roles in. */
export interface MockStore {
	session: Session | undefined;
	accounts: AccountMembership[];
}

/**
 * The base32 alphabet a real `did:plc` suffix draws from (RFC 4648,
 * lowercase, unpadded — the canonical implementation drops `0`/`1`/`8`/`9` as
 * visually confusable with letters). Every mock DID below is built ONLY from
 * this alphabet, so each is did:plc-SHAPED (`did:plc:` + exactly
 * {@link DID_PLC_SUFFIX_LEN} of these chars) without minting anything real.
 */
const DID_PLC_ALPHABET = 'abcdefghijklmnopqrstuvwxyz234567';
const DID_PLC_SUFFIX_LEN = 24;
const DID_PLC_PAD_CHAR = DID_PLC_ALPHABET.charAt(0);

/** `value` written in {@link DID_PLC_ALPHABET} (plain base-32, most significant digit first). */
function toBase32(value: number): string {
	if (value === 0) return DID_PLC_PAD_CHAR;
	let remaining = value;
	let digits = '';
	while (remaining > 0) {
		digits = DID_PLC_ALPHABET.charAt(remaining % DID_PLC_ALPHABET.length) + digits;
		remaining = Math.floor(remaining / DID_PLC_ALPHABET.length);
	}
	return digits;
}

/**
 * `slug`, filtered to {@link DID_PLC_ALPHABET} and padded/truncated to
 * exactly {@link DID_PLC_SUFFIX_LEN}. A plain indexed loop, not
 * spread/`.split('')` — both decompose on Unicode boundaries that don't
 * matter here (every input is a developer-chosen ASCII slug or
 * {@link toBase32}'s own output) but the lint rule can't know that.
 */
function didPlcSuffix(slug: string): string {
	const lower = slug.toLowerCase();
	let safe = '';
	for (let index = 0; index < lower.length; index += 1) {
		const char = lower.charAt(index);
		if (DID_PLC_ALPHABET.includes(char)) safe += char;
	}
	return (safe + DID_PLC_PAD_CHAR.repeat(DID_PLC_SUFFIX_LEN)).slice(0, DID_PLC_SUFFIX_LEN);
}

/** A did:plc-shaped mock DID for a fixed fixture identity. */
function fixtureDid(slug: string): Did {
	return did(`did:plc:${didPlcSuffix(slug)}`);
}

/** Every did:plc-shaped DID minted after boot draws the next value from this — never from untrusted text (see {@link nextMockDid}). */
let mockDidCounter = 0;

/**
 * A fresh did:plc-shaped DID for a newly mock-minted actor (an account
 * {@link mockCreateAccount} founds, or a visitor {@link mockSignin} signs in
 * as) — a base32-safe COUNTER, never the raw handle text: a handle's `.`/`-`
 * aren't in {@link DID_PLC_ALPHABET} anyway, and embedding untrusted input
 * into a value minted through {@link did} (a TRUSTED-decode cast, not a
 * validating one) would quietly break that constructor's contract.
 */
function nextMockDid(): Did {
	mockDidCounter += 1;
	return fixtureDid(`mock${toBase32(mockDidCounter)}`);
}

/** The fixture visitor mock mode boots signed in as. */
const FIXTURE_SESSION: Session = {
	did: fixtureDid('mockalice'),
	handle: handleFromTrusted('alice.zurfur.app'),
	displayName: 'Alice',
	avatarUrl: undefined
};

/** The one account the fixture visitor owns, seeded into every fresh store. */
const FIXTURE_MEMBERSHIP: AccountMembership = {
	id: accountId('mock-account-alice-studio'),
	did: fixtureDid('mockalicestudio'),
	handle: handleFromTrusted('alice-studio.zurfur.app'),
	name: "Alice's Studio",
	role: 'owner'
};

/**
 * A fresh, signed-in-by-default store. Used both to seed the shared
 * singleton below and, per-test, to give each spec case its own isolated
 * world instead of sharing (and fighting over) the dev singleton. CLONES
 * both fixtures (`{ ...FIXTURE_SESSION }` / `{ ...FIXTURE_MEMBERSHIP }`) —
 * every store must own its OWN objects, never a shared reference to the
 * module constant, or a mutation in one store (a later `signout`, a
 * membership edit) would bleed into every other store built from the same
 * fixture (`sharedStore` included).
 */
export function createMockStore(): MockStore {
	return { session: { ...FIXTURE_SESSION }, accounts: [{ ...FIXTURE_MEMBERSHIP }] };
}

/**
 * The one store the dev process actually runs on. {@link zurfurApiMock} and
 * {@link mockSignin} default to it; a test passes its own store instead.
 */
const sharedStore: MockStore = createMockStore();

/**
 * `ZURFUR_WEB_MOCK=1` was asked for — independent of whether this is a dev
 * build or a build in progress. Read exactly once, here; every other check
 * below composes this one rather than re-reading the env var.
 */
export function mockModeRequested(): boolean {
	return env.ZURFUR_WEB_MOCK === '1';
}

/**
 * Whether mock mode is actually live: requested AND a dev build. The single
 * check `runtime.ts`'s seam branch and the `/mock/signin` route's
 * fail-closed 404 both call, so the two cannot read the flag two different
 * ways.
 */
export function mockModeEnabled(): boolean {
	return dev && mockModeRequested();
}

/**
 * Whether a REAL, booted server is misconfigured: requested, NOT a dev
 * build, and NOT `vite build`'s own postbuild `building` phase. That third
 * term is load-bearing — `just`'s `dotenv-load` applies `.env` to every
 * recipe, so `ZURFUR_WEB_MOCK=1 yarn build` must stay green (the flag never
 * reaches a running server in that case, it only ever gets bundled-and-
 * discarded), while an actual booted, non-dev process with the flag set is
 * a real misconfiguration. `hooks.server.ts` is the ONLY caller — it throws
 * at boot when this is true.
 */
export function mockModeMisconfigured(): boolean {
	return mockModeRequested() && !dev && !building;
}

// Fires exactly once, at this module's first import: the loudest signal a
// dev running mock mode gets that `ZurfurApi` isn't the real backend, short
// of the fixture data itself.
if (mockModeEnabled()) {
	console.warn('[ZURFUR] MOCK MODE — ZurfurApi is in-memory fixtures, not the backend');
}

/** A plain snapshot of `store`'s current session — no `effect` needed; lets a spec (or the route) observe state without the mutable object itself ever leaving this module. */
export function mockSessionSnapshot(store: MockStore = sharedStore): Session | undefined {
	return store.session;
}

/** The local URL {@link mockStartSignin} hands back instead of a PDS authorize URL — exported so nothing re-hardcodes the literal. */
export const MOCK_SIGNIN_PATH = '/mock/signin';

/** `me`: the store's current session, or `NotAuthenticated` with the backend's own anonymous shape. */
function mockMe(store: MockStore): ZurfurApiShape['me'] {
	return Effect.suspend(() =>
		store.session === undefined
			? Effect.fail(new NotAuthenticated({ problem: NOT_AUTHENTICATED_PROBLEM }))
			: Effect.succeed(store.session)
	);
}

/** `startSignin`: never dials a PDS — hands back {@link MOCK_SIGNIN_PATH}, which the login form's existing redirect-to-`location` handling already knows how to follow. */
function mockStartSignin(requestedHandle: string): ReturnType<ZurfurApiShape['startSignin']> {
	return Effect.succeed(`${MOCK_SIGNIN_PATH}?handle=${encodeURIComponent(requestedHandle)}`);
}

/** `signout`: clears the store's session; succeeds with the one cookie name the real backend clears (`logout`'s action reads names off this return). */
function mockSignout(store: MockStore): ZurfurApiShape['signout'] {
	return Effect.sync(() => {
		store.session = undefined;
		return [SESSION_COOKIE_NAME];
	});
}

/** `listAccounts`: a COPY of the store's rows (never the live array — a caller must not be able to mutate mock state by holding onto what this returns), or `ApiProblem` when nobody is signed in. */
function mockListAccounts(store: MockStore): ZurfurApiShape['listAccounts'] {
	return Effect.suspend(() =>
		store.session === undefined
			? Effect.fail(new ApiProblem({ problem: NOT_AUTHENTICATED_PROBLEM }))
			: Effect.succeed([...store.accounts])
	);
}

/**
 * `createAccount`: mints a fresh fixture-shaped row, appends it as an owner
 * membership, and returns it. `requestedHandle` is UNTRUSTED input (a form
 * field, ultimately), so it goes through {@link handle}'s VALIDATED mint —
 * never {@link handleFromTrusted} — and a handle that fails it answers the
 * SAME `ApiProblem` the real `POST /accounts` gives a malformed handle:
 * {@link invalidRequestProblem}, the backend's own `Problem::invalid_request`
 * shape (`backend/crates/api/src/routes/accounts.rs:325-326` calls it for
 * exactly this failure).
 */
function mockCreateAccount(
	store: MockStore,
	name: string,
	requestedHandle: string
): ReturnType<ZurfurApiShape['createAccount']> {
	return Effect.suspend(() => {
		if (store.session === undefined)
			return Effect.fail(new ApiProblem({ problem: NOT_AUTHENTICATED_PROBLEM }));
		const validatedHandle = handle(requestedHandle);
		if (validatedHandle === undefined) {
			const problem = invalidRequestProblem(
				"That handle isn't shaped like a valid atproto handle."
			);
			return Effect.fail(new ApiProblem({ problem }));
		}
		const created: CreatedAccount = {
			id: accountId(crypto.randomUUID()),
			did: nextMockDid(),
			handle: validatedHandle,
			name
		};
		store.accounts.push({ ...created, role: 'owner' });
		return Effect.succeed(created);
	});
}

/** `deleteAccount`: authorization precedes lookup (anonymous fails BEFORE the id is even checked, matching every sibling call), then removes a known row and reports `'hard'` (the mock never keeps a fact-bearing tombstone), or `ApiProblem` account-not-found for an id the store doesn't hold. */
function mockDeleteAccount(
	store: MockStore,
	id: string
): ReturnType<ZurfurApiShape['deleteAccount']> {
	return Effect.suspend(() => {
		if (store.session === undefined)
			return Effect.fail(new ApiProblem({ problem: NOT_AUTHENTICATED_PROBLEM }));
		const index = store.accounts.findIndex((row) => row.id === id);
		if (index === -1) return Effect.fail(new ApiProblem({ problem: ACCOUNT_NOT_FOUND_PROBLEM }));
		store.accounts.splice(index, 1);
		return Effect.succeed<DeleteOutcome>('hard');
	});
}

/**
 * The dev-mode Layer: the full {@link ZurfurApiShape} over `store` (the
 * shared singleton by default), six uniform `mock*`-named entries. Callers
 * that run in a real server hold this Layer for the whole process
 * (`runtime.ts` builds it once at module scope, ZMVP-198) rather than
 * rebuilding it per request — safe because every entry below is already
 * LAZY (`Effect.suspend`/`Effect.sync`), so state is read at EFFECT-RUN
 * time, never at Layer-construction time. Kept as a parameterized factory
 * (not just the module-scope singleton) so a spec can build one over its own
 * isolated store.
 */
export function zurfurApiMock(store: MockStore = sharedStore): Layer.Layer<ZurfurApi> {
	const shape: ZurfurApiShape = {
		me: mockMe(store),
		startSignin: mockStartSignin,
		signout: mockSignout(store),
		listAccounts: mockListAccounts(store),
		createAccount: (name, requestedHandle) => mockCreateAccount(store, name, requestedHandle),
		deleteAccount: (id) => mockDeleteAccount(store, id)
	};
	return Layer.succeed(ZurfurApi, shape);
}

/**
 * `/mock/signin`'s whole job: mint a session for the query param `rawHandle`
 * and store it, or answer `undefined` — a PLAIN function, not an `Effect`:
 * the route that calls it is a `+server.ts`, which the containment glob
 * (`src/**\/*.server.ts`) does NOT match — there is no dot before "server" in
 * `+server.ts` — so `effect` must stay out of that file entirely.
 *
 * Fail-closed and containment-checked from its own first line, not only at
 * the call site (defense in depth — a future second caller gets the same
 * guarantee for free): mock mode off answers `undefined` without touching
 * `store` at all. `rawHandle` absent boots the fixture visitor. A PRESENT
 * `rawHandle` is UNTRUSTED input, so it goes through {@link handle}'s
 * VALIDATED mint (never {@link handleFromTrusted}, which is for
 * already-trusted decode boundaries only); a handle that fails that
 * claim-tier check answers `undefined` too, and — unlike the earlier
 * revision of this function — WITHOUT a silent fallback to the fixture
 * session, so a caller can tell "you typed something the mock can't accept"
 * apart from "you signed in as the default visitor".
 */
export function mockSignin(
	rawHandle: string | undefined,
	store: MockStore = sharedStore
): Session | undefined {
	if (!mockModeEnabled()) return undefined;

	if (rawHandle === undefined) {
		// Clone, not the shared constant itself — every `store.session` must own
		// its OWN object (see createMockStore's doc), including the one this
		// signin mints; aliasing FIXTURE_SESSION here would let one store's
		// later mutation reach into every other "signed in as the fixture
		// visitor" store, sharedStore included.
		const session: Session = { ...FIXTURE_SESSION };
		store.session = session;
		return session;
	}

	const validated = handle(rawHandle);
	if (validated === undefined) return undefined;

	const session: Session = {
		did: nextMockDid(),
		handle: validated,
		displayName: undefined,
		avatarUrl: undefined
	};
	store.session = session;
	return session;
}
