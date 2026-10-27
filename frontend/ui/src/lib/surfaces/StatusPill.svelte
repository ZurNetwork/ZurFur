<script lang="ts">
	import Icon from '../icons/Icon.svelte';
	import { iconOf, statusToneOf } from './choices';
	import type { StatusFact, StatusParts } from './types';

	interface Props {
		/** One part, or two side by side; the pill never draws a third. */
		readonly parts: StatusParts;
		/** A quiet line beside the pill. */
		readonly fact?: StatusFact | undefined;
		readonly testid?: string | undefined;
	}

	let { parts, fact, testid }: Props = $props();

	const shownParts = $derived(parts.slice(0, 2));
	const factIcon = $derived(fact === undefined ? undefined : iconOf(fact.icon));
</script>

<span class="zf-status" data-testid={testid}>
	{#if shownParts.length > 0}
		<span class="zf-pill">
			{#each shownParts as part, index (index)}
				{@const icon = iconOf(part.icon)}
				<span class="zf-pill__seg zf-status--{statusToneOf(part.tone)}">
					{#if icon !== undefined}<Icon name={icon} size="sm" />{/if}
					<span>{part.label}</span>
				</span>
			{/each}
		</span>
	{/if}
	{#if fact !== undefined}
		<span class="zf-status__fact">
			{#if factIcon !== undefined}<Icon name={factIcon} size="sm" />{/if}
			<span>{fact.text}</span>
		</span>
	{/if}
</span>
