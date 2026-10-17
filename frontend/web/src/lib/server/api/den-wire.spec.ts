// The tolerant reader, fail closed: every fallback den.proto states, driven
// through the live port's decode and the den program, so each one is proven
// where the page reads it.
import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import { cardTypeWord, type DenEntry, type DenPage } from '$lib/api/den';
import { fetchStub } from '$lib/testing/http';
import { denPage, type DenPageResult } from '../den';
import { decodeContract, RequestFetch, ZurfurApiLive } from './zurfur-api';
import type { ContractViolation, NetworkFailure } from './errors';
import type { JsonObject, JsonValue } from '@bufbuild/protobuf';
import { commissionId, did } from '$lib/types/brand';
import { denReadFromNode } from './den-wire';
import type { DenRead, DenReadAccess, DenReadNode } from './den-read';
import { create } from '@bufbuild/protobuf';
import { GetDenNodeResponseSchema } from './generated/zurfur/api/v1/den_pb';

/** Run the den page program for `address` over the live port, every read answered by `body`. */
function runPage(
	address: string,
	respond: (url: string) => Response,
	withAncestors = false
): Promise<DenPageResult | NetworkFailure | ContractViolation> {
	const url = new URL(address, 'http://127.0.0.1:5174');
	const { fetch } = fetchStub(respond);
	const program = denPage(url.pathname, url.searchParams, withAncestors).pipe(
		Effect.either,
		Effect.map((either) => (either._tag === 'Right' ? either.right : either.left)),
		Effect.provide(ZurfurApiLive),
		Effect.provideService(RequestFetch, fetch)
	);
	return Effect.runPromise(program);
}

/** The page a body renders as, or a thrown test failure. */
async function pageOf(address: string, body: unknown): Promise<DenPage> {
	const result = await runPage(address, () => Response.json(body));
	if (!('result' in result) || result.result !== 'page')
		throw new Error(`expected a page, got ${JSON.stringify(result)}`);
	const outcome = result.data.outcome;
	if (outcome.outcome !== 'page') throw new Error(`expected a page, got ${outcome.outcome}`);
	return outcome.page;
}

/** The listing entries of an open page, or a thrown test failure. */
function entriesOf(page: DenPage): readonly DenEntry[] {
	if (page.view !== 'open' || page.body.body !== 'listing') throw new Error('expected a listing');
	return page.body.entries;
}

const openNode = {
	segments: ['commissions'],
	name: 'commissions',
	type: 'user.commissions',
	kind: 'directory',
	open: { ownLevel: 'private' }
};

/** A folder answer at `/den/commissions` holding `entries`. */
function folderBody(entries: readonly unknown[], extra: object = {}): object {
	return { node: openNode, crumbs: [{ name: 'Alice' }], listing: { entries }, ...extra };
}

const entry = {
	segments: ['commissions', 'c1'],
	name: 'Untitled',
	type: 'commission',
	kind: 'directory',
	open: { ownLevel: 'private' }
};

describe('the tolerant reader: unknown fields', () => {
	it('ignores an unknown field at the top level and inside a node', async () => {
		const body = folderBody([{ ...entry, fromTheFuture: 1, open: { ownLevel: 'private', x: 2 } }], {
			aFieldFromTheFuture: 'later'
		});
		const [first] = entriesOf(await pageOf('/den/commissions', body));
		expect(first).toMatchObject({ view: 'open', name: 'Untitled', ownLevel: 'private' });
	});
});

describe('the tolerant reader: unknown words', () => {
	it('gives an unknown kind a generic marker, an unknown level no level and an unknown soft-delete a generic mark', async () => {
		const odd = { ...entry, kind: 'portal', open: { ownLevel: 'secret', softDelete: 'shelved' } };
		const [first] = entriesOf(await pageOf('/den/commissions', folderBody([odd])));
		expect(first).toMatchObject({
			view: 'open',
			kind: 'unknown',
			ownLevel: 'unknown',
			removed: 'unknown'
		});
	});

	it('marks no soft-delete for an empty one', async () => {
		const [first] = entriesOf(await pageOf('/den/commissions', folderBody([entry])));
		expect(first).toMatchObject({ view: 'open', removed: undefined });
	});

	it('shows no type word on a card of a type this build doesn’t know', async () => {
		const card = { name: 'Locked', type: 'account.characters', card: {} };
		const [first] = entriesOf(await pageOf('/den/commissions', folderBody([card])));
		expect(first?.view).toBe('card');
		if (first?.view === 'card') expect(cardTypeWord(first.type)).toBeUndefined();
	});
});

