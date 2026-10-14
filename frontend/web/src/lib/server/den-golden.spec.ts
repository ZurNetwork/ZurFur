// The Den's golden responses (`contract/golden/den/`), recorded from the real
// routes, served through the live port and read by the den program: one test
// per recorded file, plus the cases the fixtures exist to prove.
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import { render } from 'svelte/server';
import type { JsonValue } from '@bufbuild/protobuf';
import { cardTypeWord, type CardEntry, type DenEntry, type DenPage } from '$lib/api/den';
import { fetchStub } from '$lib/testing/http';
import { denHref, denName, denType, did, pageToken } from '$lib/types/brand';
import EntryList from '$lib/components/explorer/EntryList.svelte';
import NodeHeader from '$lib/components/explorer/NodeHeader.svelte';
import Tree from '$lib/components/explorer/Tree.svelte';
import { TreeMemory } from '$lib/components/explorer/tree-memory.svelte';
import { denPage, type DenPageResult } from './den';
import { decodeContract, RequestFetch, ZurfurApiLive } from './api/zurfur-api';
import type { ContractViolation, NetworkFailure } from './api/errors';
import type { DenQuery } from './api/den-read';
import { segmentPath } from './path-builder';
import { denApiPathOf, denHrefOf, denPathFromPathname, denQueryFromSearch } from './den/den-path';
import {
	type DenNode,
	GetDenNodeResponseSchema,
	GetDenRootResponseSchema
} from './api/generated/zurfur/api/v1/den_pb';

/** One recorded answer, as `index.json` lists it. */
interface GoldenEntry {
	readonly file: string;
	readonly message: 'GetDenRootResponse' | 'GetDenNodeResponse' | 'Problem';
	readonly path: string;
	readonly status: number;
	readonly contentType: string;
	readonly source: 'generated' | 'handwritten';
}

const GOLDEN_DIR = new URL('../../../../../contract/golden/den/', import.meta.url);
const INDEX = JSON.parse(readFileSync(new URL('index.json', GOLDEN_DIR), 'utf8')) as {
	files: GoldenEntry[];
};

/** Every recorded file on disk, relative to the golden folder. */
function filesOnDisk(): string[] {
	return readdirSync(GOLDEN_DIR, { recursive: true, withFileTypes: true })
		.filter((dirent) => dirent.isFile() && dirent.name.endsWith('.json'))
		.map((dirent) => relative(fileURLToPath(GOLDEN_DIR), join(dirent.parentPath, dirent.name)))
		.filter((file) => file !== 'index.json')
		.sort();
}

function golden(file: string): GoldenEntry {
	const entry = INDEX.files.find((candidate) => candidate.file === file);
	if (entry === undefined) throw new Error(`no golden ${file}`);
	return entry;
}

function bodyText(entry: GoldenEntry): string {
	return readFileSync(new URL(entry.file, GOLDEN_DIR), 'utf8');
}

/** The query every link in a page carries: the flag, never a page token. */
function linkQueryOf(address: string): DenQuery {
	const query = denQueryFromSearch(new URL(address, 'http://127.0.0.1:5174').searchParams);
	return { includeDeleted: query.includeDeleted, pageToken: undefined };
}

/** The link to `segments` under `query`, built by the path builder as the den program does. */
function hrefOf(segments: readonly string[], query: DenQuery): string | undefined {
	const path = segmentPath(segments);
	return path === undefined ? undefined : denHrefOf(path, query);
}

/** The row the den program must make of a recorded node, built from the recording alone. */
function expectedRow(node: DenNode, query: DenQuery): object {
	if (node.access.case !== 'open')
		return { view: 'card', name: node.name, type: node.type, mounted: node.mount };
	const open = node.access.value;
	return {
		view: 'open',
		href: hrefOf(node.segments, query),
		name: node.name,
		type: node.type,
		kind: node.kind,
		mounted: node.mount,
		ownLevel: open.ownLevel,
		contentNotShown: open.contentNotShown,
		removed: open.softDelete === '' ? undefined : open.softDelete
	};
}

