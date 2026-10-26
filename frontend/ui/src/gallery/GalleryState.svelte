<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		/** The state's name, which a test harness finds it by. */
		readonly name: string;
		readonly children: Snippet;
	};

	let { name, children }: Props = $props();
</script>

<div data-gallery-state={name}>
	{@render children()}
</div>