describe('the tolerant reader: access fails closed', () => {
	it.each([
		['unset', { name: 'No access', type: 'commission', segments: ['commissions', 'c2'] }],
		[
			'null',
			{ name: 'Null access', type: 'commission', segments: ['commissions', 'c2'], open: null }
		]
	])('shows an %s access as a card with no link', async (_, node) => {
		const [first] = entriesOf(await pageOf('/den/commissions', folderBody([node])));
		expect(first).toEqual({ view: 'card', name: node.name, type: 'commission', mounted: false });
	});

	it('shows a node page whose access is unset as a card page', async () => {
		const body = { node: { name: 'No access', type: 'commission' }, crumbs: [{ name: 'Alice' }] };
		const page = await pageOf('/den/commissions/c2', body);
		expect(page.view).toBe('card');
	});

	it.each([
		['card first', { card: {}, open: { ownLevel: 'public' } }],
		['open first', { open: { ownLevel: 'public' }, card: {} }]
	])('never opens a node with both access members set (%s)', async (_, access) => {
		const result = await runPage('/den/commissions', () =>
			Response.json(folderBody([{ ...entry, ...access }]))
		);
		expect('_tag' in result && result._tag).toBe('ContractViolation');
	});
});

describe('the tolerant reader: content', () => {
	it('shows an unknown content case as "content not shown yet", never an empty folder', async () => {
		const body = { node: openNode, crumbs: [{ name: 'Alice' }], gallery: { items: [] } };
		const page = await pageOf('/den/commissions', body);
		expect(page.view === 'open' && page.body).toEqual({ body: 'notShown' });
	});

	it('shows an open node with no content as "content not shown yet"', async () => {
		const page = await pageOf('/den/commissions', { node: openNode, crumbs: [{ name: 'Alice' }] });
		expect(page.view === 'open' && page.body).toEqual({ body: 'notShown' });
	});

	it('reads an empty page token as the last page', async () => {
		const body = folderBody([entry]);
		const withToken = { ...body, listing: { entries: [entry], nextPageToken: '' } };
		const page = await pageOf('/den/commissions', withToken);
		expect(page.view === 'open' && page.body.body === 'listing' && page.body.more).toBeUndefined();
	});

	it('links a listing’s next page through its page token', async () => {
		const body = folderBody([entry]);
		const withToken = { ...body, listing: { entries: [entry], nextPageToken: 'p1.50' } };
		const page = await pageOf('/den/commissions', withToken);
		expect(page.view === 'open' && page.body.body === 'listing' && page.body.more).toBe(
			'/den/commissions?pageToken=p1.50'
		);
	});

	it('links a folder above the item to its next page too', async () => {
		const rootBody = {
			node: { ...openNode, segments: [], name: 'Alice' },
			listing: { entries: [openNode], nextPageToken: 'p1.25' }
		};
		const result = await runPage(
			'/den/commissions',
			(url) => Response.json(url === '/api/v1/den' ? rootBody : folderBody([entry])),
			true
		);
		if (!('result' in result) || result.result !== 'page') throw new Error('expected a page');
		expect(result.data.ancestors).toEqual([
			expect.objectContaining({ folder: '/den', more: '/den?pageToken=p1.25' })
		]);
	});

	it('gives the root no crumbs', async () => {
		const body = { node: { ...openNode, segments: [], name: 'Alice' }, listing: { entries: [] } };
		const page = await pageOf('/den', body);
		expect(page.crumbs).toEqual([]);
	});
});

describe('the tolerant reader: broken answers', () => {
	it.each([
		['an answer with no node', () => Response.json({ crumbs: [], listing: { entries: [] } })],
		['a body that isn’t JSON', () => new Response('<html>', { status: 200 })],
		['a node of the wrong shape', () => Response.json({ node: { segments: 'commissions' } })]
	])('is a contract violation for %s', async (_, respond) => {
		const root = await runPage('/den', respond);
		const node = await runPage('/den/commissions', respond);
		expect('_tag' in root && root._tag).toBe('ContractViolation');
		expect('_tag' in node && node._tag).toBe('ContractViolation');
	});
});

