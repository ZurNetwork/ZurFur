import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import type { DenOutcome, DenPageData } from '$lib/api/den';
import { invalidRequestProblem, NODE_NOT_FOUND_PROBLEM, type Problem } from '$lib/api/problem';
import { denPage, type DenPageResult } from './den';
import { zurfurApiTest, type DenReadError, type ZurfurApiShape } from './api/zurfur-api';
import type { DenQuery, DenRead, DenReadNode } from './api/den-read';
import { ApiProblem, ContractViolation, DenNotConnected, NotAuthenticated } from './api/errors';
import type { SegmentPath } from './path-builder';

/** Every Den read the stubbed port saw: the path (empty for the root) and the query. */
interface SeenRead {
	readonly path: readonly string[];
	readonly query: DenQuery;
}

/** A port whose Den reads answer `answer(path)`, recording each read. */
function denPort(
	answer: (path: SegmentPath, query: DenQuery) => Effect.Effect<DenRead, DenReadError>
) {
	const seen: SeenRead[] = [];
	const read = (path: SegmentPath, query: DenQuery) => {
		seen.push({ path: [...path], query });
		return answer(path, query);
	};
	const shape: Partial<ZurfurApiShape> = {
		denRoot: (query) => read([], query),
		denNode: (path, query) => read(path, query)
	};
	return { layer: zurfurApiTest(shape), seen };
}

/** Run the den page program for `address` (path and query) over `layer`. */
async function run(
	layer: ReturnType<typeof zurfurApiTest>,
	address: string,
	withAncestors = false
): Promise<DenPageResult> {
	const url = new URL(address, 'http://127.0.0.1:5174');
	const program = denPage(url.pathname, url.searchParams, withAncestors).pipe(
		Effect.provide(layer)
	);
	return await Effect.runPromise(program);
}

/** The page data of a `page` result, or a thrown test failure. */
function pageData(result: DenPageResult): DenPageData {
	if (result.result !== 'page') throw new Error(`expected a page, got ${result.result}`);
	return result.data;
}

/** An open folder node at `segments`. */
function folder(
	segments: readonly string[],
	name: string,
	extra: Partial<DenReadNode> = {}
): DenReadNode {
	return {
		segments,
		name,
		type: 'folder',
		kind: 'directory',
		mount: false,
		access: { access: 'open', ownLevel: 'private', contentNotShown: false, removed: undefined },
		...extra
	};
}

/** A read of a folder at `segments` holding `entries`. */
function folderRead(
	segments: readonly string[],
	entries: readonly DenReadNode[],
	nextPageToken?: string
): DenRead {
	const crumbs = segments.map((_, index) => ({
		segments: segments.slice(0, index),
		name: index === 0 ? 'Alice' : (segments[index - 1] ?? '')
	}));
	return {
		node: folder(segments, segments.at(-1) ?? 'Alice'),
		crumbs,
		content: { content: 'listing', entries, nextPageToken }
	};
}

/** A port that answers every path with the folder listing `entries`. */
function listingPort(entries: readonly DenReadNode[] = [], nextPageToken?: string) {
	return denPort((path) => Effect.succeed(folderRead(path, entries, nextPageToken)));
}

/** A port that fails every read with `problem`. */
function problemPort(problem: Problem) {
	return denPort(() => Effect.fail(new ApiProblem({ problem })));
}

describe('denPage: refused addresses', () => {
	it.each(['/den/a%2Fb', '/den/%2e%2e', '/den/.%2e', '/den/a//b', '/den/%zz'])(
		'answers %s with the one not-found, without reading the Den',
		async (address) => {
			const { layer, seen } = listingPort();
			const data = pageData(await run(layer, address));
			expect(data.outcome).toEqual({ outcome: 'notFound' });
			expect(seen).toEqual([]);
		}
	);
});

