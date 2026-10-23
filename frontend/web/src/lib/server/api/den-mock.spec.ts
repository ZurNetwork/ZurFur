import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import type { DenEntry, DenPageData } from '$lib/api/den';
import { accountId, commissionId, did, pageToken } from '$lib/types/brand';
import { denPage } from '../den';
import { segmentPath } from '../path-builder';
import type { DenRead, DenReadNode } from './den-read';
import { createMockStore, zurfurApiMock, type MockStore } from './zurfur-api-mock';
import { ZurfurApi } from './zurfur-api';
import { mockDenRead, MOCK_PAGE_SIZE, type MockDenWorld } from './den-mock';

/** A mock Den world with nobody signed in. */
const NOBODY: MockDenWorld = { session: undefined, accounts: [], commissions: [] };

/** The first page, nothing deleted shown. */
const PLAIN = { includeDeleted: false, pageToken: undefined };

/** `segments` as a path the builder accepts, or a thrown test failure. */
function segmentPathOf(segments: readonly string[]) {
	const path = segmentPath(segments);
	if (path === undefined) throw new Error('bad test path');
	return path;
}

/** The fixture commission "Untitled". */
const UNTITLED = '01a0ef9c-5b2e-7c41-9d3a-6f1e2b7c8d90';

/** The hand-made commission whose attachments run past one page. */
const PAGED = '0192d00d-0b16-7000-8000-0000000b16b1';

/** The hand-made card. */
const CARD = '0192c0de-7a11-7000-8000-00000000ca7d';

/** The hand-made archived commission. */
const ARCHIVED = '0192b7e0-0aa0-7000-8000-00000000a7c1';

/** The den page for `address` over the mock world in `store`, as a full page load. */
async function open(address: string, store: MockStore = createMockStore()): Promise<DenPageData> {
	const url = new URL(address, 'http://127.0.0.1:5174');
	const program = denPage(url.pathname, url.searchParams, true).pipe(
		Effect.provide(zurfurApiMock(store))
	);
	const result = await Effect.runPromise(program);
	if (result.result !== 'page') throw new Error(`expected a page, got ${result.result}`);
	return result.data;
}

/** The names a page's listing shows, in the server's order. */
function names(data: DenPageData): readonly string[] {
	return entries(data).map((entry) => entry.name);
}

/** A page's listing entries. */
function entries(data: DenPageData): readonly DenEntry[] {
	const { outcome } = data;
	if (outcome.outcome !== 'page' || outcome.page.view !== 'open') return [];
	return outcome.page.body.body === 'listing' ? outcome.page.body.entries : [];
}

