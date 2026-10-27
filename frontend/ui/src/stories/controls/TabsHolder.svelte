<script lang="ts">
	import { Tabs, type TabItem } from '@zurfur/ui';

	interface Props {
		readonly label: string;
		readonly tabs: readonly TabItem<string>[];
		readonly value: string;
		readonly onChange: (id: string) => void;
	}

	let { label, tabs, value, onChange }: Props = $props();

	/** The tab showing: the story's value to begin with, then whatever is asked for. */
	let showing = $derived(value);

	/** Hands the id to the story's spy, then shows that tab. */
	function take(next: string): void {
		onChange(next);
		showing = next;
	}
</script>

{#snippet panel(id: string)}
	<p>{tabs.find((tab) => tab.id === id)?.label ?? id} would show here.</p>
{/snippet}

<Tabs {label} {tabs} value={showing} onChange={take} {panel} />
