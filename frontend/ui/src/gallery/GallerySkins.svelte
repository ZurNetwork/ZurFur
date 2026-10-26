<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Skin } from '../lib/styles/environment.ts';

	interface Props {
		/** Rendered once per skin, and handed the skin it is rendered in. */
		readonly children: Snippet<[Skin]>;
	}

	let { children }: Props = $props();

	interface Column {
		readonly skin: Skin;
		readonly caption: string;
	}

	const columns: readonly Column[] = [
		{ skin: 'dark', caption: 'Paper dark' },
		{ skin: 'light', caption: 'Paper light' }
	];
</script>

<div class="gallery-skins">
	{#each columns as column (column.skin)}
		<figure>
			<figcaption>{column.caption}</figcaption>
			<div class="copy" data-skin={column.skin} data-gallery-skin={column.skin}>
				{@render children(column.skin)}
			</div>
		</figure>
	{/each}
</div>

<style>
	.gallery-skins {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--space-4);
	}

	figure {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		margin: 0;
		min-width: 0;
	}

	figcaption {
		color: var(--color-text-muted);
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}

	.copy {
		flex: 1;
		padding: var(--space-4);
		border-radius: var(--radius-sheet);
		background: var(--color-bg);
		color: var(--color-text);
	}
</style>
