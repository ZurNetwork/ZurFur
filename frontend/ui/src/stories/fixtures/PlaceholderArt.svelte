<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		/** What screen readers hear the art called. */
		readonly label: string;
		/** What sits on the art, placed against its box, such as pins. */
		readonly children?: Snippet | undefined;
	}

	let { label, children }: Props = $props();
</script>

<div class="placeholder-art">
	<svg viewBox="0 0 320 240" role="img" aria-label={label}>
		<rect class="ground" width="320" height="240" rx="12" />
		<path class="cross" d="M0 0 L320 240 M320 0 L0 240" />
		<circle class="sun" cx="236" cy="70" r="28" />
		<polygon class="hill" points="20,220 120,96 220,220" />
		<polygon class="hill-far" points="140,220 230,128 310,220" />
	</svg>
	{@render children?.()}
</div>

<style>
	.placeholder-art {
		position: relative;
		width: min(100%, 20rem);
	}

	svg {
		display: block;
		width: 100%;
		height: auto;
	}

	.ground {
		fill: var(--color-surface-raised);
		stroke: var(--color-border);
		stroke-width: 2;
	}

	.cross {
		stroke: var(--color-border);
		stroke-width: 2;
		stroke-dasharray: 8 8;
	}

	.sun {
		fill: var(--color-secondary-crown-soft);
	}

	.hill {
		fill: var(--color-secondary-iris-soft);
	}

	.hill-far {
		fill: var(--color-secondary-taupe-soft);
	}
</style>