/** The page the den program must make of `entry`'s recorded body, built from the recording alone. */
function expectedPage(entry: GoldenEntry): object {
	const raw = JSON.parse(bodyText(entry)) as JsonValue;
	const query = linkQueryOf(entry.path);
	const root = entry.message === 'GetDenRootResponse';
	const message = root
		? decodeContract(GetDenRootResponseSchema, raw)
		: decodeContract(GetDenNodeResponseSchema, raw);
	if (message.node === undefined) throw new Error(`${entry.file} has no node`);
	const crumbs = (root ? [] : decodeContract(GetDenNodeResponseSchema, raw).crumbs).map(
		(crumb) => ({ href: hrefOf(crumb.segments, query), name: crumb.name })
	);
	const node = expectedRow(message.node, query);
	if (message.node.access.case !== 'open') return { view: 'card', node, crumbs };
	const content = message.content;
	const token = content.case === 'listing' ? pageToken(content.value.nextPageToken) : undefined;
	const more =
		token === undefined ? undefined : hrefOf(message.node.segments, { ...query, pageToken: token });
	const entries =
		content.case === 'listing' ? content.value.entries.map((row) => expectedRow(row, query)) : [];
	const body =
		content.case === 'listing'
			? { body: 'listing', entries, more }
			: { body: content.case ?? 'notShown' };
	return { view: 'open', node, crumbs, body };
}

/** The API request the web makes for a browser address: its allowlisted query only. */
function requestOf(address: string): string {
	const url = new URL(address, 'http://127.0.0.1:5174');
	const path = denPathFromPathname(url.pathname);
	const apiPath =
		path === undefined ? undefined : denApiPathOf(path, denQueryFromSearch(url.searchParams));
	return `/api/v1${apiPath ?? ''}`;
}

/** The den program for `address`, every read answered by `entry`'s recorded body. */
async function serve(
	entry: GoldenEntry,
	address = entry.path
): Promise<{ result: DenPageResult | NetworkFailure | ContractViolation; calls: string[] }> {
	const url = new URL(address, 'http://127.0.0.1:5174');
	const { fetch, calls } = fetchStub(
		() =>
			new Response(bodyText(entry), {
				status: entry.status,
				headers: { 'content-type': entry.contentType }
			})
	);
	const program = denPage(url.pathname, url.searchParams, false).pipe(
		Effect.either,
		Effect.map((either) => (either._tag === 'Right' ? either.right : either.left)),
		Effect.provide(ZurfurApiLive),
		Effect.provideService(RequestFetch, fetch)
	);
	return { result: await Effect.runPromise(program), calls };
}

/** The page `entry` renders as, or a thrown test failure. */
async function pageOf(entry: GoldenEntry, address = entry.path): Promise<DenPage> {
	const { result } = await serve(entry, address);
	if (!('result' in result) || result.result !== 'page') throw new Error('expected a page');
	if (result.data.outcome.outcome !== 'page') throw new Error('expected a page outcome');
	return result.data.outcome.page;
}

function entriesOf(page: DenPage): readonly DenEntry[] {
	if (page.view !== 'open' || page.body.body !== 'listing') throw new Error('expected a listing');
	return page.body.entries;
}

describe('the Den golden responses', () => {
	it('index every recorded file, and nothing else', () => {
		const indexed = INDEX.files.map((entry) => entry.file).sort();
		expect(indexed).toEqual(filesOnDisk());
	});

	for (const entry of INDEX.files) {
		it(entry.file, async () => {
			const { result, calls } = await serve(entry);
			switch (entry.status) {
				case 200: {
					expect(calls).toEqual([`/api/v1${entry.path}`]);
					if (!('result' in result) || result.result !== 'page') throw new Error('expected a page');
					const outcome = result.data.outcome;
					if (outcome.outcome !== 'page') throw new Error('expected a page outcome');
					expect(outcome.page).toEqual(expectedPage(entry));
					break;
				}
				case 404:
					expect(calls).toEqual([`/api/v1${entry.path}`]);
					expect('result' in result && result.result === 'page' && result.data.outcome).toEqual({
						outcome: 'notFound'
					});
					break;
				case 401:
					expect(calls).toEqual([`/api/v1${entry.path}`]);
					expect(result).toEqual({ result: 'signedOut' });
					break;
				case 400:
				case 422:
					expect('_tag' in result && result._tag).toBe('ContractViolation');
					// The web sends only its allowlisted query, never what the address carried.
					expect(calls).toEqual([requestOf(entry.path)]);
					expect(calls).not.toEqual([`/api/v1${entry.path}`]);
					break;
				default:
					throw new Error(`no expectation for status ${String(entry.status)}`);
			}
		});
	}
});

