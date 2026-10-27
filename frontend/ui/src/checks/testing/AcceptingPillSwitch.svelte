<script lang="ts" generics="Id extends string">
	import PillSwitch from '../../lib/controls/PillSwitch.svelte';
	import type { PillOption } from '../../lib/controls/types.ts';

	interface Props {
		readonly label: string;
		readonly options: readonly PillOption<Id>[];
		/** The id chosen to begin with. */
		readonly initial: Id;
		/** Whether this parent takes the choices it is told of; when not, the value never moves. */
		readonly accepts: boolean;
		readonly disabled?: boolean;
		readonly name?: string | undefined;
		readonly testid?: string | undefined;
		/** Told of every choice asked for. */
		readonly onChange: (id: Id) => void;
	}

	let {
		label,
		options,
		initial,
		accepts,
		disabled = false,
		name,
		testid,
		onChange
	}: Props = $props();

	/** The parent's own state: a switch is controlled, so whoever renders it holds `value`. */
	let chosen = $state<Id>();
	const value = $derived(chosen ?? initial);

	function take(next: Id): void {
		onChange(next);
		if (accepts) chosen = next;
	}
</script>

<PillSwitch {label} {options} {value} {disabled} {name} {testid} onChange={take} />
