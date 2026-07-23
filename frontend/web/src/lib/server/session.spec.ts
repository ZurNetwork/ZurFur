import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import type { Session } from '$lib/api/session';
import { ApiProblem, ContractViolation, NetworkFailure } from './api/errors';
import { zurfurApiTest, type ZurfurApi } from './api/zurfur-api';
import { sessionOrAnonymous, sessionOrNull, signinOutcome, signoutOutcome } from './session';

const alice: Session = {
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	display_name: 'Alice',
	avatar_url: null
};

/** Run a program against the in-memory Layer — no fetch, no network. */
function runTest<A, E>(
	overrides: Parameters<typeof zurfurApiTest>[0],
	program: Effect.Effect<A, E, ZurfurApi>
): Promise<A> {
	return Effect.runPromise(program.pipe(Effect.provide(zurfurApiTest(overrides))));
}

describe('sessionOrNull', () => {
	it('carries the session for a signed-in visitor', async () => {
		const session = await runTest({ me: Effect.succeed(alice) }, sessionOrNull);
		expect(session).toEqual(alice);
	});

	it('is null for an anonymous visitor (the 401 branch)', async () => {
		const session = await runTest({}, sessionOrNull);
		expect(session).toBeNull();
	});
});

describe('sessionOrAnonymous', () => {
	it('degrades an unreachable backend to anonymous', async () => {
		const unreachableMe = Effect.fail(new NetworkFailure({ cause: new TypeError('fetch failed') }));
		const session = await runTest({ me: unreachableMe }, sessionOrAnonymous);
		expect(session).toBeNull();
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
	it('carries the authorize location on success', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const outcome = await runTest(
			{ startSignin: () => Effect.succeed(authorizeUrl) },
			signinOutcome('alice.zurfur.app')
		);
		expect(outcome).toEqual({ location: authorizeUrl });
	});

	it('carries the problem when the backend rejects the handle', async () => {
		const problem = {
			type: 'urn:zurfur:error:invalid-request',
			code: 'invalid_request',
			title: 'invalid_request',
			status: 422
		};
		const rejected = () => Effect.fail(new ApiProblem({ problem }));
		const outcome = await runTest({ startSignin: rejected }, signinOutcome('not a handle'));
		expect(outcome).toEqual({ problem });
	});
});

describe('signoutOutcome', () => {
	it('carries the cleared cookie names on success', async () => {
		const outcome = await runTest({ signout: Effect.succeed(['zurfur.sid']) }, signoutOutcome);
		expect(outcome).toEqual({ clearedCookies: ['zurfur.sid'] });
	});

	it('carries the failing status when the backend does not redirect', async () => {
		const outcome = await runTest({}, signoutOutcome);
		expect(outcome).toEqual({ failedStatus: 500 });
	});
});
