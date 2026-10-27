<script lang="ts">
	import { TextInput, type IconName } from '@zurfur/ui';

	interface Props {
		readonly label: string;
		readonly labelHidden?: boolean;
		readonly value: string;
		readonly onChange: (text: string) => void;
		readonly hint?: string | undefined;
		readonly error?: string | undefined;
		readonly icon?: IconName | undefined;
		readonly prefix?: string | undefined;
		readonly mono?: boolean;
		readonly multiline?: boolean;
		readonly type?: 'text' | 'search';
		readonly required?: boolean;
		readonly disabled?: boolean;
		readonly readonly?: boolean;
		readonly placeholder?: string | undefined;
	}

	let {
		label,
		labelHidden = false,
		value,
		onChange,
		hint,
		error,
		icon,
		prefix,
		mono = false,
		multiline = false,
		type = 'text',
		required = false,
		disabled = false,
		readonly = false,
		placeholder
	}: Props = $props();

	/** The text the field holds: the story's value to begin with, then whatever is typed. */
	let text = $derived(value);

	/** Hands the text to the story's spy, then keeps it. */
	function take(next: string): void {
		onChange(next);
		text = next;
	}
</script>

<TextInput
	{label}
	{labelHidden}
	value={text}
	onChange={take}
	{hint}
	{error}
	{icon}
	{prefix}
	{mono}
	{multiline}
	{type}
	{required}
	{disabled}
	{readonly}
	{placeholder}
/>
