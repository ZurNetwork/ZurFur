/**
 * Account programs over the {@link import('./api/zurfur-api').ZurfurApi} port
 * — the Account successors of {@link import('./session')}'s outcome-union
 * shape. Pages run these through {@link import('./runtime').runApi}.
 */

import { Effect } from 'effect';
import type { AccountMembership, CreatedAccount, DeleteOutcome } from '$lib/api/account';
import { type Problem, ProblemKind } from '$lib/api/problem';
import { HttpStatus } from '$lib/api/http-status';
import { ZurfurApi } from './api/zurfur-api';
import type { ContractViolation, NetworkFailure } from './api/errors';

/** The two ways the listing comes back: the rows, or a problem to render. */
export type AccountsOutcome = { accounts: ReadonlyArray<AccountMembership> } | { problem: Problem };

/** Every account the caller holds a role in, or the problem that blocked the read. */
export const accountsOutcome: Effect.Effect<
	AccountsOutcome,
	NetworkFailure | ContractViolation,
	ZurfurApi
> = Effect.gen(function* () {
	const api = yield* ZurfurApi;
	const accounts = yield* api.listAccounts;
	return { accounts } satisfies AccountsOutcome;
}).pipe(
	Effect.catchTag('ApiProblem', ({ problem }) => Effect.succeed<AccountsOutcome>({ problem }))
);

/**
 * The synthetic 404 ⚠️ F1's derived read answers for an id absent from the
 * listing. Field-for-field the domain's own `Problem::account_not_found`
 * (`backend/crates/api/src/problem.rs`) — reused verbatim rather than
 * invented, because the two cases mean the same thing: "not in your list"
 * IS "you hold no role in it", the same authorization answer a real detail
 * endpoint would give.
 */
const accountNotFoundProblem: Problem = {
	...ProblemKind.AccountNotFound,
	title: 'Account not found',
	detail: 'No such account.',
	status: HttpStatus.NotFound
};

/** The two ways a single-account read comes back: the row, or a problem (including the derived not-found) to render. */
export type AccountOutcome = { account: AccountMembership } | { problem: Problem };

/**
 * One account, resolved by id from the listing (⚠️ F1 — there is no
 * `GET /accounts/{id}` on the wire; `contract_routes.rs` pins the corpus at
 * exactly 9 endpoints and this ticket keeps that promise). A listing
 * problem propagates as itself rather than being reported as not-found.
 */
export function accountOutcome(
	id: string
): Effect.Effect<AccountOutcome, NetworkFailure | ContractViolation, ZurfurApi> {
	return accountsOutcome.pipe(
		Effect.map((outcome): AccountOutcome => {
			if ('problem' in outcome) return outcome;
			const account = outcome.accounts.find((row) => row.id === id);
			return account === undefined ? { problem: accountNotFoundProblem } : { account };
		})
	);
}

/** The two ways founding comes back: the new account, or a problem (`handle_taken` / `invalid_request`) to render. */
export type CreateAccountOutcome = { account: CreatedAccount } | { problem: Problem };

/** Found an account; the caller becomes its Owner. */
export function createAccountOutcome(
	name: string,
	handle: string
): Effect.Effect<CreateAccountOutcome, NetworkFailure | ContractViolation, ZurfurApi> {
	const created = Effect.gen(function* () {
		const api = yield* ZurfurApi;
		const account = yield* api.createAccount(name, handle);
		return { account } satisfies CreateAccountOutcome;
	});
	return created.pipe(
		Effect.catchTag('ApiProblem', ({ problem }) =>
			Effect.succeed<CreateAccountOutcome>({ problem })
		)
	);
}

/** How a delete lands: which deletion happened (never a raw `ApiProblem` even for the `'unknown'` R8 fallback), or a problem (`forbidden` / `account_not_found`) to render. */
export type DeleteAccountOutcome = { outcome: DeleteOutcome } | { problem: Problem };

/** Delete an account. Owner-only; a non-Owner or an already-gone id surfaces as `{problem}`. */
export function deleteAccountOutcome(
	id: string
): Effect.Effect<DeleteAccountOutcome, NetworkFailure | ContractViolation, ZurfurApi> {
	const deleted = Effect.gen(function* () {
		const api = yield* ZurfurApi;
		const outcome = yield* api.deleteAccount(id);
		return { outcome } satisfies DeleteAccountOutcome;
	});
	return deleted.pipe(
		Effect.catchTag('ApiProblem', ({ problem }) =>
			Effect.succeed<DeleteAccountOutcome>({ problem })
		)
	);
}
