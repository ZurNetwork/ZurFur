<script lang="ts">
	import { useEnvironment } from '../environment/context';
	import { skeletonLinesOf } from './choices';

	/** Text bars, the last one shorter; a count that is not a whole number from 1 to 12 draws one bar. */
	interface Lines {
		readonly shape: 'lines';
		readonly lines: number;
		readonly testid?: string | undefined;
	}

	/** A block the width of its parent. */
	interface Rect {
		readonly shape: 'rect';
		readonly lines?: undefined;
		readonly testid?: string | undefined;
	}

	/** A round blank, for an avatar. */
	interface Circle {
		readonly shape: 'circle';
		readonly lines?: undefined;
		readonly testid?: string | undefined;
	}

	type Props = Lines | Rect | Circle;

	let { shape, lines, testid }: Props = $props();

	const { moves } = useEnvironment();

	/** One entry per bar, so each bar is its own element for the loop. */
	const bars = $derived(Array.from({ length: skeletonLinesOf(lines) }, (_, index) => index));
</script>

{#if shape === 'rect'}
	<span
		class="zf-skeleton zf-skeleton--rect"
		aria-hidden="true"
		data-testid={testid}
		{@attach moves.loop('breathe')}
	></span>
{:else if shape === 'circle'}
	<span
		class="zf-skeleton zf-skeleton--circle"
		aria-hidden="true"
		data-testid={testid}
		{@attach moves.loop('breathe')}
	></span>
{:else}
	<span class="zf-skeleton-lines" aria-hidden="true" data-testid={testid}>
		{#each bars as bar (bar)}
			<span class="zf-skeleton zf-skeleton--text" {@attach moves.loop('breathe')}></span>
		{/each}
	</span>
{/if}
