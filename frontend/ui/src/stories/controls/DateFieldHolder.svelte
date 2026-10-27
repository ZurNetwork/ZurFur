<script lang="ts">
	import { DateField, type CalendarDay, type DateFieldWords, type LocaleTag } from '@zurfur/ui';

	interface Props {
		readonly label: string;
		readonly value: CalendarDay | undefined;
		readonly onChange: (day: CalendarDay | undefined) => void;
		readonly locale: LocaleTag;
		readonly words: DateFieldWords;
		readonly min?: CalendarDay | undefined;
		readonly max?: CalendarDay | undefined;
		readonly startFrom?: CalendarDay | undefined;
		readonly hint?: string | undefined;
		readonly error?: string | undefined;
		readonly disabled?: boolean;
		readonly required?: boolean;
	}

	let {
		label,
		value,
		onChange,
		locale,
		words,
		min,
		max,
		startFrom,
		hint,
		error,
		disabled = false,
		required = false
	}: Props = $props();

	/** The day held: the story's value to begin with, then whatever is typed. */
	let day = $derived(value);

	/** Hands the day to the story's spy, then keeps it. */
	function take(next: CalendarDay | undefined): void {
		onChange(next);
		day = next;
	}
</script>

<DateField
	{label}
	value={day}
	onChange={take}
	{locale}
	{words}
	{min}
	{max}
	{startFrom}
	{hint}
	{error}
	{disabled}
	{required}
/>
