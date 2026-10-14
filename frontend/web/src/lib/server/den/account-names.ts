/**
 * An Account's own name in the Den view. The Den keys an Account node by its
 * DID and names it by a name fit for a path segment, so an Account whose name
 * holds a `/` comes back named by its handle. The den program reads the
 * viewer's `GET /accounts` list once for a page that shows an Account node
 * and names each such node from it.
 */

import { Effect } from 'effect';
import type { DenRead, DenReadContent, DenReadNode } from '../api/den-read';
import { ZurfurApi } from '../api/zurfur-api';

/** The Den type of an Account node. */
const ACCOUNT_TYPE = 'account';

/** The viewer's Account names, keyed by the Account's DID. */
export type AccountNames = ReadonlyMap<string, string>;

/** No names: every node keeps the Den's name. */
const NO_NAMES: AccountNames = new Map();

/** The DID an Account node is keyed by, its last segment; `undefined` for any other node or a card. */
function accountDid(node: DenReadNode): string | undefined {
	if (node.type !== ACCOUNT_TYPE) return undefined;
	return node.segments.at(-1);
}

/** Every node `read` shows: the node itself and, for a listing, its entries. */
function shownNodes(read: DenRead): readonly DenReadNode[] {
	const entries = read.content.content === 'listing' ? read.content.entries : [];
	return [read.node, ...entries];
}

/** Whether any of `reads` shows an Account node a name could be found for. */
function showsAnAccount(reads: readonly DenRead[]): boolean {
	return reads.some((read) => shownNodes(read).some((node) => accountDid(node) !== undefined));
}

/**
 * The viewer's Account names for a page made of `reads`: one `GET /accounts`
 * when they show an Account node, none otherwise. A list that can't be read
 * names nothing, so the Den's names stand.
 */
export function accountNamesFor(
	reads: readonly DenRead[]
): Effect.Effect<AccountNames, never, ZurfurApi> {
	if (!showsAnAccount(reads)) return Effect.succeed(NO_NAMES);
	return Effect.gen(function* () {
		const api = yield* ZurfurApi;
		const accounts = yield* api.listAccounts;
		const names: AccountNames = new Map(accounts.map((account) => [account.did, account.name]));
		return names;
	}).pipe(Effect.catchAll(() => Effect.succeed(NO_NAMES)));
}

/** `node` named by `names` when it is an Account they list, else as the Den named it. */
function named(node: DenReadNode, names: AccountNames): DenReadNode {
	const did = accountDid(node);
	const name = did === undefined ? undefined : names.get(did);
	return name === undefined ? node : { ...node, name };
}

/** `read` with each Account node it shows named by `names`; every other node as it was. */
export function withAccountNames(read: DenRead, names: AccountNames): DenRead {
	if (names.size === 0) return read;
	const content: DenReadContent =
		read.content.content === 'listing'
			? { ...read.content, entries: read.content.entries.map((entry) => named(entry, names)) }
			: read.content;
	return { ...read, node: named(read.node, names), content };
}
