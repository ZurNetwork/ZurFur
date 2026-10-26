<script lang="ts">
	import type { Skin, Token, TokenModel } from '@zurfur/ui';
	import EasingCurve from '../../gallery/EasingCurve.svelte';
	import GalleryPage from '../../gallery/GalleryPage.svelte';
	import GallerySkins from '../../gallery/GallerySkins.svelte';
	import GalleryState from '../../gallery/GalleryState.svelte';
	import TokenModelGate from '../../gallery/TokenModelGate.svelte';
	import TokenSection from '../../gallery/TokenSection.svelte';
	import UsageText from '../../gallery/UsageText.svelte';
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
					name="duration"
					heading="Duration"
					tokens={tokensOf(model, 'duration')}
				/>
				<TokenSection {skin} name="easing" heading="Easing" tokens={tokensOf(model, 'easing')}>
					{#snippet drawing(token: Token, skin: Skin)}
						<EasingCurve easing={token.resolved[skin]} label={token.name} />
					{/snippet}
				</TokenSection>
				<GalleryState name="sound">
					<h3>Sound</h3>
					<GallerySkins only={skin}>
						{#snippet children(skin: Skin)}
							<table>
								<thead>
									<tr>
										<th scope="col">Name</th>
										<th scope="col">Value</th>
										<th scope="col">Usage</th>
									</tr>
								</thead>
								<tbody>
									{#each tokensOf(model, 'sound') as token (token.name)}
										<tr data-token={token.name}>
											<td><code>{token.name}</code></td>
											<td><code>{token.resolved[skin]}</code></td>
											<td><UsageText text={token.usage} /></td>
										</tr>
									{/each}
								</tbody>
							</table>
						{/snippet}
					</GallerySkins>
				</GalleryState>
			{/snippet}
		</GalleryPage>
	{/snippet}
</TokenModelGate>

<style>
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-sm);
		line-height: var(--leading-sm);
	}

	th,
	td {
		padding: var(--space-1) var(--space-2);
		border-bottom: 1px solid var(--color-border);
		text-align: start;
		vertical-align: top;
		overflow-wrap: anywhere;
	}

	th {
		font-weight: var(--weight-semibold);
	}
</style>
