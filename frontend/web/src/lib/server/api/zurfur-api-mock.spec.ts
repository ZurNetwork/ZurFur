import { afterEach, describe, expect, it, vi } from 'vitest';
import { Effect } from 'effect';
import { handleFromTrusted } from '$lib/types/brand';
import { accountsOutcome, createAccountOutcome, deleteAccountOutcome } from '../accounts';
import { optionalSession, signinOutcome, signoutOutcome } from '../session';
import {
	createMockStore,
	MOCK_SIGNIN_PATH,
	zurfurApiMock,
	type MockStore
} from './zurfur-api-mock';
import { ZurfurApi } from './zurfur-api';

/** Every mock DID must be shaped like a real did:plc: `did:plc:` + exactly 24 chars of [a-z2-7]. */
const DID_PLC_SHAPE = /^did:plc:[a-z2-7]{24}$/;

/** Run a program against a Layer built over a FRESH store — no bleed between cases. */
function runOn<A, E>(store: MockStore, program: Effect.Effect<A, E, ZurfurApi>): Promise<A> {
	return Effect.runPromise(program.pipe(Effect.provide(zurfurApiMock(store))));
}

/** The one raw `api.me` case this file keeps — {@link optionalSession} folds `NotAuthenticated` to `undefined`, so the tag itself is only observable below the outcome helper. */
const me = Effect.gen(function* () {
	const api = yield* ZurfurApi;
	return yield* api.me;
});

interface MockEnvStub {
	/** Defaults to `true` — most cases below care about the flag, not the dev/build state. */
	dev?: boolean;
	flag: string | undefined;
}

/**
 * Load a FRESH `zurfur-api-mock` module with `$app/environment` and
 * `$env/dynamic/private` stubbed to exactly the two facts
 * {@link import('./zurfur-api-mock').mockModeEnabled} reads — rather than
 * relying on whatever `ZURFUR_WEB_MOCK` happens to be set to in the process
 * actually running the test suite.
 */
async function loadFreshMockModule(stub: MockEnvStub) {
	vi.resetModules();
	vi.doMock('$app/environment', () => ({
		dev: stub.dev ?? true,
		building: false,
		browser: false,
		version: 'test'
	}));
	vi.doMock('$env/dynamic/private', () => ({ env: { ZURFUR_WEB_MOCK: stub.flag } }));
	return import('./zurfur-api-mock');
}

afterEach(() => {
	vi.doUnmock('$app/environment');
	vi.doUnmock('$env/dynamic/private');
	vi.resetModules();
});

describe('zurfurApiMock', () => {
	it('boots signed in as the fixture visitor, with a did:plc-shaped DID', async () => {
		const session = await runOn(createMockStore(), optionalSession);
		expect(session?.did).toMatch(DID_PLC_SHAPE);
		expect(session).toMatchObject({
			handle: handleFromTrusted('alice.zurfur.app'),
			displayName: 'Alice',
			avatarUrl: undefined
		});
	});

	it('lists the seeded owner membership', async () => {
		const outcome = await runOn(createMockStore(), accountsOutcome);
		if ('problem' in outcome) throw new Error('expected the listing to succeed');
		expect(outcome.accounts).toHaveLength(1);
		expect(outcome.accounts[0]?.did).toMatch(DID_PLC_SHAPE);
		expect(outcome.accounts[0]?.role).toBe('owner');
		expect(outcome.accounts[0]?.name).toBe("Alice's Studio");
	});

	it('signout clears the session and returns the cleared cookie name', async () => {
		const store = createMockStore();
		const outcome = await runOn(store, signoutOutcome);
		expect(outcome).toEqual({ clearedCookies: ['zurfur.sid'] });
		expect(store.session).toBeUndefined();
	});

	it('me fails NotAuthenticated after signout', async () => {
		const store = createMockStore();
		store.session = undefined;
		const failure = await runOn(store, Effect.flip(me));
		expect(failure._tag).toBe('NotAuthenticated');
	});

	it('startSignin returns the local mock callback URL, handle encoded', async () => {
		const outcome = await runOn(createMockStore(), signinOutcome('bob test.zurfur.app'));
		expect(outcome).toEqual({ location: `${MOCK_SIGNIN_PATH}?handle=bob%20test.zurfur.app` });
	});

	it('createAccount adds a new owner membership visible in listAccounts', async () => {
		const store = createMockStore();
		const created = await runOn(store, createAccountOutcome('Bob Studio', 'bob-studio.zurfur.app'));
		if ('problem' in created) throw new Error('expected account creation to succeed');
		expect(created.account.name).toBe('Bob Studio');
		expect(created.account.did).toMatch(DID_PLC_SHAPE);

		const listing = await runOn(store, accountsOutcome);
		if ('problem' in listing) throw new Error('expected the listing to succeed');
		expect(listing.accounts).toHaveLength(2);
		expect(
			listing.accounts.some((row) => row.id === created.account.id && row.role === 'owner')
		).toBe(true);
	});

	it('createAccount fails invalid_request for a malformed handle, mirroring the backend', async () => {
		const outcome = await runOn(
			createMockStore(),
			createAccountOutcome('Bob Studio', 'not a handle')
		);
		expect(outcome).toMatchObject({
			problem: {
				code: 'invalid_request',
				type: 'urn:zurfur:error:invalid-request',
				status: 422
			}
		});
	});

	it('deleteAccount removes a known row and reports the outcome', async () => {
		const store = createMockStore();
		const [seeded] = store.accounts;
		if (seeded === undefined) throw new Error('fixture store must seed one account');

		const outcome = await runOn(store, deleteAccountOutcome(seeded.id));
		if ('problem' in outcome) throw new Error('expected the delete to succeed');
		expect(outcome.outcome).toBe('hard');
		expect(store.accounts).toHaveLength(0);
	});

	it('deleteAccount fails account-not-found for an unknown id', async () => {
		const outcome = await runOn(createMockStore(), deleteAccountOutcome('not-a-real-id'));
		expect(outcome).toMatchObject({ problem: { code: 'account_not_found' } });
	});

	it('deleteAccount fails not-authenticated when nobody is signed in', async () => {
		const store = createMockStore();
		store.session = undefined;
		const [seeded] = store.accounts;
		if (seeded === undefined) throw new Error('fixture store must seed one account');

		const outcome = await runOn(store, deleteAccountOutcome(seeded.id));
		expect(outcome).toMatchObject({ problem: { code: 'not_authenticated' } });
	});

	it('fixture isolation: mutating one store leaves a second store built from the same fixtures untouched', () => {
		const storeA = createMockStore();
		const storeB = createMockStore();

		storeA.session = undefined;
		const [membershipA] = storeA.accounts;
		if (membershipA === undefined) throw new Error('fixture store must seed one account');
		membershipA.name = 'Renamed in A';

		expect(storeB.session).toEqual({
			did: storeB.session?.did,
			handle: handleFromTrusted('alice.zurfur.app'),
			displayName: 'Alice',
			avatarUrl: undefined
		});
		expect(storeB.accounts[0]?.name).toBe("Alice's Studio");
	});
});