describe('denPage: reading and mapping', () => {
	it('reads the root through denRoot and titles it My Den', async () => {
		const { layer, seen } = listingPort([folder(['accounts'], 'accounts')]);
		const data = pageData(await run(layer, '/den'));
		expect(seen).toEqual([{ path: [], query: { includeDeleted: false, pageToken: undefined } }]);
		expect(data.title).toBe('My Den · Zurfur');
		expect(data.trail).toEqual([{ step: 'root', href: undefined }]);
		expect(data.rootHref).toBe('/den');
	});

	it('builds every link through the path builder, carrying the flag but never a page token', async () => {
		const { layer } = listingPort(
			[folder(['commissions', 'c1'], 'Untitled', { mount: true })],
			'p1.25'
		);
		const data = pageData(await run(layer, '/den/commissions?includeDeleted=true&pageToken=p1.0'));
		const outcome = data.outcome;
		if (outcome.outcome !== 'page' || outcome.page.view !== 'open')
			throw new Error('expected an open page');
		const body = outcome.page.body;
		if (body.body !== 'listing') throw new Error('expected a listing');

		expect(outcome.page.node.href).toBe('/den/commissions?includeDeleted=true');
		expect(outcome.page.crumbs).toEqual([{ href: '/den?includeDeleted=true', name: 'Alice' }]);
		expect(body.entries[0]).toMatchObject({
			view: 'open',
			href: '/den/commissions/c1?includeDeleted=true',
			mounted: true
		});
		expect(body.more).toBe('/den/commissions?includeDeleted=true&pageToken=p1.25');
		expect(outcome.flagLinks).toEqual({
			on: '/den/commissions?includeDeleted=true',
			off: '/den/commissions'
		});
		expect(data.continued).toBe(true);
	});

	it('shows an entry whose path the builder refuses as a card, never as a link', async () => {
		const { layer } = listingPort([
			folder(['commissions', '..'], 'Sneaky'),
			folder(['commissions', 'a/b'], 'Slashed')
		]);
		const data = pageData(await run(layer, '/den/commissions'));
		const outcome = data.outcome;
		if (
			outcome.outcome !== 'page' ||
			outcome.page.view !== 'open' ||
			outcome.page.body.body !== 'listing'
		)
			throw new Error('expected a listing');

		expect(outcome.page.body.entries.map((entry) => entry.view)).toEqual(['card', 'card']);
	});

	it('folds unknown kinds, levels and soft-delete words to unknown', async () => {
		const odd = folder(['x', 'y'], 'Odd', {
			kind: 'portal',
			access: { access: 'open', ownLevel: 'secret', contentNotShown: false, removed: 'vanished' }
		});
		const { layer } = listingPort([odd]);
		const data = pageData(await run(layer, '/den/x'));
		const outcome = data.outcome;
		if (
			outcome.outcome !== 'page' ||
			outcome.page.view !== 'open' ||
			outcome.page.body.body !== 'listing'
		)
			throw new Error('expected a listing');

		expect(outcome.page.body.entries[0]).toMatchObject({
			kind: 'unknown',
			ownLevel: 'unknown',
			removed: 'unknown'
		});
	});

	it('shows an access value this build doesn’t know as a card, never as a link (entries and the page)', async () => {
		const unknownAccess = { access: 'teleport' } as unknown as DenReadNode['access'];
		const odd = folder(['x', 'odd'], 'Odd', { access: unknownAccess });
		const listing = listingPort([odd]);
		const listed = pageData(await run(listing.layer, '/den/x'));
		const { outcome } = listed;
		if (
			outcome.outcome !== 'page' ||
			outcome.page.view !== 'open' ||
			outcome.page.body.body !== 'listing'
		)
			throw new Error('expected a listing');
		expect(outcome.page.body.entries[0]?.view).toBe('card');

		const self: DenRead = { ...folderRead(['x', 'odd'], []), node: odd };
		const pageAnswer = pageData(await run(denPort(() => Effect.succeed(self)).layer, '/den/x/odd'));
		expect(pageAnswer.outcome).toMatchObject({ page: { view: 'card' } });
	});

	it('maps a card page to a card with no level, no kind and no body', async () => {
		const card: DenRead = {
			node: {
				segments: [],
				name: 'Some commission',
				type: 'commission',
				kind: '',
				mount: true,
				access: { access: 'card' }
			},
			crumbs: [
				{ segments: [], name: 'Alice' },
				{ segments: ['commissions'], name: 'commissions' }
			],
			content: { content: 'none' }
		};
		const { layer } = denPort(() => Effect.succeed(card));
		const data = pageData(await run(layer, '/den/commissions/c9'));

		expect(data.outcome).toMatchObject({
			outcome: 'page',
			page: { view: 'card', node: { view: 'card', name: 'Some commission', mounted: true } }
		});
		expect(data.title).toBe('My Den · Zurfur');
		expect(data.trail).toEqual([
			{ step: 'root', href: '/den' },
			{ step: 'named', label: 'commissions', href: '/den/commissions' },
			{ step: 'named', label: 'Some commission', href: undefined }
		]);
	});

	it('shows an unknown content case as "not shown", never as an empty folder', async () => {
		const read: DenRead = { ...folderRead(['x'], []), content: { content: 'none' } };
		const { layer } = denPort(() => Effect.succeed(read));
		const data = pageData(await run(layer, '/den/x'));

		expect(data.outcome).toMatchObject({ page: { body: { body: 'notShown' } } });
	});
});

