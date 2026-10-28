<script lang="ts">
	import type { Skin, Token, TokenModel } from '@zurfur/ui';
	import { backgroundLabel, contrastReport } from '../../gallery/contrast/contrast-pairs';
	import ContrastTable from '../../gallery/ContrastTable.svelte';
	import GalleryPage from '../../gallery/GalleryPage.svelte';
	import GallerySkins from '../../gallery/GallerySkins.svelte';
	import GalleryState from '../../gallery/GalleryState.svelte';
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
				<TokenSection
					{skin}
					name="colour-tokens"
					heading="Colours"
					tokens={tokensOf(model, 'color')}
				>
					{#snippet drawing(token: Token)}
						<span class="swatch" style:background-color="var(--{token.name})"></span>
					{/snippet}
				</TokenSection>
				<GalleryState name="contrast-table">
					<h3>Contrast</h3>
					<GallerySkins only={skin}>
						{#snippet children(skin: Skin)}
							<ContrastTable report={contrastReport(model, skin)} />
						{/snippet}
					</GallerySkins>
				</GalleryState>
				<GalleryState name="filled-parts">
					<h3>Filled</h3>
					<GallerySkins only={skin}>
						{#snippet children(skin: Skin)}
							<ul class="entries">
								{#each contrastReport(model, skin).filled as entry (entry.token)}
									<li data-filled={entry.token}>
										<span class="swatch" style:background-color="var(--{entry.token})"></span>
										<div class="words">
											<code>{entry.token}</code>
											<code class="muted">{entry.value}</code>
											<span>{entry.part}</span>
										</div>
									</li>
								{/each}
							</ul>
						{/snippet}
					</GallerySkins>
				</GalleryState>
				<GalleryState name="decorative-tokens">
					<h3>Decorative</h3>
					<GallerySkins only={skin}>
						{#snippet children(skin: Skin)}
							<ul class="entries">
								{#each contrastReport(model, skin).decorative as entry (entry.token)}
									<li data-decorative={entry.token}>
										<span class="swatch" style:background-color="var(--{entry.token})"></span>
										<div class="words">
											<code>{entry.token}</code>
											<code class="muted">{entry.value}</code>
											<span>{entry.why}</span>
										</div>
									</li>
								{/each}
							</ul>
						{/snippet}
					</GallerySkins>
				</GalleryState>
				<GalleryState name="excluded-pairs">
					<h3>Excluded</h3>
					<GallerySkins only={skin}>
						{#snippet children(skin: Skin)}
							<ul class="entries">
								{#each contrastReport(model, skin).excluded as entry, index (index)}
									<li data-excluded>
										<div class="words">
											<code>{entry.foreground}</code>
											<code>{backgroundLabel(entry.background)}</code>
											<span>{entry.reason}</span>
										</div>
									</li>
								{/each}
							</ul>
						{/snippet}
					</GallerySkins>
				</GalleryState>
			{/snippet}
		</GalleryPage>
	{/snippet}
</TokenModelGate>

<style>
	.swatch {
		display: block;
		flex: none;
		width: var(--size-control-lg);
		height: var(--size-control-lg);
		border: 1px solid var(--color-border-strong);
		border-radius: var(--radius-sm);
	}

	.entries {
		display: grid;
		gap: var(--space-3);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.entries li {
		display: flex;
		gap: var(--space-3);
		align-items: center;
	}

	.words {
		display: grid;
		gap: var(--space-half);
		min-width: 0;
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
		overflow-wrap: anywhere;
	}

	.muted {
		color: var(--color-text-muted);
	}
</style>
