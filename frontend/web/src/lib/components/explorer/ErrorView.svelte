<script lang="ts">
	import { resolve } from '$app/paths';
	import { denRootPath } from '$lib/api/den-route';
	import Button from '$lib/components/ui/Button.svelte';
	import EmptyState from '$lib/components/ui/EmptyState.svelte';
	import { errorCopy } from './error-copy';
	import ReportBugButton from './ReportBugButton.svelte';

	/**
	 * The error view, chosen by status and route alone: it takes no message,
	 * so no error's own text (which may hold a path or an internal detail) can
	 * ever reach the screen. Inside the frame its way back is My Den, beside
	 * "Report this problem", which opens the report with the status (never the
	 * message); outside it, the start page alone.
	 */
	let {
		status,
		routeId,
		inFrame = false
	}: { status: number; routeId: string | undefined; inFrame?: boolean } = $props();

	const copy = $derived(errorCopy(status, routeId));
</script>

<div class="error-view" data-testid="error-view" data-status={status}>
	<h1>{copy.title}</h1>
	<EmptyState message={copy.message}>
		{#snippet action()}
			{#if inFrame}
				<div class="error-view__actions">
					<Button href={denRootPath()}>Back to My Den</Button>
					<ReportBugButton
						current={false}
						label="Report this problem"
						extras={{ errorStatus: status }}
					/>
				</div>
			{:else}
				<Button href={resolve('/')}>Back to the start</Button>
			{/if}
		{/snippet}
	</EmptyState>
</div>

<style>
	.error-view__actions {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-4);
	}
</style>