describe('mockModeRequested', () => {
	it.each([
		{ flag: undefined, requested: false },
		{ flag: '', requested: false },
		{ flag: '0', requested: false },
		{ flag: 'true', requested: false },
		{ flag: '1', requested: true }
	])('ZURFUR_WEB_MOCK=$flag → requested=$requested', async ({ flag, requested }) => {
		const mod = await loadFreshMockModule({ flag });
		expect(mod.mockModeRequested()).toBe(requested);
	});
});

describe('mockSignin', () => {
	it('returns undefined when mock mode is off, and does not mutate the store', async () => {
		const mod = await loadFreshMockModule({ flag: undefined });
		const store = mod.createMockStore();
		const before = store.session;

		const session = mod.mockSignin('carol.zurfur.app', store);

		expect(session).toBeUndefined();
		expect(store.session).toBe(before);
	});

	it('mints a did:plc-shaped session from a valid handle and stores it', async () => {
		const mod = await loadFreshMockModule({ flag: '1' });
		const store = mod.createMockStore();
		store.session = undefined;

		const session = mod.mockSignin('carol.zurfur.app', store);

		expect(session?.handle).toBe(handleFromTrusted('carol.zurfur.app'));
		expect(session?.did).toMatch(DID_PLC_SHAPE);
		expect(store.session).toEqual(session);
	});

	it('falls back to the fixture visitor when the handle is absent', async () => {
		const mod = await loadFreshMockModule({ flag: '1' });
		const store = mod.createMockStore();
		store.session = undefined;

		const session = mod.mockSignin(undefined, store);

		expect(session?.handle).toBe(handleFromTrusted('alice.zurfur.app'));
	});

	it('the fixture-visitor branch clones per call — mutating one store never reaches another store signed in the same way', async () => {
		const mod = await loadFreshMockModule({ flag: '1' });
		const storeA = mod.createMockStore();
		const storeB = mod.createMockStore();
		storeA.session = undefined;
		storeB.session = undefined;

		const sessionA = mod.mockSignin(undefined, storeA);
		const sessionB = mod.mockSignin(undefined, storeB);
		if (sessionA === undefined || sessionB === undefined) throw new Error('expected a session');
		sessionA.displayName = 'Renamed in A';

		expect(sessionB.displayName).toBe('Alice');
	});

	it('returns undefined for a handle that fails claim-tier validation, without touching the store', async () => {
		const mod = await loadFreshMockModule({ flag: '1' });
		const store = mod.createMockStore();
		const before = store.session;

		const session = mod.mockSignin('not a handle', store);

		expect(session).toBeUndefined();
		expect(store.session).toBe(before);
	});

	it('re-establishes the session after a signout', async () => {
		const mod = await loadFreshMockModule({ flag: '1' });
		const store = mod.createMockStore();
		store.session = undefined;

		const session = mod.mockSignin('dana.zurfur.app', store);

		expect(session?.handle).toBe(handleFromTrusted('dana.zurfur.app'));
		expect(store.session).toEqual(session);
	});
});
