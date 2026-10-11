<script lang="ts">
	import type { CardEntry, DenBody, DenEntry, DenPageData, OpenEntry } from '$lib/api/den';
	import type { Problem } from '$lib/api/problem';
	import ProblemNote from '$lib/components/ProblemNote.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import EmptyState from '$lib/components/ui/EmptyState.svelte';
	import Skeleton from '$lib/components/ui/Skeleton.svelte';
	import type { DenHref } from '$lib/types/brand';
	import EntryList from './EntryList.svelte';
	import NodeHeader from './NodeHeader.svelte';
	import { denRootPath } from '$lib/api/den-route';

	/**
	 * The right pane for one Den answer, in every state: an open folder (its
	 * entries, or "This folder is empty"), content not shown yet, a file, a
	 * link, a card, the one not-found, "not connected yet", and an API
	 * problem. `busy` swaps the entries for skeleton rows while the next item
	 * loads; `continuation` carries the pages fetched in place after the
	 * first, with the link to the one after them.
	 */
	let {
		data,
		busy = false,
		continuation,
		moreState = 'idle',
		onMore,
		heading = $bindable()
	}: {
		data: DenPageData;
		busy?: boolean;
		continuation?:
			{ readonly entries: readonly DenEntry[]; readonly more: DenHref | undefined } | undefined;
		moreState?: 'idle' | 'loading' | 'failed';
		onMore?: ((more: DenHref) => void) | undefined;
		heading?: HTMLHeadingElement | undefined;
	} = $props();

	/** What the pane shows, worked out once so the markup reads one level deep. */
	type PaneView =
		| { readonly pane: 'notConnected' }
		| { readonly pane: 'notFound' }
		| { readonly pane: 'problem'; readonly problem: Problem }
		| { readonly pane: 'card'; readonly node: CardEntry }
		| {
				readonly pane: 'open';
				readonly node: OpenEntry;
				readonly root: boolean;
				readonly body: PaneBody;
		  };

	/** What sits under an open node's header. */
	type PaneBody =
		| {
				readonly body: 'entries';
				readonly entries: readonly DenEntry[];
				readonly more: DenHref | undefined;
		  }
		| { readonly body: 'empty' }
		| { readonly body: 'notShown' }
		| { readonly body: 'link' };

	/** The body for an open node: its listing (with any pages fetched in place), or a notice. */
	function paneBody(node: OpenEntry, body: DenBody): PaneBody {
		if (node.contentNotShown) return { body: 'notShown' };
		switch (body.body) {
			case 'listing': {
				const entries = [...body.entries, ...(continuation?.entries ?? [])];
				const more = continuation === undefined ? body.more : continuation.more;
				return entries.length === 0 ? { body: 'empty' } : { body: 'entries', entries, more };
			}
			case 'link':
				return { body: 'link' };
			case 'file':
			case 'notShown':
				return { body: 'notShown' };
		}
	}

	const view: PaneView = $derived.by(() => {
		const { outcome } = data;
		switch (outcome.outcome) {
			case 'notConnected':
				return { pane: 'notConnected' };
			case 'notFound':
				return { pane: 'notFound' };
			case 'problem':
				return { pane: 'problem', problem: outcome.problem };
			case 'page': {
				const { page } = outcome;
				if (page.view === 'card') return { pane: 'card', node: page.node };
				const root = page.crumbs.length === 0;
				return { pane: 'open', node: page.node, root, body: paneBody(page.node, page.body) };
			}
		}
	});
</script>

<section class="den-pane" aria-busy={busy ? 'true' : undefined} data-testid="den-pane">
	{#if view.pane === 'notConnected'}
		<h1 tabindex="-1" bind:this={heading}>My Den</h1>
		<EmptyState message="My Den isn't connected to the backend yet." testid="den-not-connected" />
	{:else if view.pane === 'notFound'}
		<h1 tabindex="-1" bind:this={heading}>Nothing here</h1>
		<EmptyState message="This doesn't exist, or you can't open it." testid="den-not-found">
			{#snippet action()}
				<Button href={denRootPath()}>Back to My Den</Button>
			{/snippet}
		</EmptyState>
	{:else if view.pane === 'problem'}
		<h1 tabindex="-1" bind:this={heading}>My Den</h1>
		<ProblemNote problem={view.problem} />
	{:else if view.pane === 'card'}
		<NodeHeader node={view.node} bind:heading />
		<EmptyState message="You can see that this exists, but you can't open it." testid="den-card" />
	{:else if view.pane === 'open'}
		<NodeHeader node={view.node} root={view.root} bind:heading />
		{#if busy}
			<Skeleton rows={4} />
		{:else if view.body.body === 'entries'}
			<EntryList entries={view.body.entries} more={view.body.more} {moreState} {onMore} />
		{:else if view.body.body === 'empty'}
			<EmptyState message="This folder is empty." testid="den-empty" />
		{:else if view.body.body === 'link'}
			<EmptyState message="Opening links isn't available yet." testid="den-link" />
		{:else}
			<EmptyState message="Content not shown yet." testid="den-not-shown" />
		{/if}
	{/if}
</section>

<style>
	.den-pane {
		max-width: var(--pane-max-width);
	}
</style>
