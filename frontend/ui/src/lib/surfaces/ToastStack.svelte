<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		/** What the region is called, in words. */
		readonly label: string;
		/** The toasts the page puts in; the live region stays mounted when there are none. */
		readonly children?: Snippet | undefined;
	}

	let { label, children }: Props = $props();
</script>

<section class="zf-toast-stack" aria-label={label}>
	<div aria-live="polite">
		{@render children?.()}
	</div>
</section>
