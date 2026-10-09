<script lang="ts">
	import { onMount, tick } from 'svelte';
	import type { DenHref } from '$lib/types/brand';
	import { loadFolder, loadMore, type Preload } from './tree-loader';
	import type { TreeMemory } from './tree-memory.svelte';
	import { treeModel, visibleKeys, type TreeCurrent, type TreeNode } from './tree-model';
	import TreeBranch from './TreeBranch.svelte';

	/**
	 * My Den's folder tree, labelled "My Den". Before the page's script runs
	 * it is nested lists of links; once it mounts it becomes a W3C tree:
	 * Up/Down move, Right opens a folder (or steps into it), Left closes it
	 * (or steps out), Home/End jump, a letter jumps to the next name, and
	 * Enter or Space opens the item in the pane. Clicking a name opens it;
	 * only the arrow opens or closes a folder. The one Tab stop is the open
	 * item (or the row last focused while focus stays in the tree).
	 */
	let {
		memory,
		current,
		fallbackRootHref,
		preload
	}: {
		memory: TreeMemory;
		current: TreeCurrent;
		fallbackRootHref: DenHref;
		preload?: Preload | undefined;
	} = $props();

	const idPrefix = $props.id();
	let enhanced = $state(false);
	let treeElement: HTMLUListElement | undefined = $state();
	let focusedKey: string | undefined = $state();

	onMount(() => {
		enhanced = true;
	});

	const rootHref = $derived(memory.rootHref ?? fallbackRootHref);
	const model = $derived(
		treeModel({ rootHref, folders: memory.folders, expanded: memory.expanded, current })
	);
	const keys = $derived(visibleKeys(model));
	const homeKey = $derived(
		current.href !== undefined && keys.includes(current.href) ? current.href : model.key
	);
	const activeKey = $derived(
		focusedKey !== undefined && keys.includes(focusedKey) ? focusedKey : homeKey
	);

	/** Every tree row in the DOM, in visible order. */
	function rows(): HTMLElement[] {
		if (treeElement === undefined) return [];
		return [...treeElement.querySelectorAll<HTMLElement>('[data-tree-key]')];
	}

	/** The row for `key`, if it is shown. */
	function rowFor(key: string): HTMLElement | undefined {
		return rows().find((row) => row.dataset.treeKey === key);
	}

	/** Open or close `folder`, fetching its listing the first time it opens. */
	async function toggle(folder: DenHref): Promise<void> {
		if (memory.expanded.has(folder)) {
			const tree = treeElement;
			const focusWasInside = tree?.contains(document.activeElement) ?? false;
			memory.expanded.delete(folder);
			await tick();
			// Closing a folder can remove the focused row; focus goes to the folder.
			const focusIsInside = tree?.contains(document.activeElement) ?? false;
			if (focusWasInside && !focusIsInside) rowFor(folder)?.focus();
			return;
		}
		memory.expanded.add(folder);
		const state = memory.folders.get(folder);
		if (state === undefined || state.state === 'failed') await loadFolder(memory, folder, preload);
	}

	/**
	 * Fetch the next page under "More…", then move focus to its first new row.
	 * A page the pane appended first is dropped here, and focus stays put.
	 */
	async function more(folder: DenHref, href: DenHref): Promise<void> {
		const childOf = (row: HTMLElement) => row.dataset.treeParent === folder;
		const before = new Set(
			rows()
				.filter(childOf)
				.map((row) => row.dataset.treeKey)
		);
		const appended = await loadMore(memory, folder, href, preload);
		if (!appended) return;
		await tick();
		const first = rows().find(
			(row) => childOf(row) && !before.has(row.dataset.treeKey) && row.dataset.treeKind !== 'more'
		);
		first?.focus();
	}

	/**
	 * Fetch a failed folder again. Its "Retry" row gives way to the loading
	 * rows, so focus waits on the folder, and returns to "Retry" if it fails again.
	 */
	async function retry(folder: DenHref): Promise<void> {
		const focusWasInside = treeElement?.contains(document.activeElement) ?? false;
		const loading = loadFolder(memory, folder, preload);
		await tick();
		if (!focusWasInside) return;
		const folderRow = rowFor(folder);
		folderRow?.focus();
		await loading;
		await tick();
		if (document.activeElement === folderRow) rowFor(`${folder}#failed`)?.focus();
	}

	/** A "More…" or "Retry" row was activated. */
	function activate(node: TreeNode): void {
		if (node.item === 'more' && node.state !== 'loading') {
			void more(node.folder, node.href);
		} else if (node.item === 'failed') {
			void retry(node.folder);
		}
	}

	/** The node behind a row, found by key in the model. */
	function nodeFor(key: string, node: TreeNode = model): TreeNode | undefined {
		if (node.key === key) return node;
		if ((node.item === 'root' || node.item === 'folder') && node.children.state === 'shown') {
			for (const child of node.children.nodes) {
				const found = nodeFor(key, child);
				if (found !== undefined) return found;
			}
		}
		return undefined;
	}

	/** Move focus to the row at `index`, if there is one. */
	function focusAt(list: readonly HTMLElement[], index: number): void {
		list[index]?.focus();
	}

	/** The next row after `index` whose name starts with `letter`, wrapping round. */
	function typeahead(list: readonly HTMLElement[], index: number, letter: string): void {
		const wanted = letter.toLowerCase();
		for (let step = 1; step <= list.length; step += 1) {
			const candidate = list[(index + step) % list.length];
			if (candidate?.dataset.treeName?.startsWith(wanted) === true) {
				candidate.focus();
				return;
			}
		}
	}

	/** Right: open a closed folder; on an open one, step to its first child. */
	function arrowRight(list: readonly HTMLElement[], index: number, row: HTMLElement): void {
		const key = row.dataset.treeKey;
		const node = key === undefined ? undefined : nodeFor(key);
		if (node === undefined || (node.item !== 'root' && node.item !== 'folder')) return;
		if (!node.expanded) {
			void toggle(node.href);
			return;
		}
		const next = list[index + 1];
		if (next !== undefined && key !== undefined && next.dataset.treeParent === key) next.focus();
	}

	/** Left: close an open folder; otherwise step out to the parent. */
	function arrowLeft(row: HTMLElement): void {
		const key = row.dataset.treeKey;
		const node = key === undefined ? undefined : nodeFor(key);
		if (node !== undefined && (node.item === 'root' || node.item === 'folder') && node.expanded) {
			void toggle(node.href);
			return;
		}
		const parent = row.dataset.treeParent;
		if (parent !== undefined) rowFor(parent)?.focus();
	}

	/** Enter or Space: open a linked row in the pane, or activate an action row. */
	function choose(event: KeyboardEvent, row: HTMLElement): void {
		if (row instanceof HTMLAnchorElement) {
			// Enter already follows a link natively; Space doesn't, so it clicks.
			if (event.key === ' ') {
				event.preventDefault();
				row.click();
			}
			return;
		}
		event.preventDefault();
		const key = row.dataset.treeKey;
		const node = key === undefined ? undefined : nodeFor(key);
		if (node !== undefined) activate(node);
	}

	function onkeydown(event: KeyboardEvent): void {
		if (!enhanced || event.altKey || event.ctrlKey || event.metaKey) return;
		const row = event.target;
		if (!(row instanceof HTMLElement) || row.dataset.treeKey === undefined) return;
		const list = rows();
		const index = list.indexOf(row);
		const handled = ['ArrowDown', 'ArrowUp', 'ArrowRight', 'ArrowLeft', 'Home', 'End'];
		if (handled.includes(event.key)) event.preventDefault();
		switch (event.key) {
			case 'ArrowDown':
				focusAt(list, index + 1);
				return;
			case 'ArrowUp':
				focusAt(list, index - 1);
				return;
			case 'Home':
				focusAt(list, 0);
				return;
			case 'End':
				focusAt(list, list.length - 1);
				return;
			case 'ArrowRight':
				arrowRight(list, index, row);
				return;
			case 'ArrowLeft':
				arrowLeft(row);
				return;
			case 'Enter':
			case ' ':
				choose(event, row);
				return;
			default:
				if (event.key.length === 1 && event.key.trim() !== '') {
					event.preventDefault();
					typeahead(list, index, event.key);
				}
		}
	}

	function onfocusin(event: FocusEvent): void {
		const row = event.target;
		if (row instanceof HTMLElement && row.dataset.treeKey !== undefined) {
			focusedKey = row.dataset.treeKey;
		}
	}

	function onfocusout(event: FocusEvent): void {
		const next = event.relatedTarget;
		if (!(next instanceof Node) || treeElement?.contains(next) !== true) focusedKey = undefined;
	}
</script>

<ul
	bind:this={treeElement}
	class="tree"
	role={enhanced ? 'tree' : undefined}
	aria-label="My Den"
	data-testid="tree"
	{onkeydown}
	{onfocusin}
	{onfocusout}
>
	<TreeBranch
		node={model}
		parentKey={undefined}
		{enhanced}
		{activeKey}
		{idPrefix}
		onToggle={(folder: DenHref) => {
			void toggle(folder);
		}}
		onActivate={activate}
	/>
</ul>

<style>
	.tree {
		margin: 0;
		padding: 0 var(--space-2);
	}
</style>