describe('denPage: ancestors', () => {
	it('on a full page load, also reads the root and every folder above the node, first pages only', async () => {
		const { layer, seen } = listingPort();
		await run(layer, '/den/commissions/c1/attachments?includeDeleted=true&pageToken=p1.25', true);
		const paths = seen.map((read) => read.path.join('/')).toSorted();

		expect(paths).toEqual(['', 'commissions', 'commissions/c1', 'commissions/c1/attachments']);
		const ancestorQueries = seen.filter((read) => read.path.length < 3).map((read) => read.query);
		expect(
			ancestorQueries.every((query) => query.pageToken === undefined && query.includeDeleted)
		).toBe(true);
	});

	it('on a full page load of a later root page, also reads the root’s first page', async () => {
		const { layer, seen } = listingPort([folder(['accounts'], 'accounts')], 'p1.50');
		const data = pageData(await run(layer, '/den?pageToken=p1.25', true));

		expect(seen.map((read) => read.query.pageToken).toSorted()).toEqual(['p1.25', undefined]);
		expect(data.continued).toBe(true);
		expect(data.ancestors).toEqual([
			{
				folder: '/den',
				entries: [expect.objectContaining({ href: '/den/accounts' })],
				more: '/den?pageToken=p1.50'
			}
		]);
	});

	it('on a full page load of the root’s first page, reads the root once', async () => {
		const { layer, seen } = listingPort();
		const data = pageData(await run(layer, '/den', true));
		expect(seen).toHaveLength(1);
		expect(data.ancestors).toEqual([]);
	});

	it('on an in-app navigation, reads only the node', async () => {
		const { layer, seen } = listingPort();
		await run(layer, '/den/commissions/c1', false);
		expect(seen.map((read) => read.path)).toEqual([['commissions', 'c1']]);
	});

	it('leaves out an ancestor that fails to read', async () => {
		const { layer } = denPort((path) =>
			path.length === 0
				? Effect.fail(new ApiProblem({ problem: NODE_NOT_FOUND_PROBLEM }))
				: Effect.succeed(folderRead(path, []))
		);
		const data = pageData(await run(layer, '/den/a/b', true));
		expect(data.ancestors.map((listing) => listing.folder)).toEqual(['/den/a']);
	});
});

describe('denPage: failures', () => {
	it('answers notConnected while the live port has no Den', async () => {
		const { layer } = denPort(() => Effect.fail(new DenNotConnected()));
		const data = pageData(await run(layer, '/den'));
		expect(data.outcome).toEqual({ outcome: 'notConnected' });
	});

	it('gives a refused address and a backend miss the same title and path bar', async () => {
		const refused = pageData(await run(listingPort().layer, '/den/a%2Fb'));
		const missed = pageData(await run(problemPort(NODE_NOT_FOUND_PROBLEM).layer, '/den/nope'));
		expect([refused.outcome, refused.title, refused.trail]).toEqual([
			missed.outcome,
			missed.title,
			missed.trail
		]);
	});

	it('answers the one not-found for node_not_found', async () => {
		const data = pageData(await run(problemPort(NODE_NOT_FOUND_PROBLEM).layer, '/den/nope'));
		const notFound: DenOutcome = { outcome: 'notFound' };
		expect(data.outcome).toEqual(notFound);
		expect(data.trail).toEqual([{ step: 'root', href: '/den' }]);
		expect(data.title).toBe('My Den · Zurfur');
	});

	it('reloads a folder from its first page when its page token is refused (422)', async () => {
		const { layer } = problemPort(invalidRequestProblem('stale token'));
		const result = await run(layer, '/den/commissions?includeDeleted=true&pageToken=p9.stale');
		expect(result).toEqual({ result: 'reload', location: '/den/commissions?includeDeleted=true' });
	});

	it('fails as a contract error on a 422 with no page token, so a web bug cannot loop', async () => {
		const { layer } = problemPort(invalidRequestProblem('bad'));
		const program = denPage('/den/commissions', new URLSearchParams(), false).pipe(
			Effect.provide(layer),
			Effect.flip
		);
		const failure = await Effect.runPromise(program);
		expect(failure).toBeInstanceOf(ContractViolation);
		expect(failure.message).not.toContain('commissions');
	});

	it('fails as a contract error on a 400, naming only the route template', async () => {
		const badRequest: Problem = {
			type: 'urn:zurfur:error:bad-request',
			code: 'unknown_parameter',
			title: 'Bad request',
			detail: 'Unknown parameter.',
			status: 400
		};
		const program = denPage(
			'/den/accounts/did:plc:secretsecret',
			new URLSearchParams(),
			false
		).pipe(Effect.provide(problemPort(badRequest).layer), Effect.flip);
		const failure = await Effect.runPromise(program);
		expect(failure).toBeInstanceOf(ContractViolation);
		expect(failure.message).toContain('/den/{path}');
		expect(failure.message).not.toContain('secret');
	});

	it('sends an ended session to sign in', async () => {
		const problem: Problem = {
			type: 'urn:zurfur:error:not-authenticated',
			code: 'not_authenticated',
			title: 'Not authenticated',
			detail: 'Sign in.',
			status: 401
		};
		const { layer } = denPort(() => Effect.fail(new NotAuthenticated({ problem })));
		expect(await run(layer, '/den')).toEqual({ result: 'signedOut' });
	});

	it('hands any other problem to the pane', async () => {
		const problem: Problem = {
			type: 'urn:zurfur:error:rate-limited',
			code: 'rate_limited',
			title: 'Slow down',
			detail: 'Slow down.',
			status: 429
		};
		const data = pageData(await run(problemPort(problem).layer, '/den'));
		expect(data.outcome).toEqual({ outcome: 'problem', problem });
	});
});
