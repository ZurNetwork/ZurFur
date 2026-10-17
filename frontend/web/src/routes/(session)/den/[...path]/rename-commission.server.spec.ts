import type { ActionFailure } from '@sveltejs/kit';
import { Effect, type Layer } from 'effect';
import { describe, expect, it, vi } from 'vitest';
import {
	COMMISSION_NOT_FOUND_PROBLEM,
	FORBIDDEN_PROBLEM,
	invalidFieldsProblem,
	type FieldError,
	type Problem
} from '$lib/api/problem';
import {
	RENAME_COMMISSION_FORBIDDEN_PROBLEM,
	RENAME_COMMISSION_UNANSWERED_PROBLEM
} from '$lib/server/commissions';
import { ApiProblem, NetworkFailure } from '$lib/server/api/errors';
import { zurfurApiTest, type ZurfurApi } from '$lib/server/api/zurfur-api';
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

/** Run the rename action from the page at `address`, with the hidden `id` and `title`. */
async function rename(address: string, id: string, title: string): Promise<unknown> {
	const url = new URL(`${address}?/renameCommission`, 'http://127.0.0.1:5174');
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

	it.each([
		['an empty id', ''],
		['an oversized id', 'x'.repeat(2049)],
		['an id the path builder refuses', '../accounts/x']
	])(
		'calls nothing for %s and shows the commission not-found as the form’s message',
		async (_, raw) => {
			const calls = api('renamed');

			const failure = failureOf(await rename(ADDRESS, raw, 'Winter fox'));

			expect(calls).toEqual([]);
			expect(failure.status).toBe(404);
			expect(failure.data.form.message).toEqual(COMMISSION_NOT_FOUND_PROBLEM);
		}
	);

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
		api(new ApiProblem({ problem: COMMISSION_NOT_FOUND_PROBLEM }));

		const failure = failureOf(await rename(ADDRESS, THING_ID, 'Winter fox'));

		expect(failure.status).toBe(404);
		expect(failure.data.form.message).toEqual(COMMISSION_NOT_FOUND_PROBLEM);
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
	});
});
