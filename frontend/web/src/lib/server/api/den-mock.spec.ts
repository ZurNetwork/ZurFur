import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import type { DenEntry, DenPageData } from '$lib/api/den';
import { accountId, did, pageToken } from '$lib/types/brand';
import { denPage } from '../den';
import { createMockStore, zurfurApiMock, type MockStore } from './zurfur-api-mock';
import { mockDenRead, MOCK_PAGE_SIZE } from './den-mock';

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
			Effect.flip(mockDenRead(undefined, [], [], { includeDeleted: false, pageToken: undefined }))
		);
		expect(failure._tag).toBe('NotAuthenticated');
	});

	it('gives an empty page past the end', async () => {
		const store = createMockStore();
		const token = pageToken('p1.999');
		const read = await Effect.runPromise(
			mockDenRead(store.session, store.accounts, [], { includeDeleted: false, pageToken: token })
		);
		expect(read.content).toEqual({ content: 'listing', entries: [], nextPageToken: undefined });
	});
});
