<script lang="ts">
	import Icon from '$lib/components/ui/Icon.svelte';
	import Skeleton from '$lib/components/ui/Skeleton.svelte';
	import TreeItem from '$lib/components/ui/TreeItem.svelte';
	import type { DenHref } from '$lib/types/brand';
	import TreeBranch from './TreeBranch.svelte';
	import type { TreeNode } from './tree-model';

	/**
	 * One tree row and, when it is an open folder, the group under it. Before
	 * the page's script runs (`enhanced` false) it is a plain list of links:
	 * no tree roles and no roving tabindex. Once enhanced it follows the W3C
	 * navigation treeview: the link is the tree item, its group a sibling it
	 * owns, and only the active row is in the Tab order.
	 */
	let {
		node,
		parentKey,
		enhanced,
		activeKey,
		idPrefix,
		onToggle,
		onActivate
	}: {
		node: TreeNode;
		parentKey: string | undefined;
		enhanced: boolean;
		activeKey: string;
		idPrefix: string;
		onToggle: (folder: DenHref) => void;
		onActivate: (node: TreeNode) => void;
	} = $props();

	const tabindex = $derived(enhanced ? (node.key === activeKey ? 0 : -1) : undefined);
	const groupId = $derived(`${idPrefix}-group-${node.key}`);
	const role = $derived(enhanced ? 'treeitem' : undefined);
	const position = $derived(
		node.item === 'folder' || node.item === 'leaf' || node.item === 'card' || node.item === 'more'
			? node.position
			: undefined
	);
	const expandable = $derived(node.item === 'root' || node.item === 'folder');
	const expanded = $derived(node.item === 'root' || node.item === 'folder' ? node.expanded : false);
	const children = $derived(
		node.item === 'root' || node.item === 'folder' ? node.children : { state: 'closed' as const }
	);
	const typeaheadName = $derived.by(() => {
		switch (node.item) {
			case 'root':
				return 'my den';
			case 'folder':
			case 'leaf':
			case 'card':
				return node.entry.name.toLowerCase();
			case 'more':
				return 'more';
			case 'empty':
				return 'empty';
			case 'failed':
				return "couldn't load";
		}
	});

	/** The arrow beside a folder opens or closes it; it never follows the link. */
	function arrowClick(event: MouseEvent, folder: DenHref): void {
		event.preventDefault();
		event.stopPropagation();
		onToggle(folder);
	}
</script>

