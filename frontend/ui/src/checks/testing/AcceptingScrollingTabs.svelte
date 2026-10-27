<script lang="ts" generics="Id extends string">
	import ScrollingTabs from '../../lib/controls/ScrollingTabs.svelte';
	import type { TabItem } from '../../lib/controls/types.ts';

	interface Props {
		readonly label: string;
		readonly tabs: readonly TabItem<Id>[];
		/** The id showing to begin with. */
		readonly initial: Id;
		/** Whether this parent takes the changes it is told of; when not, the value never moves. */
		readonly accepts: boolean;
		readonly stepWords: { readonly back: string; readonly forward: string };
		readonly testid?: string | undefined;
		/** Told of every change asked for. */
		readonly onChange: (id: Id) => void;
	}

	let { label, tabs, initial, accepts, stepWords, testid, onChange }: Props = $props();

	/** The parent's own state: tabs are controlled, so whoever renders them holds `value`. */
	let chosen = $state<Id>();
	const value = $derived(chosen ?? initial);

	function take(next: Id): void {
		onChange(next);
		if (accepts) chosen = next;
	}
</script>

{#snippet panel(id: Id)}
	<p data-testid="panel-{id}">{tabs.find((tab) => tab.id === id)?.label} would show here.</p>
{/snippet}

<ScrollingTabs {label} {tabs} {value} {stepWords} {testid} {panel} onChange={take} />