describe('the mock Den world', () => {
	it("holds alice's four system folders at the root", async () => {
		expect(names(await open('/den'))).toEqual(['accounts', 'characters', 'commissions', 'posts']);
	});

	it('mounts the store’s Accounts into accounts, so a newly founded Account appears', async () => {
		const store = createMockStore();
		expect(names(await open('/den/accounts', store))).toEqual(["Alice's Studio"]);

		const [studio] = store.accounts;
		if (studio === undefined) throw new Error('fixture missing');
		const founded = {
			...studio,
			id: accountId('acct-new'),
			did: did('did:plc:mocknew'),
			name: 'New Studio'
		};
		store.accounts.push(founded);

		expect(names(await open('/den/accounts', store))).toEqual(["Alice's Studio", 'New Studio']);
	});

	it('lists a commission the mock created in commissions, naming its id', async () => {
		const store = createMockStore();
		const create = Effect.flatMap(ZurfurApi, (api) => api.createCommission('Fox in the snow'));
		const created = await Effect.runPromise(create.pipe(Effect.provide(zurfurApiMock(store))));

		const read = await Effect.runPromise(mockDenRead(store, segmentPathOf(['commissions']), PLAIN));

		const row = rowWhere(rowsOf(read), (node) => node.name === 'Fox in the snow');
		expect(row.access).toMatchObject({ thing: { thing: 'commission', id: created.id } });
	});

	it('mounts a store commission at its own segment, naming its own id', async () => {
		const store = createMockStore();
		const id = commissionId('c-0192f0c0');
		if (id === undefined) throw new Error('bad test id');
		store.commissions.push({ segment: 'fox-in-the-snow', id, title: 'Fox in the snow' });

		const read = await Effect.runPromise(mockDenRead(store, segmentPathOf(['commissions']), PLAIN));

		const row = rowWhere(rowsOf(read), (node) => node.name === 'Fox in the snow');
		expect(row.segments).toEqual(['commissions', 'fox-in-the-snow']);
		expect(row.access).toMatchObject({ thing: { thing: 'commission', id } });
	});

	it('keeps Ember and Kael-sona under characters, mounted', async () => {
		const data = await open('/den/characters');
		expect(names(data)).toEqual(['Ember', 'Kael-sona']);
		expect(entries(data).every((entry) => entry.mounted)).toBe(true);
	});

	it('gives the commission "Untitled" its four entries this slice', async () => {
		expect(names(await open(`/den/commissions/${UNTITLED}`))).toEqual([
			'attachments',
			'changelog',
			'products',
			'slots'
		]);
	});

	it('marks posts as content not shown yet, with an empty listing', async () => {
		const data = await open('/den/posts');
		expect(data.outcome).toMatchObject({ page: { node: { contentNotShown: true } } });
		expect(names(data)).toEqual([]);
	});

	it('answers a file with an empty file content', async () => {
		const data = await open(`/den/commissions/${UNTITLED}/changelog`);
		expect(data.outcome).toMatchObject({
			page: { node: { kind: 'file' }, body: { body: 'file' } }
		});
	});

	it('reads only the signed-in visitor’s own Den: another user’s DID is the not-found', async () => {
		const asRoot = await open('/den/did:plc:mockbobaaaaaaaaaaaaaaaaaa');
		const asAccount = await open('/den/accounts/did:plc:mockbobaaaaaaaaaaaaaaaaaa');
		expect(asRoot.outcome).toEqual({ outcome: 'notFound' });
		expect(asAccount.outcome).toEqual({ outcome: 'notFound' });
	});

	it('answers the one not-found for a miss, and for a path past a file', async () => {
		const miss = await open('/den/nope');
		const pastFile = await open(`/den/commissions/${UNTITLED}/changelog/x`);
		expect(miss.outcome).toEqual({ outcome: 'notFound' });
		expect(pastFile.outcome).toEqual({ outcome: 'notFound' });
	});
});

