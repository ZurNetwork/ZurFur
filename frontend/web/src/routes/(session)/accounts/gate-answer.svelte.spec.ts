import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { formStub } from '$lib/testing/superforms';
import { did, handleFromTrusted } from '$lib/types/brand';

/** Every action result the page hands SvelteKit to show, instead of rendering it. */
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

const { default: AccountsPage } = await import('./+page.svelte');

const data = {
	session: {
		did: did('did:plc:alice'),
		handle: handleFromTrusted('alice.zurfur.app'),
		displayName: 'Alice',
		avatarUrl: undefined
	},
	accounts: [],
	deleted: undefined,
	form: formStub({ name: '', handle: '' }),
	trail: [{ step: 'named' as const, label: 'Accounts', ordinal: undefined, href: undefined }]
};

/** The sign-in gate's answer to an enhanced post when Zurfur isn't answering (its own spec pins it). */
const GATE_ANSWER = () =>
	Response.json(
		{ type: 'error', error: { message: "Zurfur isn't answering right now." } },
		{ status: 503 }
	);

afterEach(() => {
	vi.restoreAllMocks();
	forms.applied.length = 0;
});

describe('/accounts: founding when the sign-in gate says Zurfur isn’t answering', () => {
	it('hands SvelteKit the gate’s 503 error, so the frame shows its error view', async () => {
		vi.spyOn(window, 'fetch').mockResolvedValue(GATE_ANSWER());
		render(AccountsPage, { data });

		await page.getByLabelText('Name').fill('New Studio');
		await page.getByLabelText('Handle').fill('new.zurfur.app');
		await page.getByRole('button', { name: 'Found Account' }).click();

		await expect
			.poll(() => forms.applied)
			.toEqual([
				{ type: 'error', status: 503, error: { message: "Zurfur isn't answering right now." } }
			]);
	});
});
