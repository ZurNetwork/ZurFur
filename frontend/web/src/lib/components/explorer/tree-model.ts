/**
 * What the tree shows, worked out from its memory and the open item: one
 * node per visible row, nested under the open folders. Pure, so the shape
 * of the tree (folders first, the partly loaded markers, where the open
 * item goes) is tested apart from the DOM.
 */

import { foldersFirst, type CardEntry, type DenEntry, type OpenEntry } from '$lib/api/den';
import type { DenHref } from '$lib/types/brand';
import type { FolderState } from './tree-memory.svelte';

/** Where a row sits, given only when its folder is partly loaded (no count is ever known). */
export interface TreePosition {
	readonly level: number;
	readonly posinset: number;
}

/** One visible row of the tree. */
export type TreeNode =
	| {
			readonly item: 'root';
			readonly key: string;
			readonly href: DenHref;
			readonly current: boolean;
			readonly expanded: boolean;
			readonly children: TreeChildren;
	  }
	| {
			readonly item: 'folder';
			readonly key: string;
			readonly href: DenHref;
			readonly entry: OpenEntry;
			readonly current: boolean;
			readonly expanded: boolean;
			readonly children: TreeChildren;
			readonly position: TreePosition | undefined;
	  }
	| {
			readonly item: 'leaf';
			readonly key: string;
			readonly href: DenHref;
			readonly entry: OpenEntry;
			readonly current: boolean;
			/** A folder whose content this read doesn't show: no arrow, a hint instead. */
			readonly notShown: boolean;
			readonly position: TreePosition | undefined;
	  }
	| {
			readonly item: 'card';
			readonly key: string;
			readonly entry: CardEntry;
			readonly position: TreePosition | undefined;
	  }
	| {
			readonly item: 'more';
			readonly key: string;
			readonly folder: DenHref;
			readonly href: DenHref;
			readonly state: 'idle' | 'loading' | 'failed';
			readonly position: TreePosition;
	  }
	| { readonly item: 'empty'; readonly key: string }
	| { readonly item: 'failed'; readonly key: string; readonly folder: DenHref };

/** What sits under an open folder. */
export type TreeChildren =
	| { readonly state: 'closed' }
	| { readonly state: 'loading' }
	| { readonly state: 'shown'; readonly nodes: readonly TreeNode[] };

/** The open item, as the tree places and highlights it. */
export interface TreeCurrent {
	/** The open item's link; absent on a page outside My Den or a card's page. */
	readonly href: DenHref | undefined;
	/** The folder it was opened in. */
	readonly parentHref: DenHref | undefined;
	/** The open item itself, for when it lies beyond its folder's loaded pages. */
	readonly entry: DenEntry | undefined;
}

/** What the tree is worked out from. */
export interface TreeInput {
	readonly rootHref: DenHref;
	readonly folders: ReadonlyMap<DenHref, FolderState>;
	readonly expanded: ReadonlySet<DenHref>;
	readonly current: TreeCurrent;
}

/** One listing row as a tree node at `level`. */
function entryNode(
	input: TreeInput,
	entry: DenEntry,
	key: string,
	level: number,
	position: TreePosition | undefined
): TreeNode {
	if (entry.view === 'card') return { item: 'card', key, entry, position };
	const current = entry.href === input.current.href;
	if (entry.kind === 'directory' && !entry.contentNotShown) {
		const expanded = input.expanded.has(entry.href);
		return {
			item: 'folder',
			key,
			href: entry.href,
			entry,
			current,
			expanded,
			children: expanded ? folderChildren(input, entry.href, level + 1) : { state: 'closed' },
			position
		};
	}
	return {
		item: 'leaf',
		key,
		href: entry.href,
		entry,
		current,
		notShown: entry.kind === 'directory' && entry.contentNotShown,
		position
	};
}

/** The key a row is known by: its link, or for a card its place in the folder. */
function entryKey(folder: DenHref, entry: DenEntry, index: number): string {
	return entry.view === 'open' ? entry.href : `${folder}#card-${String(index)}`;
}

/**
 * The rows under `folder`, at `level`: folders first, then the rest, each in
 * the server's order; a "More…" row when the server has another page; and
 * the open item after it when it lies beyond the loaded pages.
 */
function folderChildren(input: TreeInput, folder: DenHref, level: number): TreeChildren {
	const state = input.folders.get(folder);
	if (state === undefined || state.state === 'loading') return { state: 'loading' };
	if (state.state === 'failed') {
		return { state: 'shown', nodes: [{ item: 'failed', key: `${folder}#failed`, folder }] };
	}
	if (state.entries.length === 0) {
		return { state: 'shown', nodes: [{ item: 'empty', key: `${folder}#empty` }] };
	}
	const partly = state.more !== undefined;
	const ordered = foldersFirst(state.entries);
	const nodes: TreeNode[] = ordered.map((entry, index) =>
		entryNode(
			input,
			entry,
			entryKey(folder, entry, index),
			level,
			partly ? { level, posinset: index + 1 } : undefined
		)
	);
	if (state.more !== undefined) {
		nodes.push({
			item: 'more',
			key: `${folder}#more`,
			folder,
			href: state.more,
			state: state.moreState,
			position: { level, posinset: ordered.length + 1 }
		});
		const { current } = input;
		const stray = current.entry;
		const listed = state.entries.some(
			(entry) => entry.view === 'open' && entry.href === current.href
		);
		// Only an open item is placed as a stray: a card has no identity to match
		// against the loaded rows, so placing it would list it twice after "More…".
		if (stray?.view === 'open' && current.parentHref === folder && !listed) {
			nodes.push(
				entryNode(input, stray, entryKey(folder, stray, ordered.length), level, {
					level,
					posinset: ordered.length + 2
				})
			);
		}
	}
	return { state: 'shown', nodes };
}

/** The whole tree: the root `~`, and under it every open folder's rows. */
export function treeModel(input: TreeInput): TreeNode {
	const expanded = input.expanded.has(input.rootHref);
	return {
		item: 'root',
		key: input.rootHref,
		href: input.rootHref,
		current: input.current.href === input.rootHref,
		expanded,
		children: expanded ? folderChildren(input, input.rootHref, 2) : { state: 'closed' }
	};
}

/** Every row key the tree shows, in order: the roving focus falls back to the root when its key isn't among them. */
export function visibleKeys(node: TreeNode): readonly string[] {
	if (node.item !== 'root' && node.item !== 'folder') return [node.key];
	if (node.children.state !== 'shown') return [node.key];
	return [node.key, ...node.children.nodes.flatMap(visibleKeys)];
}
