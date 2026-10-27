<script lang="ts" generics="Id extends string">
	import type { Snippet } from 'svelte';
	import type { IconName } from '../icons/icons';
	import IconButton from './IconButton.svelte';
	import type { StepSide } from './step-row';
	import Tabs from './Tabs.svelte';
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
		/** The names of the two step buttons, also shown in their tips. */
		readonly stepWords: { readonly back: string; readonly forward: string };
		/** Put on the tab list. */
		readonly testid?: string | undefined;
	}

	let { label, tabs, value, onChange, panel, stepWords, testid }: Props = $props();

	const ICONS = { back: 'chevron-left', forward: 'chevron-right' } as const satisfies Record<
		StepSide,
		IconName
	>;
</script>

{#snippet stepper(step: TabStep)}
	<IconButton
		icon={ICONS[step.side]}
		label={stepWords[step.side]}
		size="sm"
		disabled={step.atEdge}
		onPress={step.step}
	/>
{/snippet}

<Tabs {label} {tabs} {value} {onChange} {panel} {testid} {stepper} />
