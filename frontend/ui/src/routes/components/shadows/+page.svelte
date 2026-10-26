<script lang="ts">
	import type { Token, TokenModel } from '@zurfur/ui';
	import GalleryPage from '$gallery/GalleryPage.svelte';
	import TokenModelGate from '$gallery/TokenModelGate.svelte';
	import TokenSection from '$gallery/TokenSection.svelte';
	import { tokensOf } from '$gallery/token-source';
</script>

<TokenModelGate>
	{#snippet content(model: TokenModel)}
		<GalleryPage>
			{#snippet states()}
				<TokenSection name="shadow" heading="Shadow" tokens={tokensOf(model, 'shadow')}>
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
