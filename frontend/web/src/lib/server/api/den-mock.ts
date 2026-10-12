/**
 * The mock Den world: alice's Den as the approved fixture tree draws it —
 * `accounts`, `characters` (Ember, Kael-sona), `commissions` (the commission
 * "Untitled") and `posts` — read the way the Den contract reads. Its
 * `accounts` folder comes from the mock's own account store, so founding or
 * deleting an Account on `/accounts` shows up here. A few HAND-MADE extras,
 * marked below, cover what the seed can't produce: a card, an archived
 * commission, a deactivated Account, a folder with more than one page, a very
 * long name and a right-to-left name.
 */

import { Effect } from 'effect';
import type { AccountMembership } from '$lib/api/account';
import {
	invalidRequestProblem,
	NODE_NOT_FOUND_PROBLEM,
	NOT_AUTHENTICATED_PROBLEM
} from '$lib/api/problem';
import type { Session } from '$lib/api/session';
import type { SegmentPath } from '../path-builder';
import type { DenQuery, DenRead, DenReadCrumb, DenReadNode } from './den-read';
import { ApiProblem, NotAuthenticated } from './errors';

/** How many entries one page of a mock listing holds. */
export const MOCK_PAGE_SIZE = 25;

/** The mock's page tokens: a version, then the offset into the viewer's filtered listing. */
const MOCK_TOKEN_PATTERN = /^p1\.(\d{1,6})$/;

/** A node of the mock world. Children are a function, so a folder can read the store. */
interface MockNode {
	readonly key: string;
	readonly name: string;
	readonly type: string;
	readonly kind: 'directory' | 'file' | 'symlink';
	readonly level: 'private' | 'listed' | 'public';
	readonly mount: boolean;
	readonly access: 'open' | 'card';
	readonly contentNotShown: boolean;
	readonly removed: 'archived' | 'deactivated' | undefined;
	readonly children: () => readonly MockNode[];
}

/** What every node starts as: an open, private, empty folder. */
const FOLDER: Omit<MockNode, 'key' | 'name' | 'type'> = {
	kind: 'directory',
	level: 'private',
	mount: false,
	access: 'open',
	contentNotShown: false,
	removed: undefined,
	children: () => []
};

/** A private, open, empty folder named by its key. */
function systemFolder(key: string, type: string, children: () => readonly MockNode[] = () => []) {
	const folder: MockNode = { ...FOLDER, key, name: key, type, children };
	return folder;
}

/** A file whose content this read doesn't show. */
function file(key: string, name: string, type: string): MockNode {
	return { ...FOLDER, key, name, type, kind: 'file', contentNotShown: true };
}

/** An Account's three system folders. */
function accountFolders(): readonly MockNode[] {
	return [
		systemFolder('characters', 'account.characters'),
		{ ...systemFolder('commissions', 'account.commissions'), contentNotShown: true },
		systemFolder('workflows', 'account.workflows')
	];
}

/** One of the viewer's Accounts, mounted into `accounts`. */
function accountMount(account: AccountMembership): MockNode {
	return {
		...FOLDER,
		key: account.did,
		name: account.name,
		type: 'account',
		mount: true,
		children: accountFolders
	};
}

/** A commission's fixed entries, with the files and slots it is given. */
function commissionEntries(
	attachments: readonly MockNode[],
	slots: readonly MockNode[]
): readonly MockNode[] {
	return [
		systemFolder('attachments', 'commission.attachments', () => attachments),
		file('changelog', 'changelog', 'commission.changelog'),
		systemFolder('products', 'commission.products'),
		systemFolder('slots', 'commission.slots', () => slots)
	];
}

/** A commission mounted into `commissions`. */
function commissionMount(key: string, name: string, children: () => readonly MockNode[]): MockNode {
	return { ...FOLDER, key, name, type: 'commission', mount: true, children };
}

/** An open, empty slot. */
function slot(key: string, name: string): MockNode {
	return { ...systemFolder(key, 'commission.slot'), name };
}

