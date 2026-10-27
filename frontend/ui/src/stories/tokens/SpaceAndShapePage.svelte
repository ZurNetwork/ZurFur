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
				<TokenSection {skin} name="spacing" heading="Spacing" tokens={tokensOf(model, 'spacing')}>
					{#snippet drawing(token: Token)}
						<span class="bar" style:width="var(--{token.name})"></span>
					{/snippet}
				</TokenSection>
				<TokenSection {skin} name="radius" heading="Radius" tokens={tokensOf(model, 'radius')}>
					{#snippet drawing(token: Token)}
						<span class="corner" style:border-radius="var(--{token.name})"></span>
					{/snippet}
				</TokenSection>
				<TokenSection {skin} name="size" heading="Size" tokens={tokensOf(model, 'size')}>
					{#snippet drawing(token: Token)}
						<span class="bar" style:width="var(--{token.name})"></span>
					{/snippet}
				</TokenSection>
				<TokenSection
					{skin}
					name="breakpoint"
					heading="Breakpoint"
					tokens={tokensOf(model, 'breakpoint')}
				/>
				<TokenSection {skin} name="blur" heading="Blur" tokens={tokensOf(model, 'blur')} />
				<TokenSection {skin} name="layer" heading="Layer" tokens={tokensOf(model, 'layer')} />
			{/snippet}
		</GalleryPage>
	{/snippet}
</TokenModelGate>

<style>
	.bar {
		display: block;
		max-width: 100%;
		height: var(--space-2);
		border-radius: var(--radius-xs);
		background: var(--color-accent);
	}

	.corner {
		display: block;
		width: var(--size-control-lg);
		height: var(--size-control-lg);
		border: 2px solid var(--color-accent-text);
		background: var(--color-surface);
	}
</style>
