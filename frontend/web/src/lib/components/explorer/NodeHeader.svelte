<script lang="ts">
	import type { CardEntry, DenKind, OpenEntry } from '$lib/api/den';
	import Badge from '$lib/components/ui/Badge.svelte';
	import Icon from '$lib/components/ui/Icon.svelte';
	import type { IconName } from '$lib/components/ui/icons';
	import { removedWord } from '$lib/components/ui/removed-word';
	import VisibilityMarker from '$lib/components/ui/VisibilityMarker.svelte';

	/**
	 * The pane's title for the open node: its kind marker and name, then a
	 * line with its kind word, its own visibility, the mounted marker and the
	 * soft-delete word. A card shows only its name and "Can't open" — never a
	 * level, which only openers see. `root` titles My Den's root "My Den".
	 */
	let {
		node,
		root = false,
		heading = $bindable()
	}: {
		node: OpenEntry | CardEntry;
		root?: boolean;
		heading?: HTMLHeadingElement | undefined;
	} = $props();

	/** The word for each kind. */
	const KIND_WORDS = {
		directory: 'Folder',
		file: 'File',
		symlink: 'Link',
		unknown: 'Item'
	} as const satisfies Record<DenKind, string>;

	/** The marker for each kind. */
	const KIND_ICONS = {
		directory: 'folder-open',
		file: 'file',
		symlink: 'link',
		unknown: 'box'
	} as const satisfies Record<DenKind, IconName>;

	const title = $derived(root ? 'My Den' : node.name);
	const icon: IconName = $derived(node.view === 'open' ? KIND_ICONS[node.kind] : 'lock');
</script>

<header class="node-header" data-testid="node-header">
	<h1 class="node-header__title" tabindex="-1" bind:this={heading}>
		<Icon name={icon} />
		<span class="node-header__name"><bdi>{title}</bdi></span>
		{#if node.mounted}
			<span class="node-header__mounted" title="Mounted"><Icon name="corner-down-right" /></span>
		{/if}
	</h1>
	<p class="node-header__facts">
		{#if node.view === 'open'}
			<span data-testid="node-kind">{KIND_WORDS[node.kind]}</span>
			<VisibilityMarker level={node.ownLevel} />
			{#if node.mounted}<span>Mounted</span>{/if}
			{#if node.removed !== undefined}<Badge label={removedWord(node.removed)} />{/if}
		{:else}
			<span data-testid="node-kind">Can't open</span>
		{/if}
	</p>
</header>

<style>
	.node-header {
		margin-bottom: var(--space-4);
		padding-bottom: var(--space-3);
		border-bottom: var(--border-width) solid var(--color-border);
	}

	.node-header__title {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		margin-bottom: var(--space-1);
	}

	.node-header__title:focus {
		outline: none;
	}

	.node-header__title:focus-visible {
		outline: var(--focus-ring-width) solid var(--color-focus);
	}

	.node-header__name {
		min-width: 0;
		overflow-wrap: anywhere;
	}

	.node-header__mounted {
		display: inline-flex;
		color: var(--color-text-muted);
	}

	.node-header__facts {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		margin: 0;
		color: var(--color-text-muted);
		font-size: var(--text-sm);
	}
</style>
