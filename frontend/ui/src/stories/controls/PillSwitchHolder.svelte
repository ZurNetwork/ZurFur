<script lang="ts" generics="Id extends string">
	import { PillSwitch, type PillOption } from '@zurfur/ui';

	interface Props {
		readonly label: string;
		readonly options: readonly PillOption<Id>[];
		readonly value: Id;
		readonly onChange: (id: Id) => void;
		readonly disabled?: boolean | undefined;
		readonly name?: string | undefined;
		readonly testid?: string | undefined;
	}

	let { label, options, value, onChange, disabled = false, name, testid }: Props = $props();

	/** The option shown as chosen: the story's value, until a choice replaces it. */
	let chosen = $derived(value);

	/** Hands the choice to the story's callback, then shows it. */
	function choose(id: Id): void {
		onChange(id);
		chosen = id;
	}
</script>

<PillSwitch {label} {options} value={chosen} onChange={choose} {disabled} {name} {testid} />
