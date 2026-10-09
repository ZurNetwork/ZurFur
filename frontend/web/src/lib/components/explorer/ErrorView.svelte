<script lang="ts">
	import { resolve } from '$app/paths';
	import Button from '$lib/components/ui/Button.svelte';
	import EmptyState from '$lib/components/ui/EmptyState.svelte';
	import { errorCopy } from './error-copy';

	/**
	 * The error view, chosen by status and route alone: it takes no message,
	 * so no error's own text (which may hold a path or an internal detail) can
	 * ever reach the screen.
	 */
	let { status, routeId }: { status: number; routeId: string | undefined } = $props();

	const copy = $derived(errorCopy(status, routeId));
</script>

<div class="error-view" data-testid="error-view" data-status={status}>
	<h1>{copy.title}</h1>
	<EmptyState message={copy.message}>
		{#snippet action()}
			<Button href={resolve('/')}>Back to the start</Button>
		{/snippet}
	</EmptyState>
</div>
