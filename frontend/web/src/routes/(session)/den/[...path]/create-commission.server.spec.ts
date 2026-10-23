import type { ActionFailure } from '@sveltejs/kit';
import { Effect, type Layer } from 'effect';
import { describe, expect, it, vi } from 'vitest';
import {
	FORBIDDEN_PROBLEM,
	invalidFieldsProblem,
	type FieldError,
	type Problem
} from '$lib/api/problem';
import { CREATE_COMMISSION_UNANSWERED_PROBLEM } from '$lib/server/commissions';
import type { DenQuery, DenRead, DenReadNode } from '$lib/server/api/den-read';
import { ApiProblem, NetworkFailure } from '$lib/server/api/errors';
import { zurfurApiTest, type ZurfurApi } from '$lib/server/api/zurfur-api';
import type { SegmentPath } from '$lib/server/path-builder';
import { fetchStub } from '$lib/testing/http';
import { expectRedirect } from '$lib/testing/redirect';
import { commissionId, did, fieldPointer, type CommissionId } from '$lib/types/brand';

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

type ActionEvent = Parameters<typeof actions.createCommission>[0];

/** A minted id, or a thrown test failure. */
function idOf(raw: string): CommissionId {
	const id = commissionId(raw);
	if (id === undefined) throw new Error('bad test id');
	return id;
}

/** The id the backend gives the new commission: never its Den segment below. */
const NEW_ID = idOf('0192f0c0-0000-7000-8000-00000000c0de');

/** An open commission entry in `commissions` at `segment`, standing for `id`. */
function commissionEntry(segment: string, id: CommissionId, name: string): DenReadNode {
	return {
		segments: ['commissions', segment],
		name,
		ordinal: undefined,
		type: 'commission',
		kind: 'directory',
		mount: true,
		access: {
			access: 'open',
			ownLevel: 'private',
			contentNotShown: false,
			removed: undefined,
			thing: { thing: 'commission', id }
		}
	};
}

/** An Account entry, which names a thing but never a commission. */
const ACCOUNT_ENTRY: DenReadNode = {
	segments: ['accounts', 'did:plc:studio'],
	name: 'Studio',
	ordinal: undefined,
	type: 'account',
	kind: 'directory',
	mount: true,
	access: {
		access: 'open',
		ownLevel: 'private',
		contentNotShown: false,
		removed: undefined,
		thing: { thing: 'account', did: did('did:plc:studio') }
	}
};

/** One page of the `commissions` folder holding `entries`, with `next` behind it. */
function commissionsPage(entries: readonly DenReadNode[], next?: string): DenRead {
	return {
		node: {
			segments: ['commissions'],
			name: 'commissions',
			ordinal: undefined,
			type: 'user.commissions',
			kind: 'directory',
			mount: false,
			access: {
				access: 'open',
				ownLevel: 'private',
				contentNotShown: false,
				removed: undefined,
				thing: undefined
			}
		},
		crumbs: [{ segments: [], name: 'Alice', ordinal: undefined, type: 'user' }],
		content: { content: 'listing', entries, nextPageToken: next }
	};
}

/** Every folder read the action made: the path and the page token. */
interface FolderRead {
	readonly path: readonly string[];
	readonly pageToken: string | undefined;
}

/**
 * The API for one case: a create that answers `created` (an id, or a
 * failure), and a folder whose pages are `pages`, each page after the first
 * reached by the token `p<n>`.
 */
function api(
	created: CommissionId | ApiProblem | NetworkFailure,
	pages: readonly (readonly DenReadNode[])[]
): FolderRead[] {
	const reads: FolderRead[] = [];
	const denNode = (path: SegmentPath, query: DenQuery) => {
		reads.push({ path: [...path], pageToken: query.pageToken });
		const index = query.pageToken === undefined ? 0 : Number(query.pageToken.slice(1)) - 1;
		const entries = pages[index] ?? [];
		const next = index + 1 < pages.length ? `p${String(index + 2)}` : undefined;
		return Effect.succeed(commissionsPage(entries, next));
	};
	const createCommission = (title: string) =>
		typeof created === 'string' ? Effect.succeed({ id: created, title }) : Effect.fail(created);
	seam.layer = zurfurApiTest({ createCommission, denNode });
	return reads;
}

