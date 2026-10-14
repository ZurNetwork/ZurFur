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
import { cardTypeWord, type DenEntry, type DenPage } from '$lib/api/den';
import { fetchStub } from '$lib/testing/http';
import { denHref, did } from '$lib/types/brand';
import EntryList from '$lib/components/explorer/EntryList.svelte';
import NodeHeader from '$lib/components/explorer/NodeHeader.svelte';
import Tree from '$lib/components/explorer/Tree.svelte';
import { TreeMemory } from '$lib/components/explorer/tree-memory.svelte';
import { denPage, type DenPageResult } from './den';
import { decodeContract, RequestFetch, ZurfurApiLive } from './api/zurfur-api';
import type { ContractViolation, NetworkFailure } from './api/errors';
import {
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

/** The recorded node name and entry names, decoded through the contract. */
function recorded(entry: GoldenEntry): { name: string; entries: string[] | undefined } {
	const raw = JSON.parse(bodyText(entry)) as JsonValue;
	const message =
		entry.message === 'GetDenRootResponse'
			? decodeContract(GetDenRootResponseSchema, raw)
			: decodeContract(GetDenNodeResponseSchema, raw);
	const listing = message.content.case === 'listing' ? message.content.value : undefined;
	return {
		name: message.node?.name ?? '',
		entries: listing?.entries.map((node) => node.name)
	};
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
					const want = recorded(entry);
					expect(outcome.page.node.name).toBe(want.name);
					const names =
						outcome.page.view === 'open' && outcome.page.body.body === 'listing'
							? outcome.page.body.entries.map((row) => row.name)
							: undefined;
					expect(names).toEqual(want.entries);
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