<li role={enhanced ? 'none' : undefined} class="tree-branch">
	{#if node.item === 'root' || node.item === 'folder' || node.item === 'leaf'}
		<!-- A DenHref is built (and resolved) by the server's Den path builder. -->
		<!-- eslint-disable svelte/no-navigation-without-resolve -->
		<a
			class="tree-row"
			class:tree-row--current={node.current}
			href={node.href}
			{role}
			{tabindex}
			aria-current={node.current ? 'page' : undefined}
			aria-expanded={enhanced && expandable ? expanded : undefined}
			aria-owns={enhanced && expandable && expanded && children.state !== 'closed'
				? groupId
				: undefined}
			aria-level={enhanced ? position?.level : undefined}
			aria-posinset={enhanced ? position?.posinset : undefined}
			aria-setsize={enhanced && position !== undefined ? -1 : undefined}
			data-tree-key={node.key}
			data-tree-parent={parentKey}
			data-tree-kind={node.item}
			data-tree-expanded={expandable ? String(expanded) : undefined}
			data-tree-name={typeaheadName}
			data-sveltekit-keepfocus
			data-sveltekit-preload-data="tap"
		>
			<!-- eslint-enable svelte/no-navigation-without-resolve -->
			{#if expandable}
				<!-- The arrow is a mouse target only; the keyboard opens and closes with Right and Left. -->
				<span
					class="tree-row__arrow"
					class:tree-row__arrow--open={expanded}
					aria-hidden="true"
					data-testid="tree-arrow"
					onclick={(event) => {
						arrowClick(event, node.href);
					}}><Icon name="chevron-right" /></span
				>
			{:else}
				<span class="tree-row__arrow tree-row__arrow--none" aria-hidden="true"></span>
			{/if}
			{#if node.item === 'root'}
				<TreeItem look="root" name="My Den" open={expanded} />
			{:else if node.item === 'folder' || node.item === 'leaf'}
				<TreeItem
					look={node.entry.kind}
					name={node.entry.name}
					open={expanded}
					mounted={node.entry.mounted}
					removed={node.entry.removed}
					notShown={node.item === 'leaf' && node.notShown}
				/>
			{/if}
		</a>
	{:else if node.item === 'card'}
		<!-- The role is `treeitem` whenever the tabindex is set (both arrive with the page's script). -->
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<span
			class="tree-row tree-row--inert"
			{role}
			{tabindex}
			aria-level={enhanced ? position?.level : undefined}
			aria-posinset={enhanced ? position?.posinset : undefined}
			aria-setsize={enhanced && position !== undefined ? -1 : undefined}
			data-tree-key={node.key}
			data-tree-parent={parentKey}
			data-tree-kind="card"
			data-tree-name={typeaheadName}
		>
			<span class="tree-row__arrow tree-row__arrow--none" aria-hidden="true"></span>
			<TreeItem look="cantOpen" name={node.entry.name} mounted={node.entry.mounted} />
		</span>
	{:else if node.item === 'more' && !enhanced}
		<!-- Before the page's script runs, "More…" is a plain link to the next page. -->
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- a DenHref is built (and resolved) by the server's Den path builder -->
		<a class="tree-row tree-row--action" href={node.href} data-tree-kind="more">More…</a>
	{:else if node.item === 'more' || node.item === 'failed'}
		<!-- Activated by Enter or Space through the tree's own key handling. -->
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<span
			class="tree-row tree-row--action"
			{role}
			{tabindex}
			aria-busy={node.item === 'more' && node.state === 'loading' ? 'true' : undefined}
			aria-level={enhanced ? position?.level : undefined}
			aria-posinset={enhanced ? position?.posinset : undefined}
			aria-setsize={enhanced && position !== undefined ? -1 : undefined}
			data-tree-key={node.key}
			data-tree-parent={parentKey}
			data-tree-kind={node.item}
			data-tree-name={typeaheadName}
			onclick={() => {
				onActivate(node);
			}}
		>
			<span class="tree-row__arrow tree-row__arrow--none" aria-hidden="true"></span>
			{#if node.item === 'failed'}
				Couldn't load. Retry.
			{:else if node.state === 'failed'}
				Couldn't load more. Retry.
			{:else if node.state === 'loading'}
				Loading more…
			{:else}
				More…
			{/if}
		</span>
	{:else}
		<!-- The role is `treeitem` whenever the tabindex is set (both arrive with the page's script). -->
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<span
			class="tree-row tree-row--inert tree-row--muted"
			{role}
			{tabindex}
			aria-disabled={enhanced ? 'true' : undefined}
			data-tree-key={node.key}
			data-tree-parent={parentKey}
			data-tree-kind="empty"
			data-tree-name={typeaheadName}
		>
			<span class="tree-row__arrow tree-row__arrow--none" aria-hidden="true"></span>
			Empty
		</span>
	{/if}

	{#if children.state !== 'closed'}
		<ul
			class="tree-group"
			id={groupId}
			role={enhanced ? 'group' : undefined}
			aria-busy={children.state === 'loading' ? 'true' : undefined}
		>
			{#if children.state === 'loading'}
				<li role={enhanced ? 'none' : undefined} class="tree-group__loading">
					<Skeleton rows={2} />
				</li>
			{:else}
				{#each children.nodes as child (child.key)}
					<TreeBranch
						node={child}
						parentKey={node.key}
						{enhanced}
						{activeKey}
						{idPrefix}
						{onToggle}
						{onActivate}
					/>
				{/each}
			{/if}
		</ul>
	{/if}
</li>

<style>
	.tree-branch {
		list-style: none;
	}

	.tree-group {
		margin: 0;
		padding: 0 0 0 var(--tree-indent);
	}

	.tree-group__loading {
		padding: 0 var(--space-2) 0 calc(var(--space-2) + var(--space-5));
	}

	.tree-row {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		min-height: var(--row-height);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		color: var(--color-text);
		font-size: var(--text-sm);
		text-decoration: none;
		cursor: pointer;
	}

	.tree-row:hover {
		background: var(--color-hover);
	}

	.tree-row--current {
		background: var(--color-selected);
		box-shadow: inset var(--space-1) 0 0 var(--color-accent);
	}

	.tree-row--inert {
		cursor: default;
	}

	.tree-row--muted,
	.tree-row--action {
		color: var(--color-text-muted);
	}

	.tree-row__arrow {
		display: inline-flex;
		flex: none;
		align-items: center;
		justify-content: center;
		width: var(--space-5);
		height: var(--space-5);
		color: var(--color-text-muted);
		transition: transform var(--duration-fast) ease;
	}

	.tree-row__arrow--open {
		transform: rotate(90deg);
	}

	@media (forced-colors: active) {
		.tree-row--current {
			outline: 1px solid Highlight;
		}
	}
</style>
