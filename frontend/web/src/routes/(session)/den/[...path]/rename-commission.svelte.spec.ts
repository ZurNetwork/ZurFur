import { page } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { DenPage, DenPageData, DenThing, OpenEntry } from '$lib/api/den';
import { type Problem } from '$lib/api/problem';
import { TREE_MEMORY_KEY, TreeMemory } from '$lib/components/explorer/tree-memory.svelte';
import { formStub } from '$lib/testing/superforms';
import {
	commissionId,
	denHref,
	denName,
	denType,
	did,
	nameOrdinal,
	type CommissionId
} from '$lib/types/brand';

/** The options the page's last form handed superForm, captured to play a result through. */
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

/** A minted id, or a thrown test failure. */
function idOf(raw: string): CommissionId {
	const id = commissionId(raw);
	if (id === undefined) throw new Error('bad test id');
	return id;
}

/** The commission's id: never its Den segment. */
const THING_ID = idOf('0192f0c0-0000-7000-8000-00000000c0de');

/** An open node at `href` of type `type`. */
function node(href: string, name: string, type: string): OpenEntry {
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

/** The den page's data for `page`, an answer below the root. */
function pageData(denPage: DenPage, address: string): DenPageData {
	return {
		kind: 'denPage',
		address: denHref(address),
		outcome: {
			outcome: 'page',
			page: denPage,
			flagLinks: { on: denHref(address), off: denHref(address) }
		},
		rootHref: denHref('/den'),
		includeDeleted: false,
		continued: false,
		ancestors: [],
		title: 'My Den · Zurfur',
		trail: []
	};
}

/** The open page for `entry`, standing for `thing`. */
function openPage(entry: OpenEntry, thing: DenThing | undefined): DenPage {
	return {
		view: 'open',
		node: entry,
		thing,
		crumbs: [{ href: denHref('/den'), name: denName('Alice'), ordinal: undefined }],
		body: { body: 'listing', entries: [], more: undefined }
	};
}

/** An open commission whose segment is not its id. */
const COMMISSION = node('/den/commissions/fox-in-the-snow', 'Fox in the snow', 'commission');

/** The page for the open commission. */
const COMMISSION_PAGE = pageData(
	openPage(COMMISSION, { thing: 'commission', id: THING_ID }),
	COMMISSION.href
);

/** What the rename form carries: pristine, or with a refused title or a problem. */
interface FormState {
	readonly errors?: { title: string[] };
	readonly message?: Problem;
}

/** The page for `den`, inside a frame whose tree memory is `memory`. */
function renderPage(den: DenPageData, state: FormState = {}, memory?: TreeMemory) {
	const createCommission = formStub({ title: '' });
	const renameCommission = formStub({ id: '', title: '' }, state);
	const context = new Map([[TREE_MEMORY_KEY, memory ?? new TreeMemory(did('did:plc:alice'))]]);
	return render(DenPage, {
		props: { data: { den, trail: den.trail, createCommission, renameCommission } },
		context
	} as never);
}

/** Open the "Rename" dialog. */
async function openDialog(): Promise<void> {
	await page.getByRole('button', { name: 'Rename' }).click();
	await expect.element(page.getByRole('dialog', { name: 'Rename' })).toBeVisible();
}

/** Play an action result through the form's own onResult. */
function playResult(type: string): void {
	const onResult = captured.onResult;
	if (typeof onResult !== 'function') throw new Error('the form handed superForm no onResult');
	Reflect.apply(onResult, undefined, [
		{ result: { type, status: 303, location: COMMISSION.href } }
	]);
}

describe('"Rename" in My Den', () => {
	it('shows on an open commission', async () => {
		renderPage(COMMISSION_PAGE);

		await expect.element(page.getByRole('button', { name: 'Rename' })).toBeVisible();
	});

	it.each([
		[
			'a folder',
			pageData(
				openPage(node('/den/commissions', 'commissions', 'user.commissions'), undefined),
				'/den/commissions'
			)
		],
		[
			'an Account',
			pageData(
				openPage(node('/den/accounts/did:plc:s', 'Studio', 'account'), undefined),
				'/den/accounts/did:plc:s'
			)
		],
		[
			'a commission-typed node that names no commission',
			pageData(openPage(COMMISSION, undefined), COMMISSION.href)
		],
		[
			'a card',
			pageData(
				{
					view: 'card',
					node: {
						view: 'card',
						name: denName('Hidden'),
						ordinal: undefined,
						type: denType('commission'),
						mounted: true
					},
					crumbs: [{ href: denHref('/den'), name: denName('Alice'), ordinal: undefined }]
				},
				'/den/commissions/hidden'
			)
		]
	])('does not show on %s', async (_, den) => {
		renderPage(den);

		await expect.element(page.getByTestId('node-header')).toBeVisible();
		await expect.element(page.getByRole('button', { name: 'Rename' })).not.toBeInTheDocument();
	});

	it('starts_with_the_name_not_the_shown_name', async () => {
		const twin: OpenEntry = {
			...node(COMMISSION.href, 'Test 1', 'commission'),
			ordinal: nameOrdinal(2)
		};
		renderPage(pageData(openPage(twin, { thing: 'commission', id: THING_ID }), twin.href));
		await expect.element(page.getByRole('heading', { name: 'Test 1 (2)' })).toBeInTheDocument();

		await openDialog();

		const title = page
			.getByRole('dialog', { name: 'Rename' })
			.getByRole('textbox', { name: 'Title' });
		await expect.element(title).toHaveValue('Test 1');
	});

	it('starts the Title field from the node’s name, carrying the thing’s id, with Save and Cancel', async () => {
		renderPage(COMMISSION_PAGE);
		await openDialog();

		const dialog = page.getByRole('dialog', { name: 'Rename' });
		await expect
			.element(dialog.getByRole('textbox', { name: 'Title' }))
			.toHaveValue('Fox in the snow');
		const hidden = dialog.element().querySelector('input[type="hidden"][name="id"]');
		expect(hidden?.getAttribute('value')).toBe(THING_ID);
		await expect.element(dialog.getByRole('button', { name: 'Save' })).toBeVisible();
		await expect.element(dialog.getByRole('button', { name: 'Cancel' })).toBeVisible();
	});

	it('ties a refused title’s message to the input by aria-describedby', async () => {
		renderPage(COMMISSION_PAGE, { errors: { title: ["This can't be blank."] } });
		await openDialog();

		const title = page.getByRole('textbox', { name: 'Title' });
		await expect.element(title).toHaveAccessibleDescription("This can't be blank.");
		await expect.element(title).toHaveAttribute('aria-invalid', 'true');
	});

	it('announces a refused title’s message as an alert', async () => {
		renderPage(COMMISSION_PAGE, { errors: { title: ["This can't be blank."] } });
		await openDialog();

		await expect.element(page.getByTestId('rename-title-error')).toHaveAttribute('role', 'alert');
	});

	it.each([
		['shows archived entries', true, '?/renameCommission&includeDeleted=true'],
		['hides them', false, '?/renameCommission']
	])(
		'posts with the page’s "Show archived" flag only when it %s',
		async (_, includeDeleted, action) => {
			renderPage({ ...COMMISSION_PAGE, includeDeleted });
			await openDialog();

			const form = page.getByRole('dialog', { name: 'Rename' }).element().querySelector('form');
			expect(form?.getAttribute('action')).toBe(action);
		}
	);

	it('clears the form when the dialog closes, so it opens again from the name', async () => {
		renderPage(COMMISSION_PAGE, { errors: { title: ["This can't be blank."] } });
		await openDialog();
		await page.getByRole('textbox', { name: 'Title' }).fill('Half-typed');

		await page.getByRole('button', { name: 'Cancel' }).click();
		await openDialog();

		const title = page.getByRole('textbox', { name: 'Title' });
		await expect.element(title).toHaveValue('Fox in the snow');
		await expect.element(title).not.toHaveAttribute('aria-describedby');
	});

	it('shows any other problem as the form’s message, under no field', async () => {
		const owner: Problem = {
			type: 'urn:zurfur:error:forbidden',
			code: 'forbidden',
			title: 'Forbidden',
			detail: "Only the commission's owner can rename it.",
			status: 403,
			errors: []
		};
		renderPage(COMMISSION_PAGE, { message: owner });
		await openDialog();

		await expect
			.element(page.getByTestId('problem'))
			.toHaveTextContent("Only the commission's owner can rename it.");
		await expect
			.element(page.getByRole('textbox', { name: 'Title' }))
			.not.toHaveAttribute('aria-describedby');
	});

	it('starts the tree over after a rename redirects, keeping ~ open', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		memory.rootHref = denHref('/den');
		memory.expanded.add(denHref('/den'));
		memory.loaded(denHref('/den/commissions'), [], undefined);
		renderPage(COMMISSION_PAGE, {}, memory);
		await expect.element(page.getByRole('button', { name: 'Rename' })).toBeVisible();

		playResult('redirect');

		expect(memory.folders.has(denHref('/den/commissions'))).toBe(false);
		expect(memory.expanded.has(denHref('/den'))).toBe(true);
	});

	it('leaves the tree alone when a rename fails', async () => {
		const memory = new TreeMemory(did('did:plc:alice'));
		memory.rootHref = denHref('/den');
		memory.loaded(denHref('/den/commissions'), [], undefined);
		renderPage(COMMISSION_PAGE, {}, memory);
		await expect.element(page.getByRole('button', { name: 'Rename' })).toBeVisible();

		playResult('failure');

		expect(memory.folders.has(denHref('/den/commissions'))).toBe(true);
	});
});
