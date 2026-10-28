<script lang="ts">
	import type { Snippet } from 'svelte';
	import { cardElementOf } from './choices';
	import type { CardElement, CardPadding } from './types';

	interface Props {
		/** Whatever the card holds; it has no slots of its own. */
		readonly children: Snippet;
		/** The element to draw; anything outside the four is a `div`. */
		readonly as?: CardElement | undefined;
		readonly padding?: CardPadding | undefined;
		readonly testid?: string | undefined;
	}

	let { children, as = 'div', padding = 'default', testid }: Props = $props();

	const element = $derived(cardElementOf(as));
	const classes = $derived(['obj', 'zf-card', padding === 'none' && 'zf-card--flush']);
</script>

<svelte:element this={element} class={classes} data-testid={testid}>
	{@render children()}
</svelte:element>
