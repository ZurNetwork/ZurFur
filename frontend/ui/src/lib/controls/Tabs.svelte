<script lang="ts" generics="Id extends string">
	import { Tabs } from 'bits-ui';
	import type { Snippet } from 'svelte';
	import { useEnvironment } from '../environment/context';
	import Icon from '../icons/Icon.svelte';
	import { stepRow, watchRow, type StepSide } from './step-row';
	import type { TabItem, TabStep } from './types';

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
		/**
		 * With it, the tab list steps one tab at a time and this draws the two step
		 * buttons, one each side of the list; without it, the list is all there is.
		 */
		readonly stepper?: Snippet<[TabStep]> | undefined;
	}

	let { label, tabs, value, onChange, panel, testid, stepper }: Props = $props();

	const { moves, sounds } = useEnvironment();

	/** A tab asked for: unknown ids and the empty string are ignored; the rest play the cue and ask the parent. */
	function choose(next: string): void {
		const asked = tabs.find((tab) => tab.id === next);
		if (next === '' || asked === undefined) return;
		sounds.play('toggle');
		onChange(asked.id);
	}

	/** The tab list, while it is on the page and stepping. */
	let list: HTMLElement | undefined;
	let atStart = $state(true);
	let atEnd = $state(true);

	/** Steps the list's own scroll by one tab in `side`; no other box is scrolled. */
	function step(side: StepSide): void {
		if (list !== undefined) stepRow(list, side);
	}

	const back = $derived<TabStep>({
		side: 'back',
		step: () => {
			step('back');
		},
		atEdge: atStart
	});
	const forward = $derived<TabStep>({
		side: 'forward',
		step: () => {
			step('forward');
		},
		atEdge: atEnd
	});

	/** The tab list, as a plain snippet to render in either place it can sit. */
	const tabList: Snippet = tablist;

	/** Keeps the list and where it stands while it steps; leaves nothing behind. */
	function follow(node: HTMLElement): () => void {
		list = node;
		const stop = watchRow(node, (edges) => {
			atStart = edges.atStart;
			atEnd = edges.atEnd;
		});
		return () => {
			stop();
			list = undefined;
		};
	}
</script>

{#snippet tablist()}
	<Tabs.List aria-label={label}>
		{#snippet child({ props })}
			<div
				{...props}
				class={['zf-tabs', 'piece', stepper !== undefined && 'zf-tabs--step']}
				data-testid={testid}
				{@attach stepper === undefined ? undefined : follow}
			>
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
{/snippet}

<Tabs.Root class="zf-tabview" bind:value={() => value, choose} activationMode="automatic">
	{#if stepper === undefined}
		{@render tabList()}
	{:else}
		<div class="zf-tabs-row">
			{@render stepper(back)}
			{@render tabList()}
			{@render stepper(forward)}
		</div>
	{/if}
	<!-- bits-ui's content only hides what is not showing; this draws the one panel that is. -->
	{#each tabs as tab (tab.id)}
		{#if tab.id === value}
			<Tabs.Content value={tab.id} class="zf-tabview__panel">{@render panel(tab.id)}</Tabs.Content>
		{/if}
	{/each}
</Tabs.Root>
