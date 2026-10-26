<script lang="ts">
	import type { Token, TokenModel, TypeStyle } from '@zurfur/ui';
	import FontFaces from '$gallery/FontFaces.svelte';
	import GalleryPage from '$gallery/GalleryPage.svelte';
	import GallerySkins from '$gallery/GallerySkins.svelte';
	import GalleryState from '$gallery/GalleryState.svelte';
	import TokenModelGate from '$gallery/TokenModelGate.svelte';
	import TokenSection from '$gallery/TokenSection.svelte';
	import { tokensOf } from '$gallery/token-source';

	/** A style's numbers on one line: size over line height, weight, and letter spacing when it has any. */
	function numbersOf(style: TypeStyle): string {
		const parts = [`${style.fontSize} / ${style.lineHeight}`, String(style.fontWeight)];
		if (style.letterSpacing !== undefined) parts.push(style.letterSpacing);
		return parts.join(', ');
	}
</script>

<TokenModelGate>
	{#snippet content(model: TokenModel)}
		<GalleryPage>
			{#snippet states()}
				<GalleryState name="font-load">
					<h3>Fonts</h3>
					<GallerySkins>
						<FontFaces fonts={model.type.fonts} />
					</GallerySkins>
				</GalleryState>
				<TokenSection name="text-tokens" heading="Text" tokens={tokensOf(model, 'text')}>
					{#snippet drawing(token: Token)}
						<span class="aa" style:font-size="var(--{token.name})">Aa</span>
					{/snippet}
				</TokenSection>
				<TokenSection name="leading-tokens" heading="Leading" tokens={tokensOf(model, 'leading')} />
				<TokenSection name="weight-tokens" heading="Weight" tokens={tokensOf(model, 'weight')}>
					{#snippet drawing(token: Token)}
						<span class="aa" style:font-weight="var(--{token.name})">Aa</span>
					{/snippet}
				</TokenSection>
				<GalleryState name="type-styles">
					<h3>Styles</h3>
					<GallerySkins>
						<ul class="styles">
							{#each model.type.groups as group (group.name)}
								{#each group.styles as style (style.name)}
									<li data-type-style={style.name}>
										<p
											class="sample"
											style:font-family="var(--font-{group.family})"
											style:font-size={style.fontSize}
											style:line-height={style.lineHeight}
											style:font-weight={style.fontWeight}
											style:letter-spacing={style.letterSpacing}
										>
											{style.sample}
										</p>
										<p class="words">
											<code>{style.name}</code>
											<code class="muted">{numbersOf(style)}</code>
										</p>
										<p class="words">{style.usage}</p>
									</li>
								{/each}
							{/each}
						</ul>
					</GallerySkins>
				</GalleryState>
			{/snippet}
		</GalleryPage>
	{/snippet}
</TokenModelGate>

<style>
	.aa {
		display: block;
		min-width: var(--size-control-lg);
		text-align: center;
	}

	.styles {
		display: grid;
		gap: var(--space-4);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.sample {
		margin: 0;
		overflow-wrap: anywhere;
	}

	.words {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		margin: var(--space-half) 0 0;
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}

	.muted {
		color: var(--color-text-muted);
	}
</style>