describe('the hand-made extras', () => {
	it('lists a card among the commissions, with no link', async () => {
		const card = entries(await open('/den/commissions')).find(
			(entry) => entry.name === 'Some commission'
		);
		expect(card).toEqual({
			view: 'card',
			name: 'Some commission',
			type: 'commission',
			mounted: true
		});
	});

	it('answers a typed path to the card with the card page, and a path past it with the not-found', async () => {
		const page = await open(`/den/commissions/${CARD}`);
		const past = await open(`/den/commissions/${CARD}/attachments`);
		expect(page.outcome).toMatchObject({ page: { view: 'card' } });
		expect(past.outcome).toEqual({ outcome: 'notFound' });
	});

	it('hides the archived commission and the deactivated Account unless the flag is on', async () => {
		expect(names(await open('/den/commissions'))).not.toContain('Winter YCH');
		expect(names(await open('/den/accounts'))).not.toContain('Former Studio');

		const commissions = entries(await open('/den/commissions?includeDeleted=true'));
		const accounts = entries(await open('/den/accounts?includeDeleted=true'));
		expect(commissions.find((entry) => entry.name === 'Winter YCH')).toMatchObject({
			removed: 'archived'
		});
		expect(accounts.find((entry) => entry.name === 'Former Studio')).toMatchObject({
			removed: 'deactivated'
		});
	});

	it('opens the archived commission by its typed path, marked, even with the flag off', async () => {
		const data = await open(`/den/commissions/${ARCHIVED}`);
		expect(data.outcome).toMatchObject({
			page: { node: { name: 'Winter YCH', removed: 'archived' } }
		});
	});

	it('pages a long folder: a first page and a More link, then the rest', async () => {
		const first = await open(`/den/commissions/${PAGED}/attachments`);
		expect(names(first)).toHaveLength(MOCK_PAGE_SIZE);
		const { outcome } = first;
		if (
			outcome.outcome !== 'page' ||
			outcome.page.view !== 'open' ||
			outcome.page.body.body !== 'listing'
		)
			throw new Error('expected a listing');
		const more = outcome.page.body.more;
		expect(more).toBe(`/den/commissions/${PAGED}/attachments?pageToken=p1.25`);

		const second = await open(more ?? '');
		expect(names(second)).toHaveLength(40 - MOCK_PAGE_SIZE);
		expect(second.continued).toBe(true);
	});

	it('reloads from the first page when a page token is malformed', async () => {
		const url = new URL(`http://x.invalid/den/commissions?pageToken=garbage`);
		const program = denPage(url.pathname, url.searchParams, false).pipe(
			Effect.provide(zurfurApiMock(createMockStore()))
		);
		expect(await Effect.runPromise(program)).toEqual({
			result: 'reload',
			location: '/den/commissions'
		});
	});

	it('carries a very long name and a right-to-left name as they are', async () => {
		const shown = names(await open('/den/commissions'));
		expect(shown.some((name) => name.length > 100)).toBe(true);
		expect(shown).toContain('طلب رسم شخصية');
	});
});

describe('mockDenRead', () => {
	it('answers not authenticated when nobody is signed in', async () => {
		const failure = await Effect.runPromise(
			Effect.flip(mockDenRead(NOBODY, [], { includeDeleted: false, pageToken: undefined }))
		);
		expect(failure._tag).toBe('NotAuthenticated');
	});

	it('gives an empty page past the end', async () => {
		const store = createMockStore();
		const token = pageToken('p1.999');
		const read = await Effect.runPromise(
			mockDenRead(store, [], { includeDeleted: false, pageToken: token })
		);
		expect(read.content).toEqual({ content: 'listing', entries: [], nextPageToken: undefined });
	});
});

/** The mock read at `segments` for the fixture store, with the archived and deactivated shown. */
async function readAt(segments: readonly string[]): Promise<DenRead> {
	const store = createMockStore();
	const path = segmentPath(segments);
	if (path === undefined) throw new Error('bad test path');
	const query = { includeDeleted: true, pageToken: undefined };
	return Effect.runPromise(mockDenRead(store, path, query));
}

/** The entries of a listing read. */
function rowsOf(read: DenRead): readonly DenReadNode[] {
	if (read.content.content !== 'listing') throw new Error('expected a listing');
	return read.content.entries;
}

/** The first row `matches`, or a thrown test failure. */
function rowWhere(
	rows: readonly DenReadNode[],
	matches: (row: DenReadNode) => boolean
): DenReadNode {
	const row = rows.find(matches);
	if (row === undefined) throw new Error('expected a row');
	return row;
}

/** What a row names: its thing when open, and none for a card. */
function thingOf(row: DenReadNode) {
	return row.access.access === 'open' ? row.access.thing : undefined;
}

describe('the mock Den read: crumbs carry their type', () => {
	it('types each crumb by the node it stands for, the root as a user', async () => {
		const read = await readAt(['commissions', UNTITLED, 'slots']);
		expect(read.crumbs).toEqual([
			{ segments: [], name: 'Alice', type: 'user' },
			{ segments: ['commissions'], name: 'commissions', type: 'user.commissions' },
			{ segments: ['commissions', UNTITLED], name: 'Untitled', type: 'commission' }
		]);
	});

	it('types the crumbs of an Account path', async () => {
		const accountDid = createMockStore().accounts[0]?.did;
		const read = await readAt(['accounts', accountDid ?? '', 'commissions']);
		expect(read.crumbs.map((crumb) => crumb.type)).toEqual(['user', 'user.accounts', 'account']);
	});
});