/** Run the create action from the folder at `address`, titled `title`. */
async function create(address: string, title: string): Promise<unknown> {
	const url = new URL(`${address}?/createCommission`, 'http://127.0.0.1:5174');
	const body = new FormData();
	body.set('title', title);
	const request = new Request(url, { method: 'POST', body });
	const { fetch } = fetchStub(() => Response.json({}));
	const event = { request, fetch, url } as unknown as ActionEvent;
	return actions.createCommission(event);
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

describe('the createCommission action', () => {
	it('lands (303) on the href the listing gives the new entry, whose segment is not its id', async () => {
		api(NEW_ID, [
			[
				commissionEntry('untitled', idOf('c-other'), 'Untitled'),
				commissionEntry('fox-in-the-snow', NEW_ID, 'Fox in the snow')
			]
		]);

		const landing = await expectRedirect(() => create('/den/commissions', 'Fox in the snow'));

		expect(landing.status).toBe(303);
		expect(landing.location).toBe('/den/commissions/fox-in-the-snow');
	});

	it('reads the folder it was posted from, from its first page', async () => {
		const reads = api(NEW_ID, [[commissionEntry('fox', NEW_ID, 'Fox')]]);

		await expectRedirect(() => create('/den/commissions', 'Fox'));

		expect(reads).toEqual([{ path: ['commissions'], pageToken: undefined }]);
	});

	it('a_create_whose_entry_is_not_found_lands_on_the_folder', async () => {
		api(NEW_ID, [[commissionEntry('untitled', idOf('c-other'), 'Untitled'), ACCOUNT_ENTRY]]);

		const landing = await expectRedirect(() => create('/den/commissions', 'Fox in the snow'));

		expect(landing.status).toBe(303);
		expect(landing.location).toBe('/den/commissions');
	});

	it('follows later pages: an entry on page 4 is found', async () => {
		const reads = api(NEW_ID, [[], [], [], [commissionEntry('fox', NEW_ID, 'Fox')]]);

		const landing = await expectRedirect(() => create('/den/commissions', 'Fox'));

		expect(landing.location).toBe('/den/commissions/fox');
		expect(reads.map((read) => read.pageToken)).toEqual([undefined, 'p2', 'p3', 'p4']);
	});

	it('reads at most 4 pages: an entry on page 5 is not followed, and it lands on the folder', async () => {
		const reads = api(NEW_ID, [[], [], [], [], [commissionEntry('fox', NEW_ID, 'Fox')]]);

		const landing = await expectRedirect(() => create('/den/commissions', 'Fox'));

		expect(landing.location).toBe('/den/commissions');
		expect(reads).toHaveLength(4);
	});

	it('shows a refused title under its field, with no form message', async () => {
		api(new ApiProblem({ problem: invalidFieldsProblem([TITLE_EMPTY]) }), []);

		const failure = failureOf(await create('/den/commissions', ' '));

		expect(failure.status).toBe(422);
		expect(failure.data.form.errors.title).toEqual(["This can't be blank."]);
		expect(failure.data.form.message).toBeUndefined();
	});

	it('shows any other problem as the form’s message, under no field', async () => {
		api(new ApiProblem({ problem: FORBIDDEN_PROBLEM }), []);

		const failure = failureOf(await create('/den/commissions', 'Fox'));

		expect(failure.status).toBe(403);
		expect(failure.data.form.message).toEqual(FORBIDDEN_PROBLEM);
		expect(failure.data.form.errors.title).toBeUndefined();
	});

	it('says a create that wasn’t answered may have gone through', async () => {
		api(new NetworkFailure({ reason: 'timeout' }), []);

		const failure = failureOf(await create('/den/commissions', 'Fox'));

		expect(failure.status).toBe(503);
		expect(failure.data.form.message).toEqual(CREATE_COMMISSION_UNANSWERED_PROBLEM);
	});

	it('creates nothing from an address the path builder refuses', async () => {
		const reads = api(NEW_ID, []);
		let createdAny = false;
		seam.layer = zurfurApiTest({
			createCommission: (title) =>
				Effect.sync(() => {
					createdAny = true;
					return { id: NEW_ID, title };
				})
		});

		const failure = failureOf(await create('/den/a%2Fb', 'Fox'));

		expect(failure.status).toBe(404);
		expect(failure.data.form.message?.code).toBe('node_not_found');
		expect(createdAny).toBe(false);
		expect(reads).toEqual([]);
	});

	it('shows "Nothing here" as the form’s message for an address the path builder refuses', async () => {
		api(NEW_ID, []);

		const failure = failureOf(await create('/den/a%2Fb', 'Fox'));

		expect(failure.data.form.message?.detail).toBe('Nothing here');
		expect(failure.data.form.errors.title).toBeUndefined();
	});
});
