<script lang="ts">
	import { Select, type SelectOption } from '@zurfur/ui';

	interface Props {
		readonly label: string;
		readonly options: readonly SelectOption<string>[];
		readonly value: string | undefined;
		readonly onChange: (value: string) => void;
		readonly placeholder?: string | undefined;
		readonly hint?: string | undefined;
		readonly error?: string | undefined;
		readonly disabled?: boolean;
		readonly required?: boolean;
	}

	let {
		label,
		options,
		value,
		onChange,
		placeholder,
		hint,
		error,
		disabled = false,
		required = false
	}: Props = $props();

	/** The option chosen: the story's value to begin with, then whatever is picked. */
	let chosen = $derived(value);

	/** Hands the value to the story's spy, then keeps it. */
	function take(next: string): void {
		onChange(next);
		chosen = next;
	}
</script>

<Select
	{label}
	{options}
	value={chosen}
	onChange={take}
	{placeholder}
	{hint}
	{error}
	{disabled}
	{required}
/>
