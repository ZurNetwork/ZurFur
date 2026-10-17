/**
 * Commission programs over the {@link import('./api/zurfur-api').ZurfurApi}
 * port. Creating one lands on its entry in the folder it was created from,
 * found by the thing the entry names in that folder's listing; the link is
 * the one the listing gives, never one built from the id. Pages run these
 * through {@link import('./runtime').runApi}.
 */

import { Effect } from 'effect';
import { NODE_NOT_FOUND_PROBLEM, serviceUnavailableProblem, type Problem } from '$lib/api/problem';
import { pageToken, type CommissionId, type DenHref, type PageToken } from '$lib/types/brand';
import type { DenRead, DenReadNode } from './api/den-read';
import type { ContractViolation } from './api/errors';
import { ZurfurApi } from './api/zurfur-api';
import { denHrefOf, denPathFromSegments, PLAIN_QUERY } from './den/den-path';
import type { SegmentPath } from './path-builder';

/** At most this many pages of the folder are read looking for a new commission's entry. */
export const COMMISSION_SEARCH_PAGES = 4;

/** What the form says when creating wasn't answered: it may have gone through. */
export const CREATE_COMMISSION_UNANSWERED_PROBLEM: Problem = serviceUnavailableProblem(
	"Zurfur didn't answer in time. Your commission may have been created: check your commissions before you try again."
);

/** How creating comes back: where to land, or a problem to show on the form. */
export type CreateCommissionOutcome = { readonly landing: DenHref } | { readonly problem: Problem };

/** Whether `node` is the open entry for the commission `id`. */
function isEntryFor(node: DenReadNode, id: CommissionId): boolean {
	if (node.access.access !== 'open') return false;
	const { thing } = node.access;
	return thing?.thing === 'commission' && thing.id === id;
}

/** One page of `folder`, or `undefined` when it doesn't read. */
function folderPage(
	folder: SegmentPath,
	token: PageToken | undefined
): Effect.Effect<DenRead | undefined, never, ZurfurApi> {
	const query = { includeDeleted: false, pageToken: token };
	return Effect.gen(function* () {
		const api = yield* ZurfurApi;
		return yield* folder.length === 0 ? api.denRoot(query) : api.denNode(folder, query);
	}).pipe(Effect.catchAll(() => Effect.succeed(undefined)));
}

/**
 * The link `folder`'s listing gives the commission `id`, reading at most
 * {@link COMMISSION_SEARCH_PAGES} pages; `undefined` when it isn't there,
 * the folder doesn't read, or the builder refuses the entry's path.
 */
function entryHrefIn(
	folder: SegmentPath,
	id: CommissionId
): Effect.Effect<DenHref | undefined, never, ZurfurApi> {
	return Effect.gen(function* () {
		let token: PageToken | undefined = undefined;
		for (let page = 0; page < COMMISSION_SEARCH_PAGES; page += 1) {
			const read: DenRead | undefined = yield* folderPage(folder, token);
			if (read?.content.content !== 'listing') return undefined;
			const entry = read.content.entries.find((node) => isEntryFor(node, id));
			if (entry !== undefined) {
				const path = denPathFromSegments(entry.segments);
				return path === undefined ? undefined : denHrefOf(path, PLAIN_QUERY);
			}
			const next: string | undefined = read.content.nextPageToken;
			token = next === undefined ? undefined : pageToken(next);
			if (token === undefined) return undefined;
		}
		return undefined;
	});
}

/**
 * Create a commission titled `title` from `folder`, then land on its entry
 * there, or on the folder when the entry isn't found. A refused title, or
 * any other problem, comes back as the problem; a backend that doesn't
 * answer is {@link CREATE_COMMISSION_UNANSWERED_PROBLEM}.
 */
export function createCommissionOutcome(
	folder: SegmentPath,
	title: string
): Effect.Effect<CreateCommissionOutcome, ContractViolation, ZurfurApi> {
	const folderHref = denHrefOf(folder, PLAIN_QUERY);
	if (folderHref === undefined) return Effect.succeed({ problem: NODE_NOT_FOUND_PROBLEM });
	const created = Effect.gen(function* () {
		const api = yield* ZurfurApi;
		const commission = yield* api.createCommission(title);
		const found = yield* entryHrefIn(folder, commission.id);
		return { landing: found ?? folderHref } satisfies CreateCommissionOutcome;
	});
	return created.pipe(
		Effect.catchTags({
			ApiProblem: ({ problem }) => Effect.succeed<CreateCommissionOutcome>({ problem }),
			NetworkFailure: () =>
				Effect.succeed<CreateCommissionOutcome>({ problem: CREATE_COMMISSION_UNANSWERED_PROBLEM })
		})
	);
}
