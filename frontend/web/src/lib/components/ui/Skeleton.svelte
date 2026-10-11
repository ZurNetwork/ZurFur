<script lang="ts">
	/** Placeholder rows while something loads; hidden from screen readers (the busy container says so). */
	let { rows = 3 }: { rows?: number } = $props();
</script>

<div class="skeleton" aria-hidden="true" data-testid="skeleton">
	{#each { length: rows }, index (index)}
		<span class="skeleton__row" style:width="{90 - ((index * 17) % 40)}%"></span>
	{/each}
</div>

<style>
	.skeleton {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-1) 0;
	}

	.skeleton__row {
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
