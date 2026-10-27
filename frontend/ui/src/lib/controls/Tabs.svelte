<script lang="ts" generics="Id extends string">
	import { Tabs } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import { useEnvironment } from '../environment/context';
	import Icon from '../icons/Icon.svelte';
	import type { TabItem } from './types';

	interface Props {
		/** The tab list's accessible name. */
		readonly label: string;
		readonly tabs: readonly TabItem<Id>[];
		/** The id of the tab showing; the parent decides whether it follows `onChange`. */
		readonly value: Id;
		/** Handed the id of the tab an interaction asks to show. */
		readonly onChange: (id: Id) => void;
		/** The body of one tab; drawn for the tab showing and for no other. */
		readonly panel: Snippet<[Id]>;
		/** Put on the tab list. */
		readonly testid?: string | undefined;
	}

	let { label, tabs, value, onChange, panel, testid }: Props = $props();

	const { moves, sounds } = useEnvironment();

	/** A tab asked for: unknown ids and the empty string are ignored; the rest play the cue and ask the parent. */
	function choose(next: string): void {
		const asked = tabs.find((tab) => tab.id === next);
		if (next === '' || asked === undefined) return;
		sounds.play('toggle');
		onChange(asked.id);
	}
</script>

<Tabs.Root class="zf-tabview" bind:value={() => value, choose} activationMode="automatic">
	<Tabs.List aria-label={label}>
		{#snippet child({ props })}
			<div {...props} class="zf-tabs piece" data-testid={testid}>
				{#each tabs as tab (tab.id)}
					<Tabs.Trigger value={tab.id}>
						{#snippet child({ props })}
							<button {...props} class="zf-tab pick" {@attach moves.squish()}>
								{#if tab.icon !== undefined}<Icon name={tab.icon} size="sm" />{/if}
								<span>{tab.label}</span>
								{#if tab.count !== undefined}<span class="zf-tab__count">{tab.count}</span>{/if}
							</button>
						{/snippet}
					</Tabs.Trigger>
				{/each}
			</div>
		{/snippet}
	</Tabs.List>
	<!-- bits-ui's content only hides what is not showing; this draws the one panel that is. -->
	{#each tabs as tab (tab.id)}
		{#if tab.id === value}
			<Tabs.Content value={tab.id} class="zf-tabview__panel">{@render panel(tab.id)}</Tabs.Content>
		{/if}
	{/each}
</Tabs.Root>
