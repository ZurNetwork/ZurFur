import { describe, expect, it } from 'vitest';
import { fetchStub, problemResponse } from '$lib/testing/http';
import { expectRedirect } from '$lib/testing/redirect';
import { actions, load } from './+page.server';
import type { AccountMembership } from '$lib/api/account';
import type { Problem } from '$lib/api/problem';

type LoadEvent = Parameters<typeof load>[0];
type DeleteEvent = Parameters<(typeof actions)['delete']>[0];

/** `load`'s actual return, pinned past the generated type's `MaybeWithVoid` noise. */
type DetailLoadData = { account: AccountMembership } | { problem: Problem };

const aliceStudio: AccountMembership = {
	id: 'acct-alice',
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	name: 'Alice Studio',
	role: 'owner'
};

function loadEvent(fetch: typeof globalThis.fetch, id: string): LoadEvent {
	const event = { params: { id }, fetch };
	return event as unknown as LoadEvent;
}

async function runLoad(event: LoadEvent): Promise<DetailLoadData> {
	return (await load(event)) as DetailLoadData;
}

function deleteEvent(fetch: typeof globalThis.fetch, id: string, confirm?: string): DeleteEvent {
	const submitted = new FormData();
	if (confirm !== undefined) submitted.set('confirm', confirm);
	const request = new Request(`http://localhost/accounts/${id}?/delete`, {
		method: 'POST',
		body: submitted
	});
	const event = { params: { id }, fetch, request };
	return event as unknown as DeleteEvent;
}

/**
 * Routes the delete action's two backend calls: the listing read (the guard's
 * source of truth) answers `accounts`; the DELETE answers `onDelete`.
 */
function deleteFlowStub(accounts: AccountMembership[], onDelete: () => Response) {
	return fetchStub((url, init) =>
		init?.method === 'DELETE' ? onDelete() : Response.json({ accounts })
	);
}

describe('/accounts/[id] load', () => {
	it("derives the account from the caller's own listing (⚠️ F1 — no GET /accounts/{id})", async () => {
		const { fetch } = fetchStub(() => Response.json({ accounts: [aliceStudio] }));
		const result = await runLoad(loadEvent(fetch, 'acct-alice'));
		expect(result).toEqual({ account: aliceStudio });
	});

	it('answers the derived account_not_found problem for an id the caller holds no role in', async () => {
		const { fetch } = fetchStub(() => Response.json({ accounts: [aliceStudio] }));
		const result = await runLoad(loadEvent(fetch, 'acct-nobody-holds-a-role-in'));
		expect(result).toEqual({
			problem: {
				type: 'urn:zurfur:error:account-not-found',
				code: 'account_not_found',
				title: 'Account not found',
				detail: 'No such account.',
				status: 404
			}
		});
	});
});

describe('/accounts/[id] delete action', () => {
	it('redirects to the listing carrying the outcome when the exact handle confirms', async () => {
		const { fetch } = deleteFlowStub([aliceStudio], () => Response.json({ outcome: 'hard' }));
		const redirect = await expectRedirect(() =>
			actions.delete(deleteEvent(fetch, 'acct-alice', 'alice.zurfur.app'))
		);
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/accounts?deleted=hard');
	});

	it("redirects carrying the 'unknown' fallback outcome (R8) without treating it as a problem", async () => {
		const { fetch } = deleteFlowStub([aliceStudio], () =>
			Response.json({ outcome: 'quarantined' })
		);
		const redirect = await expectRedirect(() =>
			actions.delete(deleteEvent(fetch, 'acct-alice', 'alice.zurfur.app'))
		);
		expect(redirect.location).toBe('/accounts?deleted=unknown');
	});

	it('rejects a wrong confirm handle with field errors and NEVER issues the delete', async () => {
		const { fetch, calls } = deleteFlowStub([aliceStudio], () => {
			throw new Error('the delete must not be reached');
		});
		const failure = await actions.delete(deleteEvent(fetch, 'acct-alice', 'bob.zurfur.app'));
		expect(failure).toMatchObject({ status: 422, data: { form: { valid: false } } });
		expect(calls).toHaveLength(1);
	});

	it('rejects a non-Owner before the form or the delete (fail closed on role)', async () => {
		const aliceAsMember: AccountMembership = { ...aliceStudio, role: 'member' };
		const { fetch, calls } = deleteFlowStub([aliceAsMember], () => {
			throw new Error('the delete must not be reached');
		});
		const failure = await actions.delete(deleteEvent(fetch, 'acct-alice', 'alice.zurfur.app'));
		expect(failure).toMatchObject({ status: 403, data: { problem: { code: 'forbidden' } } });
		expect(calls).toHaveLength(1);
	});

	it('answers 403 (not a field error) for a non-Owner with a WRONG handle — authorization precedes validation', async () => {
		const aliceAsMember: AccountMembership = { ...aliceStudio, role: 'member' };
		const { fetch, calls } = deleteFlowStub([aliceAsMember], () => {
			throw new Error('the delete must not be reached');
		});
		const failure = await actions.delete(deleteEvent(fetch, 'acct-alice', 'wrong.zurfur.app'));
		expect(failure).toMatchObject({ status: 403, data: { problem: { code: 'forbidden' } } });
		expect(calls).toHaveLength(1);
	});

	it('answers the derived not-found and NEVER issues the delete for an id outside the caller listing', async () => {
		const { fetch, calls } = deleteFlowStub([aliceStudio], () => {
			throw new Error('the delete must not be reached');
		});
		const failure = await actions.delete(
			deleteEvent(fetch, 'acct-nobody-holds-a-role-in', 'alice.zurfur.app')
		);
		expect(failure).toMatchObject({
			status: 404,
			data: { problem: { code: 'account_not_found' } }
		});
		expect(calls).toHaveLength(1);
	});

	it('fails carrying the problem when the backend itself rejects the delete (stale Owner role)', async () => {
		const { fetch } = deleteFlowStub([aliceStudio], () => problemResponse(403, 'forbidden'));
		const failure = await actions.delete(deleteEvent(fetch, 'acct-alice', 'alice.zurfur.app'));
		expect(failure).toMatchObject({ status: 403, data: { problem: { code: 'forbidden' } } });
	});
});
