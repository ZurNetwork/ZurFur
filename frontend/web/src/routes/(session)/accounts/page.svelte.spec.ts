import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import AccountsPage from './+page.svelte';
import type { AccountMembership, DeleteOutcome } from '$lib/api/account';
import type { Problem } from '$lib/api/problem';
import type { Session } from '$lib/api/session';

/** Every accounts-group page requires a session (the layout gate) — the group's layout passes it through untouched, so every render here carries one. */
const alice: Session = {
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	displayName: 'Alice',
	avatarUrl: undefined
};

const aliceStudio: AccountMembership = {
	id: 'acct-alice',
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	name: 'Alice Studio',
	role: 'owner'
};

const bobCollective: AccountMembership = {
	id: 'acct-bob',
	did: 'did:plc:bob',
	handle: 'bob.zurfur.app',
	name: 'Bob Collective',
	role: 'member'
};

const rateLimitedProblem: Problem = {
	type: 'urn:zurfur:error:rate-limited',
	code: 'rate_limited',
	title: 'rate_limited',
	detail: 'Slow down.',
	status: 429
};

const handleTakenProblem: Problem = {
	type: 'urn:zurfur:error:handle-taken',
	code: 'handle_taken',
	title: 'handle_taken',
	detail: 'That handle is already claimed.',
	status: 409
};

function listingData(accounts: ReadonlyArray<AccountMembership>, deleted?: DeleteOutcome) {
	return { session: alice, accounts, deleted };
}

function problemData(problem: Problem, deleted?: DeleteOutcome) {
	return { session: alice, problem, deleted };
}

describe('/accounts page', () => {
	it('renders one row per account with handle and role (AC1)', async () => {
		render(AccountsPage, { data: listingData([aliceStudio, bobCollective]), form: null });

		const rows = page.getByRole('listitem');
		expect(rows.elements()).toHaveLength(2);
		await expect.element(rows.nth(0)).toHaveTextContent('alice.zurfur.app');
		await expect.element(rows.nth(0)).toHaveTextContent('owner');
		await expect.element(rows.nth(1)).toHaveTextContent('bob.zurfur.app');
		await expect.element(rows.nth(1)).toHaveTextContent('member');
	});

	it('renders the empty state with the create form present, no creator/onboarding prompt (AC1, AC4)', async () => {
		render(AccountsPage, { data: listingData([]), form: null });

		await expect.element(page.getByTestId('accounts-empty')).toBeInTheDocument();
		await expect.element(page.getByLabelText('Name')).toBeInTheDocument();
		await expect.element(page.getByLabelText('Handle')).toBeInTheDocument();
		await expect.element(page.getByRole('button', { name: 'Found Account' })).toBeInTheDocument();
		await expect
			.element(page.getByText(/welcome|get started|onboarding|default account/i))
			.not.toBeInTheDocument();
	});

	it('renders the handle_taken problem from the create action through the problem seam', async () => {
		render(AccountsPage, {
			data: listingData([]),
			form: { problem: handleTakenProblem, name: 'New Studio', handle: 'taken.zurfur.app' }
		});

		await expect.element(page.getByTestId('problem')).toHaveTextContent(handleTakenProblem.detail);
	});

	it.each([
		['soft', 'deactivated'],
		['hard', 'deleted'],
		['unknown', 'may still exist']
	] as const)('renders a distinguishable banner for ?deleted=%s', async (deleted, expectedText) => {
		render(AccountsPage, { data: listingData([], deleted), form: null });

		await expect.element(page.getByText(expectedText, { exact: false })).toBeInTheDocument();
	});

	it('renders no banner when deleted is not part of the vocabulary (garbage narrows to undefined)', async () => {
		render(AccountsPage, { data: listingData([]), form: null });

		await expect
			.element(page.getByText('The account was', { exact: false }))
			.not.toBeInTheDocument();
	});

	it('scopes problem locators when data.problem and form.problem can both render (caution case)', async () => {
		render(AccountsPage, {
			data: problemData(rateLimitedProblem),
			form: { problem: handleTakenProblem, name: '', handle: 'taken.zurfur.app' }
		});

		const problems = page.getByTestId('problem').elements();
		expect(problems).toHaveLength(2);
		const detailTexts = problems.map((element) => element.textContent?.trim());
		expect(detailTexts).toEqual(
			expect.arrayContaining([rateLimitedProblem.detail, handleTakenProblem.detail])
		);
	});
});
