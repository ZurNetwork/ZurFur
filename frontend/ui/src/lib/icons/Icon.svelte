<script lang="ts">
	import { ICONS, STROKE_HEAVY, STROKE_REGULAR, type IconName } from './icons';

	interface Props {
		readonly name: IconName;
		/** `sm` is 14px (pills, badges, twisties), `md` 16px, `lg` 20px (rail, empty states, toasts). */
		readonly size?: 'sm' | 'md' | 'lg';
		/** `heavy` is for the checkmark inside a checked control. */
		readonly weight?: 'regular' | 'heavy';
	}

	let { name, size = 'md', weight = 'regular' }: Props = $props();

	const shapes = $derived(ICONS[name]);
	const strokeWidth = $derived(weight === 'heavy' ? STROKE_HEAVY : STROKE_REGULAR);
</script>

<svg
	class={['zf-icon', size]}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width={strokeWidth}
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
	focusable="false"
>
	{#each shapes as drawn, index (index)}
		{#if drawn.shape === 'path'}
			<path d={drawn.d} />
		{:else if drawn.shape === 'circle'}
			<circle cx={drawn.cx} cy={drawn.cy} r={drawn.r} />
		{:else if drawn.shape === 'rect'}
			<rect
				x={drawn.x}
				y={drawn.y}
				width={drawn.width}
				height={drawn.height}
				rx={drawn.rx}
				ry={drawn.ry}
			/>
		{:else if drawn.shape === 'line'}
			<line x1={drawn.x1} y1={drawn.y1} x2={drawn.x2} y2={drawn.y2} />
		{:else if drawn.shape === 'polyline'}
			<polyline points={drawn.points} />
		{/if}
	{/each}
</svg>

<style>
	.zf-icon {
		flex: none;
		display: block;
	}

	.sm {
		width: var(--size-icon-sm);
		height: var(--size-icon-sm);
	}

	.md {
		width: var(--size-icon);
		height: var(--size-icon);
	}

	.lg {
		width: var(--size-icon-lg);
		height: var(--size-icon-lg);
	}
</style>
