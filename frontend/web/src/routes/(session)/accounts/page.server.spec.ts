import { describe, expect, it } from 'vitest';
import { fetchStub, problemResponse, unreachableFetch } from '$lib/testing/http';
import { expectRedirect } from '$lib/testing/redirect';
import { actions, load } from './+page.server';
import type { AccountMembership, DeleteOutcome } from '$lib/api/account';
import type { Problem } from '$lib/api/problem';
import type { SuperValidated } from 'sveltekit-superforms';

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<(typeof actions)['default']>[0];

/** The action's fail payload: everything rides the superform (message = backend Problem). */
type CreateForm = SuperValidated<{ name: string; handle: string }, Problem>;

/** `load`'s actual return, pinned past the generated type's `MaybeWithVoid` noise. */
type ListLoadData = { deleted?: DeleteOutcome; form: CreateForm } & (
	{ accounts: ReadonlyArray<AccountMembership> } | { problem: Problem }
);

const aliceStudio: AccountMembership = {
	id: 'acct-alice',
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	name: 'Alice Studio',
	role: 'owner'
};

function loadEvent(fetch: typeof globalThis.fetch, search = ''): LoadEvent {
	const event = { fetch, url: new URL(`http://localhost/accounts${search}`) };
	return event as unknown as LoadEvent;
}

async function runLoad(event: LoadEvent): Promise<ListLoadData> {
	return (await load(event)) as ListLoadData;
}

async function createAction(
	fetch: typeof globalThis.fetch,
	name: string | null,
	handle: string | null
) {
	const body = new URLSearchParams();
	if (name !== null) body.set('name', name);
	if (handle !== null) body.set('handle', handle);
	const request = new Request('http://localhost/accounts', { method: 'POST', body });
	return (await actions.default({ request, fetch } as unknown as ActionEvent)) as {
		status: number;
		data: { form: CreateForm };
	};
}

describe('/accounts load', () => {
	it('returns the rows the stubbed api hands it, plus a pristine form (AC1)', async () => {
		const { fetch } = fetchStub(() => Response.json({ accounts: [aliceStudio] }));
		const result = await runLoad(loadEvent(fetch));
		expect(result).toMatchObject({ accounts: [aliceStudio] });
		expect(result.deleted).toBeUndefined();
		expect(result.form.posted).toBe(false);
		expect(result.form.message).toBeUndefined();
	});

	it.each([
		['soft', 'soft'],
		['hard', 'hard'],
		['unknown', 'unknown']
	] as const)('narrows a known ?deleted=%s through to the same value', async (param, expected) => {
		const { fetch } = fetchStub(() => Response.json({ accounts: [] }));
		const result = await runLoad(loadEvent(fetch, `?deleted=${param}`));
		expect(result.deleted).toBe(expected);
	});

	it('narrows an unrecognized ?deleted= value (garbage) to undefined', async () => {
		const { fetch } = fetchStub(() => Response.json({ accounts: [] }));
		const result = await runLoad(loadEvent(fetch, '?deleted=lol'));
		expect(result.deleted).toBeUndefined();
	});

	it('leaves deleted undefined when the param is absent', async () => {
		const { fetch } = fetchStub(() => Response.json({ accounts: [] }));
		const result = await runLoad(loadEvent(fetch));
		expect(result.deleted).toBeUndefined();
	});
});

describe('/accounts create action', () => {
	it('rejects an empty name and handle locally with per-field errors, never reaching the backend', async () => {
		const failure = await createAction(unreachableFetch('must not reach the backend'), '', '');
		expect(failure.status).toBe(422);
		expect(failure.data.form.valid).toBe(false);
		expect(failure.data.form.errors.name).toContain('Name cannot be empty');
		expect(failure.data.form.errors.handle).toContain('Handle cannot be empty');
	});

	it('rejects a punycode handle locally — the DD 26050561 claim site', async () => {
		const failure = await createAction(
			unreachableFetch('must not reach the backend'),
			'Sneaky Studio',
			'xn--sneaky.example'
		);
		expect(failure.status).toBe(422);
		expect(failure.data.form.errors.handle).toContain('Punycode (xn--) labels are not allowed');
	});

	it('rejects an empty handle locally, carrying the typed name back on the form', async () => {
		const failure = await createAction(
			unreachableFetch('must not reach the backend'),
			'Alice Studio',
			''
		);
		expect(failure.status).toBe(422);
		expect(failure.data.form.errors.handle).toContain('Handle cannot be empty');
		expect(failure.data.form.errors.name).toBeUndefined();
		expect(failure.data.form.data.name).toBe('Alice Studio');
	});

	it('hands a backend problem to the page as the form message, values riding the form', async () => {
		const { fetch } = fetchStub(() =>
			problemResponse(409, 'handle_taken', 'That handle is already claimed.')
		);
		const failure = await createAction(fetch, 'New Studio', 'taken.zurfur.app');
		expect(failure.status).toBe(409);
		expect(failure.data.form.message).toMatchObject({ code: 'handle_taken' });
		expect(failure.data.form.data).toMatchObject({
			name: 'New Studio',
			handle: 'taken.zurfur.app'
		});
	});

	it('redirects to the listing on success, sending TRIMMED values to the backend', async () => {
		const created = {
			id: 'acct-new',
			did: 'did:plc:new',
			handle: 'new.zurfur.app',
			name: 'New Studio'
		};
		let sentBody: unknown;
		const { fetch } = fetchStub((_url, init) => {
			sentBody = init?.body === undefined ? undefined : JSON.parse(String(init.body));
			return Response.json(created, { status: 201 });
		});
		const redirect = await expectRedirect(() =>
			createAction(fetch, '  New Studio  ', '  new.zurfur.app  ')
		);
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/accounts');
		expect(sentBody).toMatchObject({ name: 'New Studio', handle: 'new.zurfur.app' });
	});
});
