import { page } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { DenPageData, OpenEntry } from '$lib/api/den';
import { FORBIDDEN_PROBLEM, type Problem } from '$lib/api/problem';
import { TREE_MEMORY_KEY, TreeMemory } from '$lib/components/explorer/tree-memory.svelte';
import { formStub } from '$lib/testing/superforms';
import { denHref, denName, denType, did } from '$lib/types/brand';

/** The options the page's "New commission" form handed superForm, captured to play a result through. */
const captured = vi.hoisted((): { onResult: unknown } => ({ onResult: undefined }));

vi.mock('sveltekit-superforms', async () => {
	const client = await import('sveltekit-superforms/client');
	return {
		superForm: (...args: Parameters<typeof client.superForm>) => {
			captured.onResult = args[1]?.onResult;
			return client.superForm(...args);
		}
	};
});

vi.mock('$app/navigation', async (importOriginal) => ({
	...(await importOriginal<typeof import('$app/navigation')>()),
	afterNavigate: () => undefined
}));

vi.mock('$app/state', () => ({ navigating: { to: undefined } }));

const { default: DenPage } = await import('./+page.svelte');

/** An open folder at `href` of type `type`. */
function folder(href: string, name: string, type: string): OpenEntry {
	return {
		view: 'open',
		href: denHref(href),
		name: denName(name),
		ordinal: undefined,
		type: denType(type),
		kind: 'directory',
		mounted: false,
		ownLevel: 'private',
		contentNotShown: false,
		removed: undefined
	};
}

/** The den page's data for `node`, an empty folder below the root. */
function folderData(node: OpenEntry): DenPageData {
	return {
		kind: 'denPage',
		address: node.href,
		outcome: {
			outcome: 'page',
			page: {
				view: 'open',
				node,
				thing: undefined,
				crumbs: [{ href: denHref('/den'), name: denName('Alice'), ordinal: undefined }],
				body: { body: 'listing', entries: [], more: undefined }
			},
			flagLinks: { on: node.href, off: node.href }
		},
		rootHref: denHref('/den'),
		includeDeleted: false,
		continued: false,
		ancestors: [],
		title: 'My Den · Zurfur',
		trail: []
	};
}

/** The viewer's own `commissions` folder. */
const USER_COMMISSIONS = folder('/den/commissions', 'commissions', 'user.commissions');

/** What the create form carries: pristine, or with a refused title or a problem. */
interface FormState {
	readonly errors?: { title: string[] };
	readonly message?: Problem;
}

/** The page for `node`, inside a frame whose tree memory is `memory`. */
function renderPage(node: OpenEntry, state: FormState = {}, memory?: TreeMemory) {
	const den = folderData(node);
	const createCommission = formStub({ title: '' }, state);
	const context = new Map([[TREE_MEMORY_KEY, memory ?? new TreeMemory(did('did:plc:alice'))]]);
	return render(DenPage, {
		props: { data: { den, trail: den.trail, createCommission } },
		context
	} as never);
}

/** Open the "New commission" dialog. */
async function openDialog(): Promise<void> {
	await page.getByRole('button', { name: 'New commission' }).click();
	await expect.element(page.getByRole('dialog', { name: 'New commission' })).toBeVisible();
}

/** Play an action result through the form's own onResult. */
function playResult(type: string): void {
	const onResult = captured.onResult;
	if (typeof onResult !== 'function') throw new Error('the form handed superForm no onResult');
	Reflect.apply(onResult, undefined, [
		{ result: { type, status: 303, location: '/den/commissions/fox' } }
	]);
}

describe('"New commission" in My Den', () => {
	it('shows on the viewer’s own commissions folder', async () => {
		renderPage(USER_COMMISSIONS);

		await expect.element(page.getByRole('button', { name: 'New commission' })).toBeVisible();
	});

	it.each([
		[
			"an Account's commissions folder",
			folder('/den/accounts/s/commissions', 'commissions', 'account.commissions')
		],
		['the characters folder', folder('/den/characters', 'characters', 'user.characters')],
		['a commission', folder('/den/commissions/fox', 'Fox', 'commission')]
	])('does not show on %s', async (_, node) => {
		renderPage(node);

		await expect.element(page.getByTestId('node-header')).toBeVisible();
		await expect
			.element(page.getByRole('button', { name: 'New commission' }))
			.not.toBeInTheDocument();
	});

	it('asks only for a title, and says new commissions are private as text, with no choice of level', async () => {
		renderPage(USER_COMMISSIONS);
		await openDialog();

		const dialog = page.getByRole('dialog', { name: 'New commission' });
		await expect.element(dialog.getByRole('textbox', { name: 'Title' })).toBeVisible();
		await expect.element(dialog.getByText('New commissions are private.')).toBeVisible();
		await expect.element(dialog.getByRole('button', { name: 'Create' })).toBeVisible();
		await expect.element(dialog.getByRole('button', { name: 'Cancel' })).toBeVisible();
		const controls = dialog.element().querySelectorAll('input, select, textarea');
		expect([...controls].map((control) => control.getAttribute('name'))).toEqual(['title']);
		for (const role of ['combobox', 'radio', 'checkbox', 'listbox', 'switch'] as const)
			expect(dialog.getByRole(role).elements()).toHaveLength(0);
		expect(page.getByTestId('new-commission-private').element().tagName).toBe('P');
	});

	it('ties a refused title’s message to the input by aria-describedby', async () => {
		renderPage(USER_COMMISSIONS, { errors: { title: ["This can't be blank."] } });
		await openDialog();

		const title = page.getByRole('textbox', { name: 'Title' });
		await expect.element(title).toHaveAccessibleDescription("This can't be blank.");
		await expect.element(title).toHaveAttribute('aria-invalid', 'true');
	});

	it('shows any other problem as the form’s message, under no field', async () => {
		renderPage(USER_COMMISSIONS, { message: FORBIDDEN_PROBLEM });
		await openDialog();

		await expect.element(page.getByTestId('problem')).toHaveTextContent(FORBIDDEN_PROBLEM.detail);
		await expect
			.element(page.getByRole('textbox', { name: 'Title' }))
			.not.toHaveAttribute('aria-describedby');
	});

	it('starts the tree over after a create redirects, keeping ~ open', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		memory.rootHref = denHref('/den');
		memory.expanded.add(denHref('/den'));
		memory.loaded(denHref('/den/commissions'), [], undefined);
		renderPage(USER_COMMISSIONS, {}, memory);
		await expect.element(page.getByRole('button', { name: 'New commission' })).toBeVisible();

		playResult('redirect');

		expect(memory.folders.has(denHref('/den/commissions'))).toBe(false);
		expect(memory.expanded.has(denHref('/den'))).toBe(true);
	});

	it('leaves the tree alone when a create fails', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		memory.rootHref = denHref('/den');
		memory.loaded(denHref('/den/commissions'), [], undefined);
		renderPage(USER_COMMISSIONS, {}, memory);
		await expect.element(page.getByRole('button', { name: 'New commission' })).toBeVisible();

		playResult('failure');

		expect(memory.folders.has(denHref('/den/commissions'))).toBe(true);
	});
});
