<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import type { Trail } from '$lib/api/trail';
	import AppShell from '$lib/components/explorer/AppShell.svelte';
	import { frameSectionOf } from '$lib/components/explorer/frame-section';
	import type { LayoutData } from './$types';
	import type { Snippet } from 'svelte';

	/**
	 * Every signed-in page sits in the Explorer frame. The frame reads nothing
	 * itself: the path comes from the open page's `trail`, and the drawer
	 * closes after every navigation, so choosing an item in it closes it.
	 */
	let { data, children }: { data: LayoutData; children: Snippet } = $props();

	let drawerOpen = $state(false);

	afterNavigate(() => {
		drawerOpen = false;
	});

	const trail: Trail = $derived(page.data.trail ?? []);
	const section = $derived(frameSectionOf(page.route.id ?? undefined));
</script>

<AppShell session={data.session} {trail} {section} bind:drawerOpen>
	{#snippet navigation()}
		<!-- The sidebar stays empty until My Den's tree arrives; the sections live in the top bar and the drawer's footer. -->
	{/snippet}
	{@render children()}
</AppShell>
