<script lang="ts">
	import SessionHeader from '$lib/components/SessionHeader.svelte';
	import type { LayoutData } from './$types';
	import type { Snippet } from 'svelte';

	/** The public pages: today's plain header above the page. */
	let { data, children }: { data: LayoutData; children: Snippet } = $props();
</script>

<SessionHeader session={data.session} />

<main class="public-page">
	{@render children()}
</main>

<style>
	.public-page {
		max-width: var(--pane-max-width);
		padding: var(--space-6) clamp(var(--space-4), 3vw, var(--space-6));
	}
</style>
