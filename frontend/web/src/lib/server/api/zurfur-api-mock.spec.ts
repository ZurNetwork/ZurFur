import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { Effect } from 'effect';
import { fetchStub } from '$lib/testing/http';
import { commissionId, fieldPointer, handleFromTrusted, type CommissionId } from '$lib/types/brand';
import { segmentPath } from '../path-builder';
import { accountsOutcome, createAccountOutcome, deleteAccountOutcome } from '../accounts';
import type { BugDescription } from '$lib/types/bug-report-rules';
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

/** The raw create call. */
const createCommission = (title: string) =>
	Effect.flatMap(ZurfurApi, (api) => api.createCommission(title));

/** The raw rename call. */
const renameCommission = (id: CommissionId, title: string) =>
	Effect.flatMap(ZurfurApi, (api) => api.renameCommission(id, title));

/** The names the mock Den lists in `commissions`. */
const commissionNames = Effect.gen(function* () {
	const api = yield* ZurfurApi;
	const path = segmentPath(['commissions']);
	if (path === undefined) return [];
	const read = yield* api.denNode(path, { includeDeleted: false, pageToken: undefined });
	return read.content.content === 'listing' ? read.content.entries.map((node) => node.name) : [];
});

/** The one raw `api.me` case this file keeps — {@link optionalSession} folds `NotAuthenticated` to `undefined`, so the tag itself is only observable below the outcome helper. */
const me = Effect.gen(function* () {
	const api = yield* ZurfurApi;
	return yield* api.me;
});

interface MockEnvStub {
	/** Defaults to `true` — most cases below care about the flag, not the dev/build state. */
	dev?: boolean;
	/** Defaults to `false`: a running server, not `vite build`'s postbuild phase. */
	building?: boolean;
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
	stubEnvironment(stub);
	return import('./zurfur-api-mock');
}

/** Start a fresh module graph whose `$app/environment` and `$env/dynamic/private` say exactly `stub`. */
function stubEnvironment(stub: MockEnvStub): void {
	vi.resetModules();
	vi.doMock('$app/environment', () => ({
		dev: stub.dev ?? true,
		building: stub.building ?? false,
		browser: false,
		version: 'test'
	}));
	vi.doMock('$env/dynamic/private', () => ({ env: { ZURFUR_WEB_MOCK: stub.flag } }));
}

/** How long the one cold import may take on a loaded machine. */
const WARM_IMPORT_TIMEOUT_MS = 60_000;

// Pay the cold transform of the module graph once, under its own timeout, so
// each test's fresh import after `vi.resetModules()` is only an evaluation.
beforeAll(async () => {
	await import('./zurfur-api-mock');
	await import('../runtime');
	await import('../session');
}, WARM_IMPORT_TIMEOUT_MS);

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
		expect(outcome).toMatchObject({
			location: `${MOCK_SIGNIN_PATH}?handle=bob%20test.zurfur.app`
		});
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

	it('createCommission stores the created commission, its title trimmed', async () => {
		const store = createMockStore();

		const created = await runOn(store, createCommission('  Fox in the snow  '));

		expect(created.title).toBe('Fox in the snow');
		expect(store.commissions).toEqual([
			{ segment: created.id, id: created.id, title: 'Fox in the snow' }
		]);
	});

	it.each([
		['a blank title', '', 'name_empty', "This can't be blank."],
		['a title of spaces', '   ', 'name_empty', "This can't be blank."],
		[
			'a title holding U+202E',
			'Fox\u202Ein the snow',
			'name_invisible',
			'Leave out invisible and direction-changing characters.'
		]
	])(
		'createCommission refuses %s as invalid_request at #/title',
		async (_, title, code, detail) => {
			const store = createMockStore();

			const failure = await runOn(store, Effect.flip(createCommission(title)));

			expect(failure._tag).toBe('ApiProblem');
			if (failure._tag !== 'ApiProblem') return;
			expect(failure.problem.code).toBe('invalid_request');
			expect(failure.problem.status).toBe(422);
			expect(failure.problem.errors).toEqual([{ pointer: fieldPointer('#/title'), code, detail }]);
			expect(store.commissions).toEqual([]);
		}
	);

	it('createCommission fails not-authenticated when nobody is signed in', async () => {
		const store = createMockStore();
		store.session = undefined;

		const failure = await runOn(store, Effect.flip(createCommission('Fox in the snow')));

		expect(failure).toMatchObject({ problem: { code: 'not_authenticated' } });
		expect(store.commissions).toEqual([]);
	});

	it('renameCommission renames a created commission, and the Den lists the new title', async () => {
		const store = createMockStore();
		const created = await runOn(store, createCommission('Fox in the snow'));

		const renamed = await runOn(store, renameCommission(created.id, '  Winter fox  '));

		expect(renamed).toEqual({ id: created.id, title: 'Winter fox' });
		const names = await runOn(store, commissionNames);
		expect(names).toContain('Winter fox');
		expect(names).not.toContain('Fox in the snow');
	});

	it.each([
		['a blank title', '', 'name_empty', "This can't be blank."],
		['a title of spaces', '   ', 'name_empty', "This can't be blank."],
		[
			'a title holding U+202E',
			'Fox\u202Ein the snow',
			'name_invisible',
			'Leave out invisible and direction-changing characters.'
		]
	])(
		'renameCommission refuses %s as invalid_request at #/title',
		async (_, title, code, detail) => {
			const store = createMockStore();
			const created = await runOn(store, createCommission('Fox in the snow'));

			const failure = await runOn(store, Effect.flip(renameCommission(created.id, title)));

			expect(failure._tag).toBe('ApiProblem');
			if (failure._tag !== 'ApiProblem') return;
			expect(failure.problem.code).toBe('invalid_request');
			expect(failure.problem.status).toBe(422);
			expect(failure.problem.errors).toEqual([{ pointer: fieldPointer('#/title'), code, detail }]);
			expect(store.commissions.map((commission) => commission.title)).toEqual(['Fox in the snow']);
		}
	);

	it.each([
		['an id it does not hold', 'c-unknown'],
		[
			'the card "Some commission", which it shows but not open',
			'0192c0de-7a11-7000-8000-00000000ca7d'
		]
	])('renameCommission answers %s with the commission not-found', async (_, raw) => {
		const store = createMockStore();
		const id = commissionId(raw);
		if (id === undefined) throw new Error('bad test id');

		const failure = await runOn(store, Effect.flip(renameCommission(id, 'Winter fox')));

		expect(failure).toMatchObject({ problem: { code: 'commission_not_found', status: 404 } });
	});

	it.each([
		['the fixed fixture "Untitled"', '01a0ef9c-5b2e-7c41-9d3a-6f1e2b7c8d90'],
		['the archived fixture "Winter YCH"', '0192b7e0-0aa0-7000-8000-00000000a7c1']
	])(
		'renameCommission answers %s, which it shows open but never created, 403 forbidden',
		async (_, raw) => {
			const store = createMockStore();
			const id = commissionId(raw);
			if (id === undefined) throw new Error('bad test id');

			const failure = await runOn(store, Effect.flip(renameCommission(id, 'Winter fox')));

			expect(failure).toMatchObject({ problem: { code: 'forbidden', status: 403 } });
		}
	);

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

