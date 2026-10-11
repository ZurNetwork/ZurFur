<script lang="ts">
	import { page } from '$app/state';
	import ErrorView from '$lib/components/explorer/ErrorView.svelte';
	import { errorCopy } from '$lib/components/explorer/error-copy';

	/**
	 * A signed-in error renders inside the frame, by its status and route
	 * alone: the error's own message is never read here, so it can't reach the
	 * screen. Its way back is My Den only where this run serves one.
	 */
	const routeId = $derived(page.route.id ?? undefined);
	const title = $derived(errorCopy(page.status, routeId).title);
</script>

<svelte:head>
	<title>{title} — Zurfur</title>
</svelte:head>

<ErrorView status={page.status} {routeId} inFrame={page.data.denServed === true} />
