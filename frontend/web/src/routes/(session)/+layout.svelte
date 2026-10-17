<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import type { DenPageData } from '$lib/api/den';
	import type { Trail } from '$lib/api/trail';
	import AppShell from '$lib/components/explorer/AppShell.svelte';
	import { BugReport, provideBugReport } from '$lib/components/explorer/bug-report.svelte';
	import ReportBugDialog from '$lib/components/explorer/ReportBugDialog.svelte';
	import { frameSectionOf } from '$lib/components/explorer/frame-section';
	import Tree from '$lib/components/explorer/Tree.svelte';
	import {
		FetchQueue,
		FOLDER_FETCHES_IN_FLIGHT,
		loadFolders
	} from '$lib/components/explorer/tree-loader';
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
	 * open folder the tree lacks is fetched through its server-built link, at
	 * most four at a time. A flip of "Show archived" starts the memory over,
	 * and so does a form action's success (the page that ran it says so),
	 * keeping `~` open. The frame also keeps the bug report's draft, starting
	 * it over when the signed-in visitor changes.
	 */
	let { data, children }: { data: LayoutData; children: Snippet } = $props();

	let drawerOpen = $state(false);
	const folderFetches = new FetchQueue(FOLDER_FETCHES_IN_FLIGHT);

	// svelte-ignore state_referenced_locally
	const memory = new TreeMemory(data.session.did);
	provideTreeMemory(memory);
	memory.absorb(page.data.den);

	// svelte-ignore state_referenced_locally
	const bugReport = new BugReport(data.session.did);
	provideBugReport(bugReport);

	$effect(() => {
		bugReport.ensureOwner(data.session.did);
	});

	afterNavigate(() => {
		drawerOpen = false;
		const missing = memory.navigated(data.session.did, page.data.den);
		void loadFolders(memory, missing, folderFetches);
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

<AppShell session={data.session} {trail} {section} bind:drawerOpen>
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
<ReportBugDialog session={data.session} buildId={data.buildId} />
