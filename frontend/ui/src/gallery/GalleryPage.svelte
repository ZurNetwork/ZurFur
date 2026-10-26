<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		readonly states?: Snippet;
		readonly example?: Snippet;
		readonly keyboard?: Snippet;
		readonly motion?: Snippet;
		readonly tests?: Snippet;
	}

	let { states, example, keyboard, motion, tests }: Props = $props();

	const idPrefix = $props.id();
</script>

{#if states}
	<section aria-labelledby="{idPrefix}-states">
		<h2 id="{idPrefix}-states">States</h2>
		{@render states()}
	</section>
{/if}
{#if example}
	<section aria-labelledby="{idPrefix}-example">
		<h2 id="{idPrefix}-example">Live example</h2>
		{@render example()}
	</section>
{/if}
{#if keyboard}
	<section aria-labelledby="{idPrefix}-keyboard">
		<h2 id="{idPrefix}-keyboard">Keyboard</h2>
		{@render keyboard()}
	</section>
{/if}
{#if motion}
	<section aria-labelledby="{idPrefix}-motion">
		<h2 id="{idPrefix}-motion">Motion</h2>
		{@render motion()}
	</section>
{/if}
{#if tests}
	<section aria-labelledby="{idPrefix}-tests">
		<h2 id="{idPrefix}-tests">Tests</h2>
		{@render tests()}
	</section>
{/if}
