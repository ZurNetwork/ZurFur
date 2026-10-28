<script lang="ts">
	import Icon from '../icons/Icon.svelte';
	import type { IconName } from '../icons/icons';
	import { badgeToneOf, iconOf } from './choices';
	import type { BadgeTone } from './types';

	interface Props {
		/** The words, as text; a long one is cut short. */
		readonly label: string;
		readonly tone?: BadgeTone;
		readonly icon?: IconName | undefined;
		readonly testid?: string | undefined;
	}

	let { label, tone = 'neutral', icon, testid }: Props = $props();

	const shownTone = $derived(badgeToneOf(tone));
	const shownIcon = $derived(iconOf(icon));
</script>

<span class="zf-badge zf-badge--{shownTone}" data-testid={testid}>
	{#if shownIcon !== undefined}<Icon name={shownIcon} size="sm" />{/if}
	<span>{label}</span>
</span>
