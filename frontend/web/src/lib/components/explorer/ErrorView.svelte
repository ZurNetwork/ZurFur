<script lang="ts">
	import { resolve } from '$app/paths';
	import { denRootPath } from '$lib/api/den-route';
	import Button from '$lib/components/ui/Button.svelte';
	import EmptyState from '$lib/components/ui/EmptyState.svelte';
	import { errorCopy } from './error-copy';

	/**
	 * The error view, chosen by status and route alone: it takes no message,
	 * so no error's own text (which may hold a path or an internal detail) can
	 * ever reach the screen. Inside the frame, where a Den is served, its way
	 * back is My Den; otherwise the start page.
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
				<Button href={denRootPath()}>Back to My Den</Button>
			{:else}
				<Button href={resolve('/')}>Back to the start</Button>
			{/if}
		{/snippet}
	</EmptyState>
</div>
