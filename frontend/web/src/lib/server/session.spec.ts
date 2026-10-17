import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import type { Session } from '$lib/api/session';
import { did, handleFromTrusted } from '$lib/types/brand';
import { ApiProblem, ContractViolation, NetworkFailure } from './api/errors';
import { zurfurApiTest, type ZurfurApi } from './api/zurfur-api';
import {
	optionalSession,
	sessionCheck,
	sessionOrAnonymous,
	signinOutcome,
	signoutOutcome
} from './session';

const alice: Session = {
	did: did('did:plc:alice'),
	handle: handleFromTrusted('alice.zurfur.app'),
	displayName: 'Alice',
	avatarUrl: undefined
};

/** Run a program against the in-memory Layer — no fetch, no network. */
function runTest<A, E>(
	overrides: Parameters<typeof zurfurApiTest>[0],
	program: Effect.Effect<A, E, ZurfurApi>
): Promise<A> {
	return Effect.runPromise(program.pipe(Effect.provide(zurfurApiTest(overrides))));
}

describe('optionalSession', () => {
	it('carries the session for a signed-in visitor', async () => {
		const session = await runTest({ me: Effect.succeed(alice) }, optionalSession);
		expect(session).toEqual(alice);
	});

	it('is undefined for an anonymous visitor (the 401 branch)', async () => {
		const session = await runTest({}, optionalSession);
		expect(session).toBeUndefined();
	});
});

describe('sessionCheck', () => {
	it('is signed in, with the session, for a signed-in visitor', async () => {
		const check = await runTest({ me: Effect.succeed(alice) }, sessionCheck);
		expect(check).toEqual({ check: 'signedIn', session: alice });
	});

	it('is signed out on the 401', async () => {
		expect(await runTest({}, sessionCheck)).toEqual({ check: 'signedOut' });
	});

	it.each(['unreachable', 'timeout'] as const)(
		'is unanswered, never signed out, when the backend is %s',
		async (reason) => {
			const me = Effect.fail(new NetworkFailure({ reason }));
			expect(await runTest({ me }, sessionCheck)).toEqual({ check: 'unanswered' });
		}
	);

	it('surfaces a broken contract', async () => {
		const brokenMe = Effect.fail(
			new ContractViolation({ path: '/me', status: 504, detail: 'no problem body' })
		);
		const failure = await runTest({ me: brokenMe }, Effect.flip(sessionCheck));
		expect(failure._tag).toBe('ContractViolation');
	});
});

describe('sessionOrAnonymous', () => {
	it('degrades an unreachable backend to anonymous', async () => {
		const unreachableMe = Effect.fail(new NetworkFailure({ reason: 'unreachable' }));
		const session = await runTest({ me: unreachableMe }, sessionOrAnonymous);
		expect(session).toBeUndefined();
	});

	it('surfaces a broken contract instead of treating it as signed-out', async () => {
		const brokenMe = Effect.fail(
			new ContractViolation({ path: '/me', status: 504, detail: 'no problem body' })
		);
		const failure = await runTest({ me: brokenMe }, Effect.flip(sessionOrAnonymous));
		expect(failure._tag).toBe('ContractViolation');
	});
});

describe('signinOutcome', () => {
	it('carries the authorize location and the browser-binding cookie on success', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const browserBinding = {
			name: 'zurfur.signin' as const,
			value: 'token',
			maxAge: 600,
			secure: false
		};
		const started = { location: authorizeUrl, browserBinding };
		const outcome = await runTest(
			{ startSignin: () => Effect.succeed(started) },
			signinOutcome('alice.zurfur.app')
		);
		expect(outcome).toEqual({ location: authorizeUrl, browserBinding });
	});

	it('carries the problem when the backend rejects the handle', async () => {
		const problem = {
			type: 'urn:zurfur:error:invalid-request',
			code: 'invalid_request',
			title: 'invalid_request',
			detail: 'That handle is not usable.',
			status: 422,
			errors: []
		};
		const rejected = () => Effect.fail(new ApiProblem({ problem }));
		const outcome = await runTest({ startSignin: rejected }, signinOutcome('not a handle'));
		expect(outcome).toEqual({ problem });
	});
});

describe('signinOutcome when Zurfur doesn’t answer', () => {
	it.each(['unreachable', 'timeout'] as const)('words a %s start for the form', async (reason) => {
		const startSignin = () => Effect.fail(new NetworkFailure({ reason }));
		const outcome = await runTest({ startSignin }, signinOutcome('alice.zurfur.app'));
		expect(outcome).toMatchObject({
			problem: {
				code: 'service_unavailable',
				detail: "Zurfur isn't answering right now. Please try again.",
				status: 503
			}
		});
	});
});

describe('signoutOutcome', () => {
	it('carries the cleared cookie names on success', async () => {
		const outcome = await runTest({ signout: Effect.succeed(['zurfur.sid']) }, signoutOutcome);
		expect(outcome).toEqual({ clearedCookies: ['zurfur.sid'] });
	});

	it('is unconfirmed (refused) when the backend does not redirect', async () => {
		const outcome = await runTest({}, signoutOutcome);
		expect(outcome).toEqual({ unconfirmed: 'refused' });
	});

	it('is unconfirmed (unreachable) when the backend can’t be reached', async () => {
		const unreachable = Effect.fail(new NetworkFailure({ reason: 'unreachable' }));
		const outcome = await runTest({ signout: unreachable }, signoutOutcome);
		expect(outcome).toEqual({ unconfirmed: 'unreachable' });
	});
});
