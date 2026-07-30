import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import type { AccountMembership, CreatedAccount } from '$lib/api/account';
import { ApiProblem } from './api/errors';
import { zurfurApiTest, type ZurfurApi } from './api/zurfur-api';
import {
	accountOutcome,
	accountsOutcome,
	createAccountOutcome,
	deleteAccountOutcome
} from './accounts';

/** Run a program against the in-memory Layer — no fetch, no network. */
function runTest<A, E>(
	overrides: Parameters<typeof zurfurApiTest>[0],
	program: Effect.Effect<A, E, ZurfurApi>
): Promise<A> {
	return Effect.runPromise(program.pipe(Effect.provide(zurfurApiTest(overrides))));
}

const aliceStudio: AccountMembership = {
	id: 'acct-alice',
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	name: 'Alice Studio',
	role: 'owner'
};

const handleTakenProblem = {
	type: 'urn:zurfur:error:handle-taken',
	code: 'handle_taken',
	title: 'handle_taken',
	detail: 'That handle is already claimed.',
	status: 409
};

describe('accountsOutcome', () => {
	it('carries the rows a stubbed listAccounts hands it', async () => {
		const outcome = await runTest({ listAccounts: Effect.succeed([aliceStudio]) }, accountsOutcome);
		expect(outcome).toEqual({ accounts: [aliceStudio] });
	});

	it('carries the problem when the backend rejects the listing', async () => {
		const problem = {
			type: 'urn:zurfur:error:rate-limited',
			code: 'rate_limited',
			title: 'rate_limited',
			detail: 'Slow down.',
			status: 429
		};
		const outcome = await runTest(
			{ listAccounts: Effect.fail(new ApiProblem({ problem })) },
			accountsOutcome
		);
		expect(outcome).toEqual({ problem });
	});
});

describe('accountOutcome (⚠️ F1 — derived from the listing, no GET /accounts/{id})', () => {
	it('finds the account by id in the listing', async () => {
		const outcome = await runTest(
			{ listAccounts: Effect.succeed([aliceStudio]) },
			accountOutcome('acct-alice')
		);
		expect(outcome).toEqual({ account: aliceStudio });
	});

	it('answers a typed not-found outcome for an id absent from the listing', async () => {
		const outcome = await runTest(
			{ listAccounts: Effect.succeed([aliceStudio]) },
			accountOutcome('acct-nobody-holds-a-role-in')
		);
		expect(outcome).toEqual({
			problem: {
				type: 'urn:zurfur:error:account-not-found',
				code: 'account_not_found',
				title: 'Account not found',
				detail: 'No such account.',
				status: 404
			}
		});
	});

	it('propagates a listing problem instead of masking it as not-found', async () => {
		const problem = {
			type: 'urn:zurfur:error:rate-limited',
			code: 'rate_limited',
			title: 'rate_limited',
			detail: 'Slow down.',
			status: 429
		};
		const outcome = await runTest(
			{ listAccounts: Effect.fail(new ApiProblem({ problem })) },
			accountOutcome('acct-alice')
		);
		expect(outcome).toEqual({ problem });
	});
});

describe('createAccountOutcome', () => {
	it('carries the founded account on success', async () => {
		const created: CreatedAccount = {
			id: 'acct-new',
			did: 'did:plc:new',
			handle: 'new.zurfur.app',
			name: 'New Studio'
		};
		const outcome = await runTest(
			{ createAccount: () => Effect.succeed(created) },
			createAccountOutcome('New Studio', 'new.zurfur.app')
		);
		expect(outcome).toEqual({ account: created });
	});

	it('carries the problem when founding is rejected (handle_taken)', async () => {
		const outcome = await runTest(
			{ createAccount: () => Effect.fail(new ApiProblem({ problem: handleTakenProblem })) },
			createAccountOutcome('New Studio', 'taken.zurfur.app')
		);
		expect(outcome).toEqual({ problem: handleTakenProblem });
	});
});

describe('deleteAccountOutcome', () => {
	it('carries the outcome on success', async () => {
		const outcome = await runTest(
			{ deleteAccount: () => Effect.succeed('hard') },
			deleteAccountOutcome('acct-alice')
		);
		expect(outcome).toEqual({ outcome: 'hard' });
	});

	it("carries the 'unknown' fallback outcome without treating it as a problem", async () => {
		const outcome = await runTest(
			{ deleteAccount: () => Effect.succeed('unknown') },
			deleteAccountOutcome('acct-alice')
		);
		expect(outcome).toEqual({ outcome: 'unknown' });
	});

	it('carries the problem when the caller is not the Owner (forbidden)', async () => {
		const problem = {
			type: 'urn:zurfur:error:forbidden',
			code: 'forbidden',
			title: 'Forbidden',
			detail: "You don't have permission to perform this action.",
			status: 403
		};
		const outcome = await runTest(
			{ deleteAccount: () => Effect.fail(new ApiProblem({ problem })) },
			deleteAccountOutcome('acct-alice')
		);
		expect(outcome).toEqual({ problem });
	});
});
