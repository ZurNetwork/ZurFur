import type { ActionFailure } from '@sveltejs/kit';
import { Effect, Layer } from 'effect';
import { describe, expect, it, vi } from 'vitest';
import {
	COMMISSION_NOT_FOUND_PROBLEM,
	FORBIDDEN_PROBLEM,
	invalidFieldsProblem,
	invalidRequestProblem,
	type FieldError,
	type Problem
} from '$lib/api/problem';
import {
	RENAME_COMMISSION_FORBIDDEN_PROBLEM,
	RENAME_COMMISSION_NOT_FOUND_PROBLEM,
	RENAME_COMMISSION_UNANSWERED_PROBLEM,
	RENAME_NODE_NOT_FOUND_PROBLEM,
	renameCommissionOutcome
} from '$lib/server/commissions';
import { ApiProblem, NetworkFailure } from '$lib/server/api/errors';
import { createMockStore, zurfurApiMock } from '$lib/server/api/zurfur-api-mock';
import {
	RequestFetch,
	ZurfurApiLive,
	zurfurApiTest,
	type ZurfurApi
} from '$lib/server/api/zurfur-api';
import { fetchStub } from '$lib/testing/http';
import { expectRedirect } from '$lib/testing/redirect';
import { commissionId, fieldPointer, type CommissionId } from '$lib/types/brand';

/** The API this spec's `runApi` answers from. */
const seam = vi.hoisted((): { layer: Layer.Layer<ZurfurApi> | undefined } => ({
	layer: undefined
}));

vi.mock('$lib/server/runtime', () => ({
	runApi: <A, E>(_fetch: typeof globalThis.fetch, program: Effect.Effect<A, E, ZurfurApi>) => {
		const layer = seam.layer;
		if (layer === undefined) throw new Error('no API for this spec');
		return Effect.runPromise(program.pipe(Effect.provide(layer)));
	}
}));

const { actions } = await import('./+page.server');

type ActionEvent = Parameters<typeof actions.renameCommission>[0];

/** A minted id, or a thrown test failure. */
function idOf(raw: string): CommissionId {
	const id = commissionId(raw);
	if (id === undefined) throw new Error('bad test id');
	return id;
}

/** The commission's id: never its Den segment below. */
const THING_ID = idOf('0192f0c0-0000-7000-8000-00000000c0de');

/** The mock Den's fixture "Untitled": shown open, never created by the mock. */
const UNTITLED_ID = idOf('01a0ef9c-5b2e-7c41-9d3a-6f1e2b7c8d90');

/** The open commission's address, whose last segment is not its thing id. */
const ADDRESS = '/den/commissions/fox-in-the-snow';

/** One rename the action asked the API for. */
interface RenameCall {
	readonly id: CommissionId;
	readonly title: string;
}

/** The API for one case: a rename that answers `answer`; every call is recorded. */
function api(answer: 'renamed' | ApiProblem | NetworkFailure): RenameCall[] {
	const calls: RenameCall[] = [];
	const renameCommission = (id: CommissionId, title: string) =>
		Effect.suspend(() => {
			calls.push({ id, title });
			return answer === 'renamed' ? Effect.succeed({ id, title }) : Effect.fail(answer);
		});
	seam.layer = zurfurApiTest({ renameCommission });
	return calls;
}

/**
 * Run the rename action from the page at `address`, with the hidden `id` and
 * `title`; `query` follows the action's name in the posted URL.
 */
async function rename(address: string, id: string, title: string, query = ''): Promise<unknown> {
	const url = new URL(`${address}?/renameCommission${query}`, 'http://127.0.0.1:5174');
	const body = new FormData();
	body.set('id', id);
	body.set('title', title);
	const request = new Request(url, { method: 'POST', body });
	const { fetch } = fetchStub(() => Response.json({}));
	const event = { request, fetch, url } as unknown as ActionEvent;
	return actions.renameCommission(event);
}

/** The form an action failure carries back. */
interface FailedForm {
	readonly form: {
		readonly errors: { readonly title?: readonly string[] | undefined };
		readonly message?: Problem | undefined;
	};
}

/** The action's answer as the failure it must be. */
function failureOf(answer: unknown): ActionFailure<FailedForm> {
	return answer as ActionFailure<FailedForm>;
}

const TITLE_EMPTY: FieldError = {
	pointer: fieldPointer('#/title'),
	code: 'name_empty',
	detail: "This can't be blank."
};

