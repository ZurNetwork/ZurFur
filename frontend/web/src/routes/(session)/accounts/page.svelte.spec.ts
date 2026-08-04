import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import AccountsPage from './+page.svelte';
import type { AccountMembership, DeleteOutcome } from '$lib/api/account';
import type { Problem } from '$lib/api/problem';
import type { Session } from '$lib/api/session';
import { formStub } from '$lib/testing/superforms';

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

/** Thin wrapper over the shared {@link formStub}: this page's field defaults. */
function createForm(
	overrides: {
		name?: string;
		handle?: string;
		errors?: { name?: string[]; handle?: string[] };
		message?: Problem;
	} = {}
) {
	return formStub({ name: overrides.name ?? '', handle: overrides.handle ?? '' }, overrides);
}

function listingData(
	accounts: ReadonlyArray<AccountMembership>,
	deleted?: DeleteOutcome,
	form: ReturnType<typeof createForm> = createForm()
) {
	return { session: alice, accounts, deleted, form };
}

function problemData(problem: Problem, form: ReturnType<typeof createForm> = createForm()) {
	return { session: alice, problem, deleted: undefined, form };
}

describe('/accounts page', () => {
	it('renders one row per account with handle and role (AC1)', async () => {
		render(AccountsPage, { data: listingData([aliceStudio, bobCollective]) });

		const rows = page.getByRole('listitem');
		expect(rows.elements()).toHaveLength(2);
		await expect.element(rows.nth(0)).toHaveTextContent('alice.zurfur.app');
		await expect.element(rows.nth(0)).toHaveTextContent('owner');
		await expect.element(rows.nth(1)).toHaveTextContent('bob.zurfur.app');
		await expect.element(rows.nth(1)).toHaveTextContent('member');
	});

	it('renders the empty state with the create form present, no creator/onboarding prompt (AC1, AC4)', async () => {
		render(AccountsPage, { data: listingData([]) });

		await expect.element(page.getByTestId('accounts-empty')).toBeInTheDocument();
		await expect.element(page.getByLabelText('Name')).toBeInTheDocument();
		await expect.element(page.getByLabelText('Handle')).toBeInTheDocument();
		await expect.element(page.getByRole('button', { name: 'Found Account' })).toBeInTheDocument();
		await expect
			.element(page.getByText(/welcome|get started|onboarding|default account/i))
			.not.toBeInTheDocument();
	});

	it('renders local validation failures as per-field errors', async () => {
		render(AccountsPage, {
			data: listingData(
				[],
				undefined,
				createForm({
					errors: { name: ['Name cannot be empty'], handle: ['Handle cannot be empty'] }
				})
			)
		});

		await expect.element(page.getByTestId('name-error')).toHaveTextContent('Name cannot be empty');
		await expect
			.element(page.getByTestId('handle-error'))
			.toHaveTextContent('Handle cannot be empty');
	});

	it('re-fills the typed values from the form state after a failed submit', async () => {
		render(AccountsPage, {
			data: listingData(
				[],
				undefined,
				createForm({
					name: 'New Studio',
					handle: 'taken.zurfur.app',
					message: handleTakenProblem
				})
			)
		});

		await expect.element(page.getByLabelText('Name')).toHaveValue('New Studio');
		await expect.element(page.getByLabelText('Handle')).toHaveValue('taken.zurfur.app');
	});

	it('renders the handle_taken problem from the create action through the problem seam', async () => {
		render(AccountsPage, {
			data: listingData([], undefined, createForm({ message: handleTakenProblem }))
		});

		await expect.element(page.getByTestId('problem')).toHaveTextContent(handleTakenProblem.detail);
	});

	it.each([
		['soft', 'deactivated'],
		['hard', 'deleted'],
		['unknown', 'may still exist']
	] as const)('renders a distinguishable banner for ?deleted=%s', async (deleted, expectedText) => {
		render(AccountsPage, { data: listingData([], deleted) });

		await expect.element(page.getByText(expectedText, { exact: false })).toBeInTheDocument();
	});

	it('suppresses the ?deleted= flash after a failed submit (stale-flash guard)', async () => {
		render(AccountsPage, {
			data: listingData([], 'hard', createForm({ errors: { name: ['Name cannot be empty'] } }))
		});

		await expect.element(page.getByTestId('name-error')).toBeInTheDocument();
		await expect.element(page.getByText('The account was deleted.')).not.toBeInTheDocument();
	});

	it('renders no banner when deleted is not part of the vocabulary (garbage narrows to undefined)', async () => {
		render(AccountsPage, { data: listingData([]) });

		await expect
			.element(page.getByText('The account was', { exact: false }))
			.not.toBeInTheDocument();
	});

	it('scopes problem locators when data.problem and the form message can both render (caution case)', async () => {
		render(AccountsPage, {
			data: problemData(rateLimitedProblem, createForm({ message: handleTakenProblem }))
		});

		const problems = page.getByTestId('problem').elements();
		expect(problems).toHaveLength(2);
		const detailTexts = problems.map((element) => element.textContent?.trim());
		expect(detailTexts).toEqual(
			expect.arrayContaining([rateLimitedProblem.detail, handleTakenProblem.detail])
		);
	});
});
