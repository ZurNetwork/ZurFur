import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import AccountDetailPage from './+page.svelte';
import { FORBIDDEN_PROBLEM } from '$lib/api/problem';
import type { AccountMembership } from '$lib/api/account';
import type { Problem } from '$lib/api/problem';
import type { Session } from '$lib/api/session';

/** Every accounts-group page requires a session (the layout gate) — the group's layout passes it through untouched, so every render here carries one. */
const alice: Session = {
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	displayName: 'Alice',
	avatarUrl: undefined
};

const aliceOwner: AccountMembership = {
	id: 'acct-alice',
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	name: 'Alice Studio',
	role: 'owner'
};

/** The REAL constant the delete action returns — so this spec pins the copy a user actually sees. */
const forbiddenProblem: Problem = FORBIDDEN_PROBLEM;

const notFoundProblem: Problem = {
	type: 'urn:zurfur:error:account-not-found',
	code: 'account_not_found',
	title: 'Account not found',
	detail: 'No such account.',
	status: 404
};

function accountData(account: AccountMembership) {
	return { session: alice, account };
}

function problemData(problem: Problem) {
	return { session: alice, problem };
}

describe('/accounts/[id] page', () => {
	it('renders the account and the delete form for an owner', async () => {
		render(AccountDetailPage, { data: accountData(aliceOwner), form: null });

		await expect
			.element(page.getByRole('heading', { level: 1 }))
			.toHaveTextContent('alice.zurfur.app');
		await expect.element(page.getByRole('button', { name: 'Delete' })).toBeInTheDocument();
	});

	it('hides the delete form for a member role (fail-closed, R8)', async () => {
		const member: AccountMembership = { ...aliceOwner, role: 'member' };
		render(AccountDetailPage, { data: accountData(member), form: null });

		await expect.element(page.getByRole('button', { name: 'Delete' })).not.toBeInTheDocument();
	});

	it('hides the delete form for an unrecognized role (fail-closed, R8)', async () => {
		const steward: AccountMembership = { ...aliceOwner, role: 'steward' };
		render(AccountDetailPage, { data: accountData(steward), form: null });

		await expect.element(page.getByRole('button', { name: 'Delete' })).not.toBeInTheDocument();
	});

	it('renders the derived account_not_found problem in place of the account', async () => {
		render(AccountDetailPage, { data: problemData(notFoundProblem), form: null });

		await expect.element(page.getByTestId('problem')).toHaveTextContent(notFoundProblem.detail);
	});

	it('renders a form.problem at the top level even when the owner gate is closed', async () => {
		const member: AccountMembership = { ...aliceOwner, role: 'member' };
		render(AccountDetailPage, {
			data: accountData(member),
			form: { problem: forbiddenProblem }
		});

		await expect.element(page.getByTestId('problem')).toHaveTextContent(forbiddenProblem.detail);
		await expect.element(page.getByRole('button', { name: 'Delete' })).not.toBeInTheDocument();
	});
});
