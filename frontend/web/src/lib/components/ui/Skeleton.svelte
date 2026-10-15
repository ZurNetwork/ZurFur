<script lang="ts">
	/**
	 * Placeholder rows while something loads, each as tall as the row it stands
	 * in for (`--row-height`); hidden from screen readers (the busy container says so).
	 */
	let { rows = 3 }: { rows?: number } = $props();
</script>

<div class="skeleton" aria-hidden="true" data-testid="skeleton">
	{#each { length: rows }, index (index)}
		<span class="skeleton__row">
			<span class="skeleton__bar" style:width="{90 - ((index * 17) % 40)}%"></span>
		</span>
	{/each}
</div>

<style>
	.skeleton {
		display: flex;
		flex-direction: column;
	}

	/* The row takes the real row's height; its bar sits centred in it. */
	.skeleton__row {
		display: flex;
		align-items: center;
		height: var(--row-height);
	}

	.skeleton__bar {
		display: block;
		height: var(--space-3);
		border-radius: var(--radius-sm);
		background: var(--color-surface-raised);
		animation: skeleton-pulse var(--duration-skeleton) ease-in-out infinite;
	}

	@keyframes skeleton-pulse {
		50% {
			opacity: 0.5;
		}
	}
</style>
