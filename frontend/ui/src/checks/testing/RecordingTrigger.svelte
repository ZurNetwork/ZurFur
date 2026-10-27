<script lang="ts">
	import Tooltip from '../../lib/controls/Tooltip.svelte';

	interface Props {
		/** Handed the record Tooltip gives its trigger, each time it changes. */
		readonly onRecord: (props: Readonly<Record<string, unknown>>) => void;
		readonly disabled?: boolean;
	}

	let { onRecord, disabled = false }: Props = $props();
</script>

<!-- A trigger that records what it is handed and spreads none of it. -->
{#snippet record({ props }: { readonly props: Record<string, unknown> })}
	<button
		type="button"
		data-testid="recording-trigger"
		{@attach () => {
			onRecord(props);
		}}>Tip</button
	>
{/snippet}

<Tooltip words="Tip" trigger={record} {disabled} />
