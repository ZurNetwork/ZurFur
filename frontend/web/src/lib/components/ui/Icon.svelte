<script lang="ts">
	import { ICONS, type IconName, type IconShape } from './icons';

	/**
	 * One inline icon, sized to the text around it and hidden from screen
	 * readers: whatever it means is always said in words beside it.
	 */
	let { name, size = '1.125em' }: { name: IconName; size?: string } = $props();

	const shapes: readonly IconShape[] = $derived(ICONS[name]);
</script>

<svg
	class="icon"
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="2"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
	focusable="false"
>
	{#each shapes as shape, index (index)}
		{#if shape.shape === 'path'}
			<path d={shape.d} />
		{:else if shape.shape === 'circle'}
			<circle cx={shape.cx} cy={shape.cy} r={shape.r} />
		{:else}
			<rect x={shape.x} y={shape.y} width={shape.width} height={shape.height} rx={shape.rx} />
		{/if}
	{/each}
</svg>

<style>
	.icon {
		flex: none;
		vertical-align: -0.15em;
	}
</style>