/** The read a node answer's JSON decodes to, through the live port's own decode, or a thrown test failure. */
function readOf(body: JsonValue): DenRead {
	const read = denReadFromNode(decodeContract(GetDenNodeResponseSchema, body));
	if (read === undefined) throw new Error('expected a read');
	return read;
}

/** The first listing entry of a read, or a thrown test failure. */
function firstEntry(read: DenRead): DenReadNode {
	if (read.content.content !== 'listing') throw new Error('expected a listing');
	const [first] = read.content.entries;
	if (first === undefined) throw new Error('expected an entry');
	return first;
}

/** The thing an access names; none for a card. */
function thingOf(access: DenReadAccess) {
	return access.access === 'open' ? access.thing : undefined;
}

/** A one-entry answer at `/den/commissions` whose entry is `node`. */
function answerWith(node: JsonObject): JsonValue {
	return { node: openNode, crumbs: [], listing: { entries: [node] } };
}

describe('the thing an open node names', () => {
	it('an open commission maps its thing', () => {
		const node = { ...entry, open: { ownLevel: 'private', commissionId: 'c1' } };
		const id = commissionId('c1');
		expect(thingOf(firstEntry(readOf(answerWith(node))).access)).toEqual({
			thing: 'commission',
			id
		});
		const own = { node: { ...entry, open: { ownLevel: 'private', commissionId: 'c1' } } };
		expect(thingOf(readOf(own).node.access)).toEqual({ thing: 'commission', id });
	});

	it('an account maps its did', () => {
		const node = {
			...entry,
			type: 'account',
			open: { ownLevel: 'private', accountDid: 'did:plc:a1' }
		};
		expect(thingOf(firstEntry(readOf(answerWith(node))).access)).toEqual({
			thing: 'account',
			did: did('did:plc:a1')
		});
	});

	it('an open node that names nothing has no thing', () => {
		expect(thingOf(firstEntry(readOf(answerWith(entry))).access)).toBeUndefined();
	});

	it('a thing case this build does not know is no thing', () => {
		const node = { ...entry, open: { ownLevel: 'private', characterDid: 'did:plc:c1' } };
		const access = firstEntry(readOf(answerWith(node))).access;
		expect(access).toMatchObject({ access: 'open', ownLevel: 'private' });
		expect(thingOf(access)).toBeUndefined();
	});

	it('a card with a stray thing key stays a card', () => {
		const onNode = { name: 'Locked', type: 'commission', card: {}, commissionId: 'c1' };
		const inCard = { name: 'Locked', type: 'account', card: { accountDid: 'did:plc:a1' } };
		for (const node of [onNode, inCard])
			expect(firstEntry(readOf(answerWith(node))).access).toEqual({ access: 'card' });
	});

	it('an empty account did is no thing', () => {
		const node = { ...entry, type: 'account', open: { ownLevel: 'private', accountDid: '' } };
		const access = firstEntry(readOf(answerWith(node))).access;
		expect(access).toMatchObject({ access: 'open' });
		expect(thingOf(access)).toBeUndefined();
	});

	it('a whitespace account did is no thing', () => {
		for (const accountDid of [' ', '\t', ' \n ']) {
			const node = { ...entry, type: 'account', open: { ownLevel: 'private', accountDid } };
			const access = firstEntry(readOf(answerWith(node))).access;
			expect(access).toMatchObject({ access: 'open' });
			expect(thingOf(access)).toBeUndefined();
		}
	});

	it('an empty commission id is no thing', () => {
		const node = { ...entry, open: { ownLevel: 'private', commissionId: '' } };
		const access = firstEntry(readOf(answerWith(node))).access;
		expect(access).toMatchObject({ access: 'open' });
		expect(thingOf(access)).toBeUndefined();
	});
});

describe('a crumb’s type', () => {
	it('a crumb carries its type', () => {
		const crumbs = [
			{ name: 'Alice', type: 'user' },
			{ name: 'commissions', type: 'user.commissions' }
		];
		expect(readOf({ node: openNode, crumbs }).crumbs.map((crumb) => crumb.type)).toEqual([
			'user',
			'user.commissions'
		]);
	});

	it('a crumb without a type reads empty', () => {
		const crumbs = [{ name: 'Alice' }, { name: 'commissions', segments: ['commissions'] }];
		expect(readOf({ node: openNode, crumbs }).crumbs).toEqual([
			{ segments: [], name: 'Alice', type: '' },
			{ segments: ['commissions'], name: 'commissions', type: '' }
		]);
	});
});

