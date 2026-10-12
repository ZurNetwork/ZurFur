<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import type { DenPageData } from '$lib/api/den';
	import type { Trail } from '$lib/api/trail';
	import AppShell from '$lib/components/explorer/AppShell.svelte';
	import { frameSectionOf } from '$lib/components/explorer/frame-section';
	import Tree from '$lib/components/explorer/Tree.svelte';
	import { loadFolder } from '$lib/components/explorer/tree-loader';
	import { provideTreeMemory, TreeMemory } from '$lib/components/explorer/tree-memory.svelte';
	import type { TreeCurrent } from '$lib/components/explorer/tree-model';
	import ViewOptions from '$lib/components/explorer/ViewOptions.svelte';
	import { denRootFallback } from '$lib/components/explorer/den-root';
	import type { LayoutData } from './$types';
	import type { Snippet } from 'svelte';

	/**
	 * Every signed-in page sits in the Explorer frame. The frame reads nothing
	 * from the Den itself: the path comes from the open page's `trail`, and the
	 * tree from what Den pages bring (kept in this frame's tree memory, never
	 * in browser storage). After each navigation the drawer closes and any
	 * open folder the tree lacks is fetched through its server-built link. A
	 * flip of "Show archived" starts the memory over, and so does a form
	 * action's success (the page that ran it says so), keeping `~` open. The sidebar, the tree and the drawer appear only where
	 * this run serves a Den.
	 */
	let { data, children }: { data: LayoutData; children: Snippet } = $props();

	let drawerOpen = $state(false);

	// svelte-ignore state_referenced_locally
	const memory = new TreeMemory(data.session.did);
	provideTreeMemory(memory);
	memory.absorb(page.data.den);

	afterNavigate(() => {
		drawerOpen = false;
		const missing = memory.navigated(data.session.did, page.data.den);
		if (!data.denServed) return;
		for (const folder of missing) void loadFolder(memory, folder);
	});

	const den: DenPageData | undefined = $derived(page.data.den);
	const trail: Trail = $derived(page.data.trail ?? []);
	const section = $derived(frameSectionOf(page.route.id ?? undefined));
	const current: TreeCurrent = $derived.by(() => {
		if (den?.outcome.outcome !== 'page') {
			return { href: undefined, parentHref: undefined, entry: undefined };
		}
		const { page: denPage } = den.outcome;
		return {
			href: denPage.view === 'open' ? denPage.node.href : undefined,
			parentHref: denPage.crumbs.at(-1)?.href,
			entry: denPage.node
		};
	});
	const flagLinks = $derived(den?.outcome.outcome === 'page' ? den.outcome.flagLinks : undefined);
</script>

<AppShell session={data.session} {trail} {section} denServed={data.denServed} bind:drawerOpen>
	{#snippet navigation()}
		<nav class="frame-nav" aria-label="My Den">
			<Tree {memory} {current} fallbackRootHref={denRootFallback()} />
			{#if den !== undefined && flagLinks !== undefined}
				<ViewOptions includeDeleted={den.includeDeleted} {flagLinks} />
			{/if}
		</nav>
	{/snippet}
	{@render children()}
</AppShell>
