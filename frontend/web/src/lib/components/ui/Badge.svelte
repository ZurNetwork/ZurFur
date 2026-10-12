<script lang="ts">
	/** A small neutral label that always carries its word, such as "Archived". */
	let { label }: { label: string } = $props();
</script>

<span class="badge" data-testid="badge">{label}</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		padding: 0 var(--space-2);
		border: var(--border-width) solid var(--color-border);
		border-radius: var(--radius-sm);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		line-height: 1.5;
		white-space: nowrap;
	}
</style>