/** The fixture commission "Untitled": two attachments, the changelog, no products, four open slots. */
const UNTITLED = commissionMount('01a0ef9c-5b2e-7c41-9d3a-6f1e2b7c8d90', 'Untitled', () =>
	commissionEntries(
		[
			file('bafkreiaref0abco', 'ref-abco.png', 'commission.attachment'),
			file('bafkreisketch001', 'sketch-01.png', 'commission.attachment')
		],
		[
			slot('0192f1a0-0001-7000-8000-000000000001', 'Abco'),
			slot('0192f1a0-0002-7000-8000-000000000002', 'Ember'),
			slot('0192f1a0-0003-7000-8000-000000000003', 'Kael'),
			slot('0192f1a0-0004-7000-8000-000000000004', 'Open slot')
		]
	)
);

/** HAND-MADE: an archived commission, listed only with "Show archived and deactivated" on. */
const ARCHIVED_COMMISSION: MockNode = {
	...commissionMount('0192b7e0-0aa0-7000-8000-00000000a7c1', 'Winter YCH', () =>
		commissionEntries([], [])
	),
	removed: 'archived'
};

/** HAND-MADE: a card — a commission the viewer may know of but can't open. */
const CARD_COMMISSION: MockNode = {
	...FOLDER,
	key: '0192c0de-7a11-7000-8000-00000000ca7d',
	name: 'Some commission',
	type: 'commission',
	mount: true,
	access: 'card'
};

/** HAND-MADE: a commission whose attachments run past one page. */
const PAGED_COMMISSION = commissionMount(
	'0192d00d-0b16-7000-8000-0000000b16b1',
	'Big ref batch',
	() =>
		commissionEntries(
			Array.from({ length: 40 }, (_, index) => {
				const number = String(index + 1).padStart(2, '0');
				return file(`bafkreibatch${number}`, `ref-${number}.png`, 'commission.attachment');
			}),
			[]
		)
);

/** HAND-MADE: a name far too long for one line. */
const LONG_NAME_COMMISSION = commissionMount(
	'0192e1e9-0000-7000-8000-00000000106e',
	'A very long commission title that keeps on going so we can see how the tree and the pane cut a name that will never fit on one line',
	() => commissionEntries([], [])
);

/** HAND-MADE: a right-to-left name ("character drawing request" in Arabic). */
const RTL_NAME_COMMISSION = commissionMount(
	'0192f2f1-0000-7000-8000-0000000002f1',
	'طلب رسم شخصية',
	() => commissionEntries([], [])
);

/** HAND-MADE: a deactivated Account, shown to its Owner with "Show archived and deactivated" on. */
const DEACTIVATED_ACCOUNT: MockNode = {
	...FOLDER,
	key: 'did:plc:mockformerstudioaaaaaaaa',
	name: 'Former Studio',
	type: 'account',
	mount: true,
	removed: 'deactivated',
	children: accountFolders
};

/** The fixture characters alice keeps: Ember (Public) and the Private Kael-sona. */
const CHARACTERS: readonly MockNode[] = [
	{
		...FOLDER,
		key: 'did:plc:mockemberaaaaaaaaaaaaaaa',
		name: 'Ember',
		type: 'character',
		level: 'public',
		mount: true
	},
	{
		...FOLDER,
		key: 'did:plc:mockkaelsonaaaaaaaaaaaaa',
		name: 'Kael-sona',
		type: 'character',
		mount: true
	}
];

/** The viewer's own root: the four system folders, `accounts` read from the store. */
function denRoot(session: Session, accounts: readonly AccountMembership[]): MockNode {
	const name = session.displayName ?? session.handle ?? session.did;
	return {
		...FOLDER,
		key: '',
		name,
		type: 'user',
		children: () => [
			systemFolder('accounts', 'user.accounts', () => [
				...accounts.map(accountMount),
				DEACTIVATED_ACCOUNT
			]),
			systemFolder('characters', 'user.characters', () => CHARACTERS),
			systemFolder('commissions', 'user.commissions', () => [
				UNTITLED,
				ARCHIVED_COMMISSION,
				CARD_COMMISSION,
				PAGED_COMMISSION,
				LONG_NAME_COMMISSION,
				RTL_NAME_COMMISSION
			]),
			{
				...FOLDER,
				key: 'posts',
				name: 'posts',
				type: 'post',
				level: 'public',
				mount: true,
				contentNotShown: true
			}
		]
	};
}

