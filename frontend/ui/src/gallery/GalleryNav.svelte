<script lang="ts">
	import { resolve } from '$app/paths';
	import { groupNames } from './groups';
	import type { PageMeta } from './page-meta';

	interface Props {
		readonly entries: readonly PageMeta[];
	}

	let { entries }: Props = $props();

	const idPrefix = $props.id();
	// Page slugs come from the registry at run time, so no typed route can name them.
	const root = resolve('/');
</script>

{#if entries.length === 0}
	<p>No components yet.</p>
{:else}
	{#each groupNames as group, position (group)}
		{@const inGroup = entries.filter((entry) => entry.group === group)}
		{#if inGroup.length > 0}
			<section aria-labelledby="{idPrefix}-group-{position}">
				<h2 id="{idPrefix}-group-{position}">{group}</h2>
				<ul>
					{#each inGroup as entry (entry.slug)}
						<li><a href={`${root}components/${entry.slug}`}>{entry.title}</a></li>
					{/each}
				</ul>
			</section>
		{/if}
	{/each}
{/if}
