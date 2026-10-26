<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Skin, Token } from '@zurfur/ui';
	import UsageText from './UsageText.svelte';

	interface Props {
		readonly tokens: readonly Token[];
		/** The skin whose value each token shows. */
		readonly skin: Skin;
		/** A small drawing of the token, shown before its words. */
		readonly drawing?: Snippet<[Token, Skin]> | undefined;
	}

	let { tokens, skin, drawing }: Props = $props();
</script>

<ul class="rows">
	{#each tokens as token (token.name)}
		<li class="row" data-token={token.name}>
			{#if drawing}
				<div class="drawing">{@render drawing(token, skin)}</div>
			{/if}
			<div class="words">
				<code class="name">{token.name}</code>
				<code class="value">{token.resolved[skin]}</code>
				<p class="usage"><UsageText text={token.usage} /></p>
			</div>
		</li>
	{/each}
</ul>

<style>
	.rows {
		display: grid;
		gap: var(--space-3);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.row {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-3);
		align-items: center;
		min-width: 0;
	}

	.drawing {
		max-width: 100%;
		overflow: hidden;
	}

	.words {
		display: grid;
		flex: 1 1 var(--size-rail);
		gap: var(--space-half);
		min-width: 0;
	}

	.name {
		font-weight: var(--weight-semibold);
	}

	.value {
		color: var(--color-text-muted);
		overflow-wrap: anywhere;
	}

	.usage {
		margin: 0;
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}
</style>