describe('the mock Den read: an open node names its thing', () => {
	it('gives a commission node its id', async () => {
		const read = await readAt(['commissions', UNTITLED]);
		expect(read.node.access).toMatchObject({
			thing: { thing: 'commission', id: commissionId(UNTITLED) }
		});
	});

	it('gives each commission entry its id, the archived one too, and the card none', async () => {
		const rows = rowsOf(await readAt(['commissions']));
		const named = rows.filter((row) => row.access.access === 'open');
		expect(named.length).toBeGreaterThan(1);
		for (const row of named)
			expect(thingOf(row)).toEqual({
				thing: 'commission',
				id: commissionId(row.segments.at(-1) ?? '')
			});
		const archived = rowWhere(rows, (row) => row.segments.at(-1) === ARCHIVED);
		expect(thingOf(archived)).toEqual({
			thing: 'commission',
			id: commissionId(ARCHIVED)
		});
		const card = rows.find((row) => row.access.access === 'card');
		expect(card).toBeDefined();
		expect(card?.access).toEqual({ access: 'card' });
	});

	it('gives each Account entry its DID, the deactivated one too', async () => {
		const store = createMockStore();
		const rows = rowsOf(await readAt(['accounts']));
		const studio = store.accounts[0];
		expect(studio).toBeDefined();
		expect(thingOf(rowWhere(rows, (row) => row.name === studio?.name))).toEqual({
			thing: 'account',
			did: studio?.did
		});
		const former = rowWhere(rows, (row) => row.name === 'Former Studio');
		expect(thingOf(former)).toEqual({
			thing: 'account',
			did: did('did:plc:mockformerstudioaaaaaaaa')
		});
	});

	it('gives system folders, files, slots, characters and the root none', async () => {
		const root = await readAt([]);
		expect(thingOf(root.node)).toBeUndefined();
		expect(rowsOf(root).map(thingOf)).toEqual([undefined, undefined, undefined, undefined]);
		const inside = rowsOf(await readAt(['commissions', UNTITLED]));
		expect(inside.map(thingOf)).toEqual([undefined, undefined, undefined, undefined]);
		const slots = rowsOf(await readAt(['commissions', UNTITLED, 'slots']));
		expect(slots).toHaveLength(4);
		expect(slots.map(thingOf)).toEqual([undefined, undefined, undefined, undefined]);
		const characters = rowsOf(await readAt(['characters']));
		expect(characters.map(thingOf)).toEqual([undefined, undefined]);
		const accountFolder = rowsOf(
			await readAt(['accounts', createMockStore().accounts[0]?.did ?? ''])
		);
		expect(accountFolder.map(thingOf)).toEqual([undefined, undefined, undefined]);
	});
});

/** Create a commission titled `title` in `store` through the mock port, answering its id. */
async function createIn(store: MockStore, title: string): Promise<string> {
	const create = Effect.flatMap(ZurfurApi, (api) => api.createCommission(title));
	const created = await Effect.runPromise(create.pipe(Effect.provide(zurfurApiMock(store))));
	return created.id;
}

/** The opened node of a page, or a thrown test failure. */
function nodeOf(data: DenPageData) {
	const { outcome } = data;
	if (outcome.outcome !== 'page') throw new Error('expected a page');
	return outcome.page;
}

/** The number each listed entry named `name` shows, in listing order. */
function ordinalsNamed(data: DenPageData, name: string): readonly (number | undefined)[] {
	return entries(data)
		.filter((entry) => entry.name === name)
		.map((entry) => entry.ordinal);
}