describe('the Den golden responses: what the fixtures prove', () => {
	it('alice’s root lists accounts, characters, commissions and posts', async () => {
		const page = await pageOf(golden('alice/root.json'));
		expect(entriesOf(page).map((row) => row.name)).toEqual([
			'accounts',
			'characters',
			'commissions',
			'posts'
		]);
		expect(page.crumbs).toEqual([]);
	});

	it('shows "Old sketch" archived, and only under includeDeleted', async () => {
		const plain = entriesOf(await pageOf(golden('alice/commissions.json')));
		const flagged = entriesOf(await pageOf(golden('alice/commissions-include-deleted.json')));
		expect(plain.map((row) => row.name)).not.toContain('Old sketch');
		expect(flagged.find((row) => row.name === 'Old sketch')).toMatchObject({
			view: 'open',
			removed: 'archived'
		});
		const own = await pageOf(golden('alice/old-sketch.json'));
		expect(own.node).toMatchObject({ view: 'open', removed: 'archived' });
	});

	it.each([
		['handwritten/listing-with-card.json', 'Hidden commission', 'Commission'],
		['handwritten/listing-with-card-mount.json', 'Locked Studio', 'Account']
	])('%s renders its card as "%s", a %s with no link', async (file, name, word) => {
		const rows = entriesOf(await pageOf(golden(file)));
		const card = rows.find((row) => row.name === name);
		expect(card).toMatchObject({ view: 'card' });
		expect(card !== undefined && 'href' in card).toBe(false);
		expect(card !== undefined && cardTypeWord(card.type)).toBe(word);

		const pane = render(EntryList, { props: { entries: rows, more: undefined } }).body;
		const cardRow = pane.slice(pane.indexOf('data-testid="entry-card"'));
		expect(cardRow).toContain(name);
		expect(cardRow).toContain(`data-testid="card-type">${word}<`);
		expect(pane.match(/<a /g)?.length).toBe(rows.filter((row) => row.view === 'open').length);

		const root = denHref('/den');
		const memory = new TreeMemory(did('did:plc:alice'));
		memory.loaded(root, rows, undefined);
		memory.expanded.add(root);
		const current = { href: undefined, parentHref: undefined, entry: undefined };
		const tree = render(Tree, { props: { memory, current, fallbackRootHref: root } }).body;
		const treeCard = tree.slice(tree.indexOf('data-tree-kind="card"'));
		expect(treeCard).toContain(name);
		expect(treeCard).toContain(`data-testid="card-type">${word}<`);
	});

	it('card-node.json is a card page with its crumbs and no body', async () => {
		const page = await pageOf(golden('handwritten/card-node.json'));
		expect(page.view).toBe('card');
		expect(page.crumbs.map((crumb) => crumb.name)).toEqual(['Alice', 'commissions']);
		expect('body' in page).toBe(false);

		const header = render(NodeHeader, { props: { node: page.node } }).body;
		expect(header).toContain('Hidden commission');
		expect(header).toContain('data-testid="card-type">Commission<');
		expect(header).toContain("Can't open");
		expect(header).not.toContain('href');
	});
});

describe('a card of a type this build doesn’t know', () => {
	const card: CardEntry = {
		view: 'card',
		name: denName('Locked characters'),
		type: denType('account.characters'),
		mounted: false
	};

	it('shows no type word in the pane, the tree or the header', () => {
		const pane = render(EntryList, { props: { entries: [card], more: undefined } }).body;
		expect(pane).toContain('Locked characters');
		expect(pane).not.toContain('data-testid="card-type"');

		const root = denHref('/den');
		const memory = new TreeMemory(did('did:plc:alice'));
		memory.loaded(root, [card], undefined);
		memory.expanded.add(root);
		const current = { href: undefined, parentHref: undefined, entry: undefined };
		const tree = render(Tree, { props: { memory, current, fallbackRootHref: root } }).body;
		expect(tree).toContain('Locked characters');
		expect(tree).not.toContain('data-testid="card-type"');

		const header = render(NodeHeader, { props: { node: card } }).body;
		expect(header).toContain('Locked characters');
		expect(header).not.toContain('data-testid="card-type"');
		expect(header).not.toContain('account.characters');
	});
});
