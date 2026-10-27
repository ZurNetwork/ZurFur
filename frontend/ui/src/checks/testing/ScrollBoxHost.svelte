<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		/** What sits in the skin scope; a spec may instead render into the scope itself. */
		readonly children?: Snippet | undefined;
	}

	let { children }: Props = $props();
</script>

<!--
	A short box that scrolls, around a transformed layer, around a skin scope:
	a floating part portals into the scope, so it sits inside the transform
	and can make the box scroll, as on Storybook's docs page.
-->
<div class="box" data-testid="scroll-box">
	<div class="layer">
		<div data-skin="light" data-testid="scroll-box-scope">{@render children?.()}</div>
	</div>
</div>

<style>
	.box {
		height: 80px;
		overflow: auto;
	}

	.layer {
		transform: translateZ(0);
	}
</style>
