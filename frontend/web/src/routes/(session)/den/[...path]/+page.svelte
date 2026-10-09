<script lang="ts">
	import { tick } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import { navigating } from '$app/state';
	import type { DenEntry } from '$lib/api/den';
	import DenPane from '$lib/components/explorer/DenPane.svelte';
	import { fetchListing } from '$lib/components/explorer/tree-loader';
	import { treeMemoryFromContext } from '$lib/components/explorer/tree-memory.svelte';
	import type { DenHref } from '$lib/types/brand';
	import type { PageData } from './$types';

	/**
	 * The right pane of My Den. While another Den item loads, the old pane
	 * stays and, after a short delay, its entries give way to skeleton rows.
	 * "More" appends the next page in place, in the tree too, and moves focus
	 * to the first new row; one that answers after the page changed is dropped. Opening an item from the pane or the path bar moves
	 * focus to the pane's heading; opening one from the tree leaves focus in
	 * the tree. The title stays generic; a live region says which item opened.
	 */
	let { data }: { data: PageData } = $props();

	/** How long a navigation runs before the pane shows it is busy, in ms: the one place this delay lives. */
	const BUSY_DELAY_MS = 300;

	let pane: HTMLDivElement | undefined = $state();
	/** Bumped on every navigation, so a "More" that answers after the page changed is dropped. */
	let pageGeneration = 0;
	let announcement = $state('');

	let heading: HTMLHeadingElement | undefined = $state();
	let busy = $state(false);
	let continuation: { entries: readonly DenEntry[]; more: DenHref | undefined } | undefined =
		$state();
	let moreState: 'idle' | 'loading' | 'failed' = $state('idle');

	const den = $derived(data.den);
	const memory = treeMemoryFromContext();
	const loadingDen = $derived(navigating.to?.route.id === '/(session)/den/[...path]');

	$effect(() => {
		if (!loadingDen) {
			busy = false;
			return;
		}
		const timer = setTimeout(() => {
			busy = true;
		}, BUSY_DELAY_MS);
		return () => {
			clearTimeout(timer);
		};
	});

	/** What the live region says once an item has opened: its name, or what the pane shows instead. */
	function spoken(): string {
		const { outcome } = den;
		switch (outcome.outcome) {
			case 'page':
				return outcome.page.crumbs.length === 0 ? 'My Den' : outcome.page.node.name;
			case 'notFound':
				return 'Nothing here';
			case 'notConnected':
				return "My Den isn't connected yet";
			case 'problem':
				return 'Something went wrong';
		}
	}

	afterNavigate((navigation) => {
		pageGeneration += 1;
		continuation = undefined;
		moreState = 'idle';
		if (navigation.type === 'enter') return;
		announcement = spoken();
		const active = document.activeElement;
		if (!(active instanceof HTMLElement) || active === document.body) heading?.focus();
	});

	/** Move focus to the first row of the entries appended from place `firstPlace` on. */
	async function focusFirstAppended(firstPlace: number): Promise<void> {
		await tick();
		const rows = pane?.querySelectorAll<HTMLElement>('[data-entry-place]') ?? [];
		const first = [...rows].find((row) => Number(row.dataset.entryPlace) >= firstPlace);
		first?.querySelector<HTMLElement>('a, [tabindex]')?.focus();
	}

	/** Fetch the page behind `more` and append it below the entries. */
	async function onMore(more: DenHref): Promise<void> {
		moreState = 'loading';
		const generation = pageGeneration;
		const memoryGeneration = memory?.generation;
		const fetched = await fetchListing(more);
		if (generation !== pageGeneration) return;
		if (fetched.fetched === 'failed') {
			moreState = 'failed';
			return;
		}
		if (fetched.fetched === 'redirected') return;
		const { outcome } = den;
		const firstBody =
			outcome.outcome === 'page' &&
			outcome.page.view === 'open' &&
			outcome.page.body.body === 'listing'
				? outcome.page.body.entries.length
				: 0;
		const firstPlace = firstBody + (continuation?.entries.length ?? 0);
		continuation = {
			entries: [...(continuation?.entries ?? []), ...fetched.entries],
			more: fetched.more
		};
		moreState = 'idle';
		if (
			outcome.outcome === 'page' &&
			outcome.page.view === 'open' &&
			memory?.generation === memoryGeneration
		) {
			memory?.appended(outcome.page.node.href, fetched.entries, fetched.more, more);
		}
		await focusFirstAppended(firstPlace);
	}
</script>

<svelte:head>
	<title>{den.title}</title>
</svelte:head>

<p class="visually-hidden" aria-live="polite" data-testid="den-announcer">{announcement}</p>

<div bind:this={pane}>
	<DenPane
		data={den}
		{busy}
		{continuation}
		{moreState}
		onMore={(more: DenHref) => {
			void onMore(more);
		}}
		bind:heading
	/>
</div>
