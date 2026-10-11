<script lang="ts">
	import type { DenEntry } from '$lib/api/den';
	import TreeItem from '$lib/components/ui/TreeItem.svelte';
	import VisibilityMarker from '$lib/components/ui/VisibilityMarker.svelte';
	import type { DenHref } from '$lib/types/brand';

	/**
	 * The open folder's entries in the pane: folders first, then the rest,
	 * each in the server's order. An entry the viewer can open is a link with
	 * its own visibility; a card is a quiet row that isn't a link. "More"
	 * links to the next page; with the page's script, `onMore` appends it in
	 * place instead.
	 */
	let {
		entries,
		more,
		moreState = 'idle',
		onMore
	}: {
		entries: readonly DenEntry[];
		more: DenHref | undefined;
		moreState?: 'idle' | 'loading' | 'failed';
		onMore?: ((more: DenHref) => void) | undefined;
	} = $props();

	/** Each entry with its place in `entries`, folders first. */
	const ordered = $derived.by(() => {
		const placed = entries.map((entry, place) => ({ entry, place }));
		const isFolder = (row: (typeof placed)[number]) =>
			row.entry.view === 'open' && row.entry.kind === 'directory';
		return [...placed.filter(isFolder), ...placed.filter((row) => !isFolder(row))];
	});

	function moreClick(event: MouseEvent, href: DenHref): void {
		if (onMore === undefined) return;
		event.preventDefault();
		if (moreState !== 'loading') onMore(href);
	}
</script>

<ul class="entry-list" data-testid="entry-list">
	{#each ordered as { entry, place } (entry.view === 'open' ? entry.href : `card-${String(place)}`)}
		<li data-entry-place={place}>
			{#if entry.view === 'open'}
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- a DenHref is built (and resolved) by the server's Den path builder -->
				<a class="entry-row" href={entry.href} data-sveltekit-preload-data="tap">
					<TreeItem
						look={entry.kind}
						name={entry.name}
						mounted={entry.mounted}
						removed={entry.removed}
					/>
					<span class="entry-row__level"><VisibilityMarker level={entry.ownLevel} /></span>
				</a>
			{:else}
				<div class="entry-row entry-row--inert" data-testid="entry-card" tabindex="-1">
					<TreeItem look="cantOpen" name={entry.name} mounted={entry.mounted} sayCantOpen={false} />
					<span class="entry-row__level entry-row__cant">Can't open</span>
				</div>
			{/if}
		</li>
	{/each}
</ul>

{#if more !== undefined}
	<p class="entry-list__more">
		<!-- A DenHref is built (and resolved) by the server's Den path builder. -->
		<!-- eslint-disable svelte/no-navigation-without-resolve -->
		<a
			class="entry-list__more-link"
			href={more}
			aria-busy={moreState === 'loading' ? 'true' : undefined}
			data-testid="entry-more"
			onclick={(event) => {
				moreClick(event, more);
			}}
			>{moreState === 'failed'
				? "Couldn't load more. Retry."
				: moreState === 'loading'
					? 'Loading more…'
					: 'More'}</a
		>
		<!-- eslint-enable svelte/no-navigation-without-resolve -->
	</p>
{/if}

<style>
	.entry-list {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.entry-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		min-height: var(--space-9);
		padding: 0 var(--space-2);
		border-radius: var(--radius-sm);
		color: var(--color-text);
		text-decoration: none;
	}

	a.entry-row:hover {
		background: var(--color-hover);
	}

	.entry-row--inert {
		color: var(--color-text-muted);
	}

	.entry-row__level {
		flex: none;
	}

	.entry-row__cant {
		font-size: var(--text-xs);
	}

	.entry-list__more {
		margin-top: var(--space-3);
	}

	.entry-list__more-link[aria-busy='true'] {
		cursor: progress;
	}
</style>
