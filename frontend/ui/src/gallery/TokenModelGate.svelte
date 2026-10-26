<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ParseOutcome, TokenModel } from '@zurfur/ui';
	import { tokenOutcome } from './token-source';

	interface Props {
		/** Rendered with the token model when the token files parse. */
		readonly content: Snippet<[TokenModel]>;
		/** The parse to show; the gallery's own by default. */
		readonly outcome?: ParseOutcome | undefined;
	}

	let { content, outcome = tokenOutcome }: Props = $props();
</script>

{#if outcome.kind === 'parsed'}
	{@render content(outcome.model)}
{:else}
	<ul data-token-refusals>
		{#each outcome.refusals as refusal, index (index)}
			<li><code>{refusal.where}</code> <code>{refusal.reason}</code> {refusal.detail}</li>
		{/each}
	</ul>
{/if}
