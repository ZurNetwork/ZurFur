<script lang="ts">
	import { untrack } from 'svelte';
	import DateField from '../../lib/controls/DateField.svelte';
	import type { CalendarDay } from '../../lib/controls/calendar-day.ts';
	import type { LocaleTag } from '../../lib/controls/locale-tag.ts';
	import type { DateFieldWords } from '../../lib/controls/types.ts';

	interface Props {
		readonly label: string;
		readonly locale: LocaleTag;
		readonly words: DateFieldWords;
		/** The day the parent holds at first. */
		readonly initial: CalendarDay | undefined;
		readonly min?: CalendarDay | undefined;
		readonly max?: CalendarDay | undefined;
		readonly name?: string | undefined;
		/** Told of every day asked for; this parent then accepts it. */
		readonly onChange: (day: CalendarDay | undefined) => void;
		/** Called with a setter the spec uses to move the parent's day, as a reset button would. */
		readonly onReady?: (set: (day: CalendarDay | undefined) => void) => void;
	}

	let { label, locale, words, initial, min, max, name, onChange, onReady }: Props = $props();

	/** The parent's own state: a date field is controlled, so whoever renders it holds `value`. */
	let value = $state(untrack(() => initial));

	function accept(next: CalendarDay | undefined): void {
		onChange(next);
		value = next;
	}

	/** Hands the spec its setter; called once, at init. */
	function ready(): void {
		onReady?.((day) => {
			value = day;
		});
	}

	ready();
</script>

<DateField {label} {locale} {words} {min} {max} {name} {value} onChange={accept} />
