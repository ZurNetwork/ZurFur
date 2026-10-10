import { page } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { TREE_MEMORY_KEY, TreeMemory } from '$lib/components/explorer/tree-memory.svelte';
import { denHref, did } from '$lib/types/brand';
import { formStub } from '$lib/testing/superforms';

/** The options the Accounts page handed superForm, captured so the spec can play a result through them. */
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

const { default: AccountsPage } = await import('./+page.svelte');

/** The page's data: an empty listing and a pristine create form. */
const data = {
	session: {
		did: did('did:plc:alice'),
		handle: undefined,
		displayName: undefined,
		avatarUrl: undefined
	},
	denServed: true,
	accounts: [],
	deleted: undefined,
	form: formStub({ name: '', handle: '' }),
	trail: []
};

/** Play an action result through the page's own onResult. */
function playResult(type: string): void {
	const onResult = captured.onResult;
	if (typeof onResult !== 'function') throw new Error('the page handed superForm no onResult');
	Reflect.apply(onResult, undefined, [{ result: { type, status: 303, location: '/accounts' } }]);
}

/** The page rendered inside a frame whose tree memory knows `~` and `accounts`. */
async function renderInFrame(): Promise<TreeMemory> {
	const memory = new TreeMemory(did('did:plc:alice'));
	memory.rootHref = denHref('/den');
	memory.expanded.add(denHref('/den'));
	memory.loaded(denHref('/den/accounts'), [], undefined);
	render(AccountsPage, {
		props: { data },
		context: new Map([[TREE_MEMORY_KEY, memory]])
	} as never);
	await expect.element(page.getByRole('button', { name: 'Found Account' })).toBeInTheDocument();
	return memory;
}

describe('/accounts page and the frame’s tree', () => {
	it('starts the tree over after founding an Account succeeds, keeping ~ open', async () => {
		const memory = await renderInFrame();

		playResult('redirect');

		expect(memory.folders.has(denHref('/den/accounts'))).toBe(false);
		expect(memory.expanded.has(denHref('/den'))).toBe(true);
	});

	it('leaves the tree alone when founding fails', async () => {
		const memory = await renderInFrame();

		playResult('failure');

		expect(memory.folders.has(denHref('/den/accounts'))).toBe(true);
	});
});
