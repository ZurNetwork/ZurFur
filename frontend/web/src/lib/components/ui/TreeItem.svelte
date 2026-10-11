<script lang="ts">
	import type { DenKind, DenRemoved } from '$lib/api/den';
	import Badge from './Badge.svelte';
	import Icon from './Icon.svelte';
	import type { IconName } from './icons';
	import { removedWord } from './removed-word';

	/** What a row shows as: one of the node kinds, the root, or an item the viewer can't open. */
	type TreeItemLook = DenKind | 'root' | 'cantOpen';

	/**
	 * The face of one tree row or list row: its kind marker, its name (cut
	 * with an ellipsis, isolated for right-to-left text), the mounted marker,
	 * the soft-delete word and the "not shown yet" hint. The interactive
	 * element around it (link, tree item) belongs to the caller.
	 */
	let {
		look,
		name,
		open = false,
		mounted = false,
		removed,
		notShown = false,
		sayCantOpen = true
	}: {
		look: TreeItemLook;
		name: string;
		open?: boolean;
		mounted?: boolean;
		removed?: DenRemoved | undefined;
		notShown?: boolean;
		/** Whether to tell screen readers "Can't be opened" (off where the row already says so). */
		sayCantOpen?: boolean;
	} = $props();

	/** The marker each look gets; an open folder shows as open. */
	const icon: IconName = $derived.by(() => {
		switch (look) {
			case 'directory':
			case 'root':
				return open ? 'folder-open' : 'folder';
			case 'file':
				return 'file';
			case 'symlink':
				return 'link';
			case 'cantOpen':
				return 'lock';
			case 'unknown':
				return 'box';
		}
	});
</script>

<span class="tree-item" class:tree-item--muted={look === 'cantOpen'}>
	<span class="tree-item__icon"><Icon name={icon} /></span>
	{#if look === 'root'}
		<span class="tree-item__name" aria-hidden="true">~</span><span class="visually-hidden"
			>My Den</span
		>
	{:else}
		<span class="tree-item__name" title={name}><bdi>{name}</bdi></span>
	{/if}
	{#if mounted}
		<span class="tree-item__mounted" title="Mounted" data-testid="mounted-marker"
			><Icon name="corner-down-right" /><span class="visually-hidden">Mounted</span></span
		>
	{/if}
	{#if removed !== undefined}
		<Badge label={removedWord(removed)} />
	{/if}
	{#if notShown}
		<span class="tree-item__hint">not shown yet</span>
	{/if}
	{#if look === 'cantOpen' && sayCantOpen}
		<span class="visually-hidden">Can't be opened</span>
	{/if}
</span>

<style>
	.tree-item {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}

	.tree-item__icon {
		display: inline-flex;
		flex: none;
		color: var(--color-text-muted);
	}

	.tree-item__name {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.tree-item--muted {
		color: var(--color-text-muted);
	}

	.tree-item__mounted {
		display: inline-flex;
		flex: none;
		color: var(--color-text-muted);
	}

	.tree-item__hint {
		flex: none;
		color: var(--color-text-muted);
		font-size: var(--text-xs);
	}
</style>