describe('zurfurApiMock: fileBugReport', () => {
	const filing = {
		description: 'Broken.' as BugDescription,
		page: undefined,
		pageType: undefined,
		webVersion: undefined,
		browser: undefined,
		window: undefined,
		display: undefined,
		errorStatus: undefined
	};
	const fileBugReport = Effect.flatMap(ZurfurApi, (api) => api.fileBugReport(filing));

	it('answers a made-up eight-character reference and keeps nothing', async () => {
		const store = createMockStore();
		const before = structuredClone(store);

		const reference = await runOn(store, fileBugReport);

		expect(reference).toMatch(/^[0-9a-f]{8}$/);
		expect(store).toStrictEqual(before);
	});

	it('fails NotAuthenticated when nobody is signed in', async () => {
		const store = createMockStore();
		store.session = undefined;
		const failure = await runOn(store, Effect.flip(fileBugReport));
		expect(failure._tag).toBe('NotAuthenticated');
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

describe('mockModeEnabled', () => {
	it.each([
		{ dev: true, flag: '1', enabled: true },
		{ dev: true, flag: undefined, enabled: false },
		{ dev: false, flag: '1', enabled: false },
		{ dev: false, flag: undefined, enabled: false }
	])('dev=$dev, ZURFUR_WEB_MOCK=$flag → enabled=$enabled', async ({ dev, flag, enabled }) => {
		const mod = await loadFreshMockModule({ dev, flag });
		expect(mod.mockModeEnabled()).toBe(enabled);
	});

	it('leaves a non-dev server with the flag set on the live API, never the fixture world', async () => {
		stubEnvironment({ dev: false, flag: '1' });
		const { runApi } = await import('../runtime');
		const { optionalSession } = await import('../session');
		const { fetch, calls } = fetchStub(() =>
			Response.json({ did: 'did:plc:bob', handle: 'bob.zurfur.app' })
		);

		const session = await runApi(fetch, optionalSession);

		expect(calls).toEqual(['/api/v1/me']);
		expect(session?.did).toBe('did:plc:bob');
	});
});

describe('mockModeMisconfigured', () => {
	it.each([
		{ dev: false, building: false, flag: '1', misconfigured: true },
		{ dev: true, building: false, flag: '1', misconfigured: false },
		{ dev: false, building: true, flag: '1', misconfigured: false },
		{ dev: false, building: false, flag: undefined, misconfigured: false }
	])(
		'dev=$dev, building=$building, ZURFUR_WEB_MOCK=$flag → misconfigured=$misconfigured',
		async ({ dev, building, flag, misconfigured }) => {
			const mod = await loadFreshMockModule({ dev, building, flag });
			expect(mod.mockModeMisconfigured()).toBe(misconfigured);
		}
	);
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