/** An answer at `/den/commissions/c1` whose node, second crumb and one entry each carry `ordinal`. */
function numberedAnswer(ordinal: number | undefined): JsonObject {
	const number = ordinal === undefined ? {} : { nameOrdinal: ordinal };
	return {
		node: { ...entry, name: 'Test 1', ...number },
		crumbs: [
			{ name: 'Alice', segments: [] },
			{ name: 'commissions', segments: ['commissions'], ...number }
		],
		listing: { entries: [{ ...entry, segments: ['commissions', 'c1', 'x'], name: 'x', ...number }] }
	};
}

/** The numbers a read carries: on its node, its second crumb and its first entry. */
function readNumbers(read: DenRead): readonly (number | undefined)[] {
	return [read.node.ordinal, read.crumbs[1]?.ordinal, firstEntry(read).ordinal];
}

/** The numbers a page carries: on its node, its second crumb, its first entry and its last two path steps. */
function pageNumbers(page: DenPage, trail: readonly { readonly step: string }[]) {
	const steps = trail.map((step) => ('ordinal' in step ? step.ordinal : undefined));
	return [page.node.ordinal, page.crumbs[1]?.ordinal, entriesOf(page)[0]?.ordinal, ...steps];
}

describe('a name’s number: the tolerant reader', () => {
	it.each([
		['absent', undefined],
		['0', 0],
		['1', 1]
	])('reads a number that is %s as no number, on the node, a crumb and an entry', (_, raw) => {
		expect(readNumbers(readOf(numberedAnswer(raw)))).toEqual([undefined, undefined, undefined]);
	});

	it.each([2, 7])('reads %d as that number, on the node, a crumb and an entry', (raw) => {
		expect(readNumbers(readOf(numberedAnswer(raw)))).toEqual([raw, raw, raw]);
	});

	it('reads a fraction as no number, on the node, a crumb and an entry', () => {
		const message = create(GetDenNodeResponseSchema, {
			node: { name: 'Test 1', nameOrdinal: 2.5, access: { case: 'open', value: {} } },
			crumbs: [{ name: 'Alice' }, { name: 'commissions', nameOrdinal: 2.5 }],
			content: {
				case: 'listing',
				value: { entries: [{ name: 'x', nameOrdinal: 2.5, access: { case: 'card', value: {} } }] }
			}
		});
		const read = denReadFromNode(message);
		if (read === undefined) throw new Error('expected a read');
		expect(readNumbers(read)).toEqual([undefined, undefined, undefined]);
	});

	it('keeps the name bare: the number never joins it', () => {
		const read = readOf(numberedAnswer(2));
		expect([read.node.name, read.crumbs[1]?.name, firstEntry(read).name]).toEqual([
			'Test 1',
			'commissions',
			'x'
		]);
	});
});

describe('a name’s number: through the den program', () => {
	/** The page and its trail for `body` at `/den/commissions/c1`, or a thrown test failure. */
	async function pageAndTrail(body: JsonObject) {
		const result = await runPage('/den/commissions/c1', () => Response.json(body));
		if (!('result' in result) || result.result !== 'page') throw new Error('expected a page');
		const { outcome, trail } = result.data;
		if (outcome.outcome !== 'page') throw new Error('expected a page');
		return { page: outcome.page, trail };
	}

	it.each([
		['absent', undefined],
		['0', 0],
		['1', 1]
	])('shows a number that is %s as no number anywhere on the page', async (_, raw) => {
		const { page, trail } = await pageAndTrail(numberedAnswer(raw));
		expect(pageNumbers(page, trail)).toEqual([
			undefined,
			undefined,
			undefined,
			undefined,
			undefined,
			undefined
		]);
	});

	it.each([2, 7])(
		'carries %d to the node, the crumb, the entry and their path steps',
		async (raw) => {
			const { page, trail } = await pageAndTrail(numberedAnswer(raw));
			expect(pageNumbers(page, trail)).toEqual([raw, raw, raw, undefined, raw, raw]);
		}
	);
});
