<script lang="ts">
	import { page } from '$app/state';
	import ErrorView from '$lib/components/explorer/ErrorView.svelte';
	import { errorCopy } from '$lib/components/explorer/error-copy';

	/** An error outside the frame (public pages), shown by its status and route alone, never by its message. */
	const routeId = $derived(page.route.id ?? undefined);
	const title = $derived(errorCopy(page.status, routeId).title);
</script>

<svelte:head>
	<title>{title} — Zurfur</title>
</svelte:head>

<main class="public-error">
	<ErrorView status={page.status} {routeId} />
</main>

<style>
	.public-error {
		max-width: var(--pane-max-width);
		padding: var(--space-6) clamp(var(--space-4), 3vw, var(--space-6));
	}
</style>
