<script lang="ts">
	import { parseEasing, plotOf } from './easing-curve';

	interface Props {
		/** The easing as the token writes it. */
		readonly easing: string;
		/** What the plot is called for assistive technology. */
		readonly label: string;
	}

	let { easing, label }: Props = $props();

	const plot = $derived(plotOf(parseEasing(easing)));
</script>

{#if plot}
	<svg class="plot" viewBox={plot.viewBox} role="img" aria-label={label}>
		<path class="diagonal" d={plot.diagonal} />
		<path class="curve" d={plot.curve} />
	</svg>
{/if}

<style>
	.plot {
		display: block;
		width: var(--size-rail);
		height: var(--size-rail);
		overflow: visible;
	}

	.diagonal,
	.curve {
		fill: none;
		stroke-linecap: round;
		vector-effect: non-scaling-stroke;
	}

	.diagonal {
		stroke: var(--color-border-strong);
		stroke-dasharray: 3 3;
		stroke-width: 1;
	}

	.curve {
		stroke: var(--color-accent-text);
		stroke-width: 2;
	}
</style>
