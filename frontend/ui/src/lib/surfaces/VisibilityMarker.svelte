<script lang="ts">
	import Icon from '../icons/Icon.svelte';
	import { visibilityIconOf, visibilityOf } from './choices';
	import type { VisibilityLevel, VisibilityWords } from './types';

	interface Props {
		readonly level: VisibilityLevel;
		/** The word for each level; the marker shows the one its level names. */
		readonly words: VisibilityWords;
		/** The icon alone on the page; the word is still read out, once. */
		readonly compact?: boolean;
		readonly testid?: string | undefined;
	}

	let { level, words, compact = false, testid }: Props = $props();

	const shownLevel = $derived(visibilityOf(level));
</script>

{#if shownLevel !== undefined}
	<span
		class={['zf-vis', `zf-vis--${shownLevel}`, compact && 'zf-vis--compact']}
		data-testid={testid}
	>
		<Icon name={visibilityIconOf(shownLevel)} size="sm" />
		<span class={[compact && 'zf-sr']}>{words[shownLevel]}</span>
	</span>
{/if}
