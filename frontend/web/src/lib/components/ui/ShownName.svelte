<script lang="ts">
	import type { NameOrdinal } from '$lib/types/brand';

	/**
	 * A Den name in its <bdi>, then " (n)" outside it after an earlier equal
	 * name in the same folder; no wrapper, so the caller's box cuts a long name.
	 * The number's placement relies on the backend's name rule, which refuses
	 * or strips bidi controls in Den names.
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
