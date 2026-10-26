<script lang="ts">
	import type { Skin, Token, TokenModel } from '@zurfur/ui';
	import GalleryPage from '../../gallery/GalleryPage.svelte';
	import TokenModelGate from '../../gallery/TokenModelGate.svelte';
	import TokenSection from '../../gallery/TokenSection.svelte';
	import { tokensOf } from '../../gallery/token-source';

	interface Props {
		/** Shows only this skin, as the toolbar picks it; unset shows both side by side. */
		readonly skin?: Skin | undefined;
	}

	let { skin }: Props = $props();
</script>

<TokenModelGate>
	{#snippet content(model: TokenModel)}
		<GalleryPage framed={skin !== undefined}>
			{#snippet states()}
				<TokenSection {skin} name="shadow" heading="Shadow" tokens={tokensOf(model, 'shadow')}>
					{#snippet drawing(token: Token)}
						<span class="sheet" style:box-shadow="var(--{token.name})"></span>
					{/snippet}
				</TokenSection>
			{/snippet}
		</GalleryPage>
	{/snippet}
</TokenModelGate>

<style>
	.sheet {
		display: block;
		width: var(--size-rail);
		height: var(--size-touch);
		margin: var(--space-2);
		border-radius: var(--radius-md);
		background: var(--color-sheet);
	}
</style>
