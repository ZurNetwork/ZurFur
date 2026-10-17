import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { TREE_MEMORY_KEY, TreeMemory } from '$lib/components/explorer/tree-memory.svelte';
import type { AccountMembership } from '$lib/api/account';
import type { Problem } from '$lib/api/problem';
import { accountId, denHref, did, handleFromTrusted } from '$lib/types/brand';

/** Every action result the page hands SvelteKit to apply, instead of applying it. */
const forms = vi.hoisted(() => ({ applied: [] as unknown[] }));
vi.mock('$app/forms', async (importActual) => {
	const actual = await importActual<typeof import('$app/forms')>();
	return {
		...actual,
		applyAction: (result: unknown) => {
			forms.applied.push(result);
			return Promise.resolve();
		}
	};
});

const { default: AccountDetailPage } = await import('./+page.svelte');

const aliceOwner: AccountMembership = {
	id: accountId('acct-alice'),
	did: did('did:plc:alice'),
	handle: handleFromTrusted('alice.zurfur.app'),
	name: 'Alice Studio',
	role: 'owner'
};

const data = {
	session: {
		did: did('did:plc:alice'),
		handle: handleFromTrusted('alice.zurfur.app'),
		displayName: 'Alice',
		avatarUrl: undefined
	},
	account: aliceOwner,
	trail: [{ step: 'named' as const, label: 'Accounts', href: '/accounts' }]
};

/** What the delete action says when the DELETE wasn't answered (the server spec pins it). */
const MAY_HAVE_DELETED: Problem = {
	type: 'urn:zurfur:error:service-unavailable',
	code: 'service_unavailable',
	title: 'Service unavailable',
	detail:
		"Zurfur didn't answer in time. The Account may have been deleted: check your Accounts before you try again.",
	status: 503,
	errors: []
};

/** The page inside a frame whose tree memory knows `~` and `accounts`, its `form` as given. */
async function renderInFrame(form?: unknown): Promise<TreeMemory> {
	const memory = new TreeMemory(did('did:plc:alice'));
	memory.rootHref = denHref('/den');
	memory.expanded.add(denHref('/den'));
	memory.loaded(denHref('/den/accounts'), [], undefined);
	render(AccountDetailPage, {
		props: { data, form },
		context: new Map([[TREE_MEMORY_KEY, memory]])
	} as never);
	await expect.element(page.getByRole('button', { name: 'Delete' })).toBeInTheDocument();
	return memory;
}

/** Submit the confirmed delete, the action answering `body` with `status`. */
async function submitDelete(body: object, status = 200): Promise<void> {
	vi.spyOn(window, 'fetch').mockResolvedValue(Response.json(body, { status }));
	await page.getByLabelText('Type alice.zurfur.app to confirm').fill('alice.zurfur.app');
	await page.getByRole('button', { name: 'Delete' }).click();
}

afterEach(() => {
	vi.restoreAllMocks();
	forms.applied.length = 0;
});

describe('/accounts/[id]: the delete form, enhanced', () => {
	it('applies a failed delete in place, the tree left alone', async () => {
		const memory = await renderInFrame();

		await submitDelete({ type: 'failure', status: 503 });

		await expect.poll(() => forms.applied).toEqual([{ type: 'failure', status: 503 }]);
		expect(memory.folders.has(denHref('/den/accounts'))).toBe(true);
	});

	it('shows the “may have been deleted” words the failure carries', async () => {
		await renderInFrame({ problem: MAY_HAVE_DELETED });

		await expect.element(page.getByText(MAY_HAVE_DELETED.detail)).toBeInTheDocument();
	});

	it('hands SvelteKit the sign-in gate’s 503 error, so the frame shows its error view', async () => {
		await renderInFrame();

		await submitDelete(
			{ type: 'error', error: { message: "Zurfur isn't answering right now." } },
			503
		);

		await expect
			.poll(() => forms.applied)
			.toEqual([
				{ type: 'error', status: 503, error: { message: "Zurfur isn't answering right now." } }
			]);
	});

	it('starts the tree over, keeping ~ open, then moves on when the delete completes', async () => {
		const memory = await renderInFrame();

		await submitDelete({ type: 'redirect', status: 303, location: '/accounts?deleted=hard' });

		await expect
			.poll(() => forms.applied)
			.toEqual([{ type: 'redirect', status: 303, location: '/accounts?deleted=hard' }]);
		expect(memory.folders.has(denHref('/den/accounts'))).toBe(false);
		expect(memory.expanded.has(denHref('/den'))).toBe(true);
	});
});