/** Listings sort by name as the viewer is shown it, then by key. */
function byName(left: MockNode, right: MockNode): number {
	return left.name.localeCompare(right.name, 'en') || left.key.localeCompare(right.key, 'en');
}

/** `node` as the Den read shows it at `segments`. A card carries no path and no kind. */
function readNode(node: MockNode, segments: readonly string[]): DenReadNode {
	if (node.access === 'card') {
		return {
			segments: [],
			name: node.name,
			type: node.type,
			kind: '',
			mount: node.mount,
			access: { access: 'card' }
		};
	}
	return {
		segments,
		name: node.name,
		type: node.type,
		kind: node.kind,
		mount: node.mount,
		access: {
			access: 'open',
			ownLevel: node.level,
			contentNotShown: node.contentNotShown,
			removed: node.removed
		}
	};
}

/** The offset a page token holds, `0` without one, or `undefined` for a malformed token. */
function pageOffset(query: DenQuery): number | undefined {
	if (query.pageToken === undefined) return 0;
	const match = MOCK_TOKEN_PATTERN.exec(query.pageToken);
	return match?.[1] === undefined ? undefined : Number(match[1]);
}

/**
 * The mock's Den read: walk `path` from the viewer's root, then answer the
 * node, its crumbs and one page of its listing. Every miss is the one
 * `node_not_found`; a malformed page token is the fixed 422, checked before
 * the path; a soft-deleted entry is listed only with `includeDeleted`.
 */
export function mockDenRead(
	session: Session | undefined,
	accounts: readonly AccountMembership[],
	path: SegmentPath,
	query: DenQuery
): Effect.Effect<DenRead, NotAuthenticated | ApiProblem> {
	return Effect.suspend((): Effect.Effect<DenRead, NotAuthenticated | ApiProblem> => {
		if (session === undefined)
			return Effect.fail(new NotAuthenticated({ problem: NOT_AUTHENTICATED_PROBLEM }));
		const offset = pageOffset(query);
		if (offset === undefined) {
			const problem = invalidRequestProblem('That page token is not one this Den reads.');
			return Effect.fail(new ApiProblem({ problem }));
		}
		const notFound = Effect.fail(new ApiProblem({ problem: NODE_NOT_FOUND_PROBLEM }));

		let node = denRoot(session, accounts);
		const crumbs: DenReadCrumb[] = [];
		const walked: string[] = [];
		for (const segment of path) {
			if (node.access === 'card' || node.kind !== 'directory') return notFound;
			crumbs.push({ segments: [...walked], name: node.name });
			const child = node.children().find((candidate) => candidate.key === segment);
			if (child === undefined) return notFound;
			// A soft-deleted node the viewer could only see as a card is the same miss.
			if (child.access === 'card' && child.removed !== undefined) return notFound;
			node = child;
			walked.push(segment);
		}

		const self = readNode(node, walked);
		if (node.access === 'card')
			return Effect.succeed({ node: self, crumbs, content: { content: 'none' } });
		if (node.kind === 'file')
			return Effect.succeed({ node: self, crumbs, content: { content: 'file' } });
		if (node.kind === 'symlink')
			return Effect.succeed({ node: self, crumbs, content: { content: 'link' } });

		const visible = node.contentNotShown
			? []
			: node
					.children()
					.filter((child) => query.includeDeleted || child.removed === undefined)
					.toSorted(byName);
		const page = visible.slice(offset, offset + MOCK_PAGE_SIZE);
		const nextOffset = offset + MOCK_PAGE_SIZE;
		const read: DenRead = {
			node: self,
			crumbs,
			content: {
				content: 'listing',
				entries: page.map((child) => readNode(child, [...walked, child.key])),
				nextPageToken: nextOffset < visible.length ? `p1.${String(nextOffset)}` : undefined
			}
		};
		return Effect.succeed(read);
	});
}