describe('the renameCommission action', () => {
	it('renames the thing the hidden field names, never an id read from the address', async () => {
		const calls = api('renamed');

		await expectRedirect(() => rename(ADDRESS, THING_ID, 'Winter fox'));

		expect(calls).toEqual([{ id: THING_ID, title: 'Winter fox' }]);
	});

	it('goes back (303) to the page’s own address', async () => {
		api('renamed');

		const landing = await expectRedirect(() => rename(ADDRESS, THING_ID, 'Winter fox'));

		expect(landing.status).toBe(303);
		expect(landing.location).toBe(ADDRESS);
	});

	it('keeps "Show archived" on the way back when the post carries it', async () => {
		api('renamed');

		const landing = await expectRedirect(() =>
			rename(ADDRESS, THING_ID, 'Winter fox', '&includeDeleted=true')
		);

		expect(landing.status).toBe(303);
		expect(landing.location).toBe(`${ADDRESS}?includeDeleted=true`);
	});

	it.each([
		['a page token', '&pageToken=p1.25', ADDRESS],
		[
			'a page token beside the flag',
			'&includeDeleted=true&pageToken=p1.25',
			`${ADDRESS}?includeDeleted=true`
		],
		['a flag that isn’t exactly one true', '&includeDeleted=true&includeDeleted=true', ADDRESS]
	])('drops %s from the way back, keeping only the flag', async (_, query, expected) => {
		api('renamed');

		const landing = await expectRedirect(() => rename(ADDRESS, THING_ID, 'Winter fox', query));

		expect(landing.location).toBe(expected);
	});

	it.each([
		['an empty id', ''],
		['an oversized id', 'x'.repeat(2049)],
		['an id the path builder refuses', '../accounts/x']
	])('calls nothing for %s and shows "Nothing here" as the form’s message', async (_, raw) => {
		const calls = api('renamed');

		const failure = failureOf(await rename(ADDRESS, raw, 'Winter fox'));

		expect(calls).toEqual([]);
		expect(failure.status).toBe(404);
		expect(failure.data.form.message?.detail).toBe('Nothing here');
		expect(failure.data.form.message).toEqual(RENAME_COMMISSION_NOT_FOUND_PROBLEM);
		expect(failure.data.form.message?.code).toBe('commission_not_found');
	});

	it('shows "Nothing here" for the backend’s commission not-found, under no field', async () => {
		api(new ApiProblem({ problem: COMMISSION_NOT_FOUND_PROBLEM }));

		const failure = failureOf(await rename(ADDRESS, THING_ID, 'Winter fox'));

		expect(failure.status).toBe(404);
		expect(failure.data.form.message?.detail).toBe('Nothing here');
		expect(failure.data.form.message?.code).toBe('commission_not_found');
		expect(failure.data.form.errors.title).toBeUndefined();
	});

	it('shows the owner-only words for an open fixture of the mock Den', async () => {
		seam.layer = zurfurApiMock(createMockStore());

		const failure = failureOf(await rename('/den/commissions/untitled', UNTITLED_ID, 'Winter fox'));

		expect(failure.status).toBe(403);
		expect(failure.data.form.message?.detail).toBe("Only the commission's owner can rename it.");
	});

	it('shows the owner-only words when the backend refuses a Participant who isn’t the owner', async () => {
		api(new ApiProblem({ problem: FORBIDDEN_PROBLEM }));

		const failure = failureOf(await rename(ADDRESS, THING_ID, 'Winter fox'));

		expect(failure.status).toBe(403);
		expect(failure.data.form.message?.detail).toBe("Only the commission's owner can rename it.");
		expect(failure.data.form.message).toEqual(RENAME_COMMISSION_FORBIDDEN_PROBLEM);
		expect(failure.data.form.errors.title).toBeUndefined();
	});

	it('shows a refused title under its field, with no form message', async () => {
		api(new ApiProblem({ problem: invalidFieldsProblem([TITLE_EMPTY]) }));

		const failure = failureOf(await rename(ADDRESS, THING_ID, ' '));

		expect(failure.status).toBe(422);
		expect(failure.data.form.errors.title).toEqual(["This can't be blank."]);
		expect(failure.data.form.message).toBeUndefined();
	});

	it('shows any other problem as the form’s message, under no field', async () => {
		const other = invalidRequestProblem('That title is not one this API reads.');
		api(new ApiProblem({ problem: other }));

		const failure = failureOf(await rename(ADDRESS, THING_ID, 'Winter fox'));

		expect(failure.status).toBe(422);
		expect(failure.data.form.message).toEqual(other);
		expect(failure.data.form.errors.title).toBeUndefined();
	});

	it('says so when the rename isn’t answered', async () => {
		api(new NetworkFailure({ reason: 'timeout' }));

		const failure = failureOf(await rename(ADDRESS, THING_ID, 'Winter fox'));

		expect(failure.status).toBe(503);
		expect(failure.data.form.message).toEqual(RENAME_COMMISSION_UNANSWERED_PROBLEM);
	});

	it('renames nothing from an address the path builder refuses', async () => {
		const calls = api('renamed');

		const failure = failureOf(await rename('/den/a%2Fb', THING_ID, 'Winter fox'));

		expect(calls).toEqual([]);
		expect(failure.status).toBe(404);
		expect(failure.data.form.message?.code).toBe('node_not_found');
		expect(failure.data.form.message?.detail).toBe('Nothing here');
		expect(failure.data.form.message).toEqual(RENAME_NODE_NOT_FOUND_PROBLEM);
	});
});

describe('renameCommissionOutcome over the live API', () => {
	it('shows "Nothing here" for an id the path builder refuses, fetching nothing', async () => {
		const { fetch, calls } = fetchStub(() => Response.json({}));
		const live = ZurfurApiLive.pipe(Layer.provide(Layer.succeed(RequestFetch, fetch)));

		const outcome = await Effect.runPromise(
			renameCommissionOutcome(idOf('../accounts/x'), 'Winter fox').pipe(Effect.provide(live))
		);

		expect(calls).toEqual([]);
		expect(outcome).toEqual({ problem: RENAME_COMMISSION_NOT_FOUND_PROBLEM });
		expect('problem' in outcome && outcome.problem.detail).toBe('Nothing here');
	});
});
