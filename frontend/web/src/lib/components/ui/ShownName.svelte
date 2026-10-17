<script lang="ts">
	import type { NameOrdinal } from '$lib/types/brand';

	/**
	 * A Den name as shown: the name isolated for right-to-left text, then
	 * " (n)" outside the isolate when an earlier equal name sits in the same
	 * folder. No wrapper: the caller's box decides how a long name is cut.
	 */
	let { name, ordinal }: { name: string; ordinal: NameOrdinal | undefined } = $props();
</script>

<bdi>{name}</bdi>{#if ordinal !== undefined}<span
		class="shown-name__number"
		data-testid="name-ordinal">{` (${String(ordinal)})`}</span
	>{/if}

<style>
	/* The leading space is kept, and the number never shrinks, even as a flex item. */
	.shown-name__number {
		flex: none;
		white-space: pre;
	}
</style>
