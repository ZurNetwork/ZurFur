import { describe, expect, it } from 'vitest';
import { fetchStub, problemResponse, unreachableFetch } from '$lib/testing/http';
import { expectRedirect } from '$lib/testing/redirect';
import { actions, load } from './+page.server';
import type { AccountMembership, DeleteOutcome } from '$lib/api/account';
import type { Problem } from '$lib/api/problem';

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<(typeof actions)['default']>[0];

/** `load`'s actual return, pinned past the generated type's `MaybeWithVoid` noise. */
type ListLoadData =
	| { accounts: ReadonlyArray<AccountMembership>; deleted?: DeleteOutcome }
	| { problem: Problem; deleted?: DeleteOutcome };

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
	return actions.default({ request, fetch } as unknown as ActionEvent);
}

describe('/accounts load', () => {
	it('returns the rows the stubbed api hands it (AC1)', async () => {
		const { fetch } = fetchStub(() => Response.json({ accounts: [aliceStudio] }));
		const result = await runLoad(loadEvent(fetch));
		expect(result).toEqual({ accounts: [aliceStudio], deleted: undefined });
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
	it('rejects an empty name and handle locally without reaching the backend', async () => {
		const failure = await createAction(unreachableFetch('must not reach the backend'), '', '');
		expect(failure).toMatchObject({
			status: 422,
			data: {
				problem: { code: 'invalid_request', title: 'Invalid values.' },
				name: '',
				handle: ''
			}
		});
	});

	it('rejects an empty handle locally, carrying the typed name back', async () => {
		const failure = await createAction(
			unreachableFetch('must not reach the backend'),
			'Alice Studio',
			''
		);
		expect(failure).toMatchObject({
			status: 422,
			data: {
				problem: { code: 'invalid_request' },
				name: 'Alice Studio',
				handle: ''
			}
		});
	});

	it('hands a backend problem to the page, carrying the typed values back', async () => {
		const { fetch } = fetchStub(() =>
			problemResponse(409, 'handle_taken', 'That handle is already claimed.')
		);
		const failure = await createAction(fetch, 'New Studio', 'taken.zurfur.app');
		expect(failure).toMatchObject({
			status: 409,
			data: {
				problem: { code: 'handle_taken' },
				name: 'New Studio',
				handle: 'taken.zurfur.app'
			}
		});
	});

	it('redirects to the listing on success', async () => {
		const created = {
			id: 'acct-new',
			did: 'did:plc:new',
			handle: 'new.zurfur.app',
			name: 'New Studio'
		};
		const { fetch } = fetchStub(() => Response.json(created, { status: 201 }));
		const redirect = await expectRedirect(() =>
			createAction(fetch, 'New Studio', 'new.zurfur.app')
		);
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/accounts');
	});
});
