<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Skin, Token } from '@zurfur/ui';
	import GallerySkins from './GallerySkins.svelte';
	import GalleryState from './GalleryState.svelte';
	import TokenList from './TokenList.svelte';

	interface Props {
		/** The state's name, kebab-case. */
		readonly name: string;
		readonly heading: string;
		readonly tokens: readonly Token[];
		readonly drawing?: Snippet<[Token, Skin]> | undefined;
		/** Shows only this skin; unset shows both side by side. */
		readonly skin?: Skin | undefined;
	}

	let { name, heading, tokens, drawing, skin }: Props = $props();
</script>

<GalleryState {name}>
	<h3>{heading}</h3>
	<GallerySkins only={skin}>
		{#snippet children(skin)}
			<TokenList {tokens} {skin} {drawing} />
		{/snippet}
	</GallerySkins>
</GalleryState>