describe('the mock Den numbers equal names', () => {
	it('gives the newer of two twins 2 and the older none', async () => {
		const store = createMockStore();
		const older = await createIn(store, 'Sketch');
		const newer = await createIn(store, 'Sketch');

		const listing = entries(await open('/den/commissions', store));

		const sketches = listing.filter((entry) => entry.name === 'Sketch');
		const byHref = sketches.map((entry) => [entry.view === 'open' && entry.href, entry.ordinal]);
		expect(byHref).toEqual([
			[`/den/commissions/${older}`, undefined],
			[`/den/commissions/${newer}`, 2]
		]);
		const others = listing.filter((entry) => entry.name !== 'Sketch');
		expect(others.map((entry) => entry.ordinal)).toEqual(others.map(() => undefined));
	});

	it('gives the opened twin the number its folder gives it, and no crumb a number', async () => {
		const store = createMockStore();
		const older = await createIn(store, 'Sketch');
		const newer = await createIn(store, 'Sketch');

		const newerPage = nodeOf(await open(`/den/commissions/${newer}`, store));
		const olderPage = nodeOf(await open(`/den/commissions/${older}`, store));

		expect(newerPage.node.ordinal).toBe(2);
		expect(newerPage.crumbs.map((crumb) => crumb.ordinal)).toEqual([undefined, undefined]);
		expect(olderPage.node.ordinal).toBeUndefined();
	});

	it('gives a crumb below a twin the twin’s number, and the path bar shows it', async () => {
		const store = createMockStore();
		await createIn(store, 'Sketch');
		const newer = await createIn(store, 'Sketch');

		const data = await open(`/den/commissions/${newer}/slots`, store);

		expect(nodeOf(data).crumbs.map((crumb) => crumb.ordinal)).toEqual([undefined, undefined, 2]);
		expect(data.trail).toEqual([
			{ step: 'root', href: '/den' },
			{ step: 'named', label: 'commissions', ordinal: undefined, href: '/den/commissions' },
			{ step: 'named', label: 'Sketch', ordinal: 2, href: `/den/commissions/${newer}` },
			{ step: 'named', label: 'slots', ordinal: undefined, href: undefined }
		]);
	});

	it('counts an archived twin only with "Show archived and deactivated" on', async () => {
		const store = createMockStore();
		const created = await createIn(store, 'Winter YCH');

		const plain = await open('/den/commissions', store);
		const shown = await open('/den/commissions?includeDeleted=true', store);
		const createdPlain = nodeOf(await open(`/den/commissions/${created}`, store));
		const createdShown = nodeOf(
			await open(`/den/commissions/${created}?includeDeleted=true`, store)
		);

		expect(ordinalsNamed(plain, 'Winter YCH')).toEqual([undefined]);
		const twins = entries(shown).filter((entry) => entry.name === 'Winter YCH');
		expect(twins.map((entry) => [entry.view === 'open' && entry.removed, entry.ordinal])).toEqual([
			['archived', undefined],
			[undefined, 2]
		]);
		expect(createdPlain.node.ordinal).toBeUndefined();
		expect(createdShown.node.ordinal).toBe(2);
	});

	it('lists an open twin first and unnumbered, then a card of the same name numbered 2', async () => {
		const store = createMockStore();
		await createIn(store, 'Some commission');

		const twins = entries(await open('/den/commissions', store)).filter(
			(entry) => entry.name === 'Some commission'
		);

		expect(twins.map((entry) => [entry.view, entry.ordinal])).toEqual([
			['open', undefined],
			['card', 2]
		]);
	});

	it('numbers over the whole listing, not one page', async () => {
		const store = createMockStore();
		for (let made = 0; made < 27; made += 1) await createIn(store, 'Sketch');

		const first = await open('/den/commissions', store);
		const firstPage = ordinalsNamed(first, 'Sketch');
		const { outcome } = first;
		if (outcome.outcome !== 'page' || outcome.page.view !== 'open')
			throw new Error('expected a page');
		if (outcome.page.body.body !== 'listing' || outcome.page.body.more === undefined)
			throw new Error('expected a second page');
		const secondPage = ordinalsNamed(await open(outcome.page.body.more, store), 'Sketch');

		const expected = [undefined, ...Array.from({ length: 26 }, (_, index) => index + 2)];
		expect([...firstPage, ...secondPage]).toEqual(expected);
		expect(secondPage.length).toBeGreaterThan(0);
	});
});
