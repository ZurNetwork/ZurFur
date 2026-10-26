<script lang="ts">
	import { DateField } from 'bits-ui';
	import { untrack, type Component, type ComponentProps } from 'svelte';
	import { parseDate, toCalendarDate, type DateValue } from '@internationalized/date';
	import FieldFrame from './FieldFrame.svelte';
	import { isCalendarDay, type CalendarDay } from './calendar-day';
	import type { LocaleTag } from './locale-tag';
	import type { DateFieldWords, FieldIds } from './types';

	interface Props {
		readonly label: string;
		/** The day shown; undefined shows an empty field. */
		readonly value: CalendarDay | undefined;
		/** Handed the day an edit makes, or undefined once the field no longer holds a whole day; the parent decides whether `value` follows. */
		readonly onChange: (day: CalendarDay | undefined) => void;
		/** Sets the order of the segments and the marks between them. */
		readonly locale: LocaleTag;
		/** The segments' names, and what an empty segment shows and says. */
		readonly words: DateFieldWords;
		/** The first day the field hands on; an earlier one is never reported. */
		readonly min?: CalendarDay | undefined;
		/** The last day the field hands on; a later one is never reported. */
		readonly max?: CalendarDay | undefined;
		/** The day an empty segment steps from with the arrow keys. */
		readonly startFrom?: CalendarDay | undefined;
		readonly hint?: string | undefined;
		readonly error?: string | undefined;
		readonly disabled?: boolean;
		readonly required?: boolean;
		/** With a name, a form posts the day as `YYYY-MM-DD`; without one it never does. */
		readonly name?: string | undefined;
		readonly testid?: string | undefined;
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
		required = false,
		name,
		testid
	}: Props = $props();

	const id = $props.id();

	/** The prop as the date library's day: rebuilt only when `value` changes, so its identity holds. */
	const shown: DateValue | undefined = $derived(value === undefined ? undefined : parseDate(value));
	const start: DateValue | undefined = $derived(
		startFrom === undefined ? undefined : parseDate(startFrom)
	);
	const lowest: DateValue | undefined = $derived(min === undefined ? undefined : parseDate(min));
	const highest: DateValue | undefined = $derived(max === undefined ? undefined : parseDate(max));

	type RootProps = ComponentProps<typeof DateField.Root>;
	/** bits-ui's root, typed the way it reads these four props: undefined is no day, as if the prop were left out. */
	type OpenRoot = Component<
		Omit<RootProps, 'value' | 'placeholder' | 'minValue' | 'maxValue'> & {
			readonly value?: DateValue | undefined;
			readonly placeholder?: DateValue | undefined;
			readonly minValue?: DateValue | undefined;
			readonly maxValue?: DateValue | undefined;
		},
		Record<never, never>,
		'value' | 'placeholder'
	>;

	/**
	 * Whether `candidate` is bits-ui's date-field root, whose `$props()` gives
	 * `value`, `placeholder`, `minValue` and `maxValue` no fallback and so reads
	 * an undefined one as absent, though its types leave undefined out.
	 */
	function isOpenRoot(candidate: unknown): candidate is OpenRoot {
		return candidate === DateField.Root;
	}

	const bitsRoot: unknown = DateField.Root;
	const Root = isOpenRoot(bitsRoot) ? bitsRoot : undefined;

	/** Bumped to rebuild the segments from `value`, once an edit is over or `value` moves on its own. */
	let generation = $state(0);
	/** Whether focus is inside the field. */
	let editing = false;
	/** What the segments last offered, while it may differ from `value`. */
	let offered: { readonly day: CalendarDay | undefined } | undefined;

	/** The library's day as a calendar day, or undefined when it names none. */
	function dayOf(next: DateValue): CalendarDay | undefined {
		const text = toCalendarDate(next).toString();
		return isCalendarDay(text) ? text : undefined;
	}

	function isOutside(day: CalendarDay): boolean {
		return (min !== undefined && day < min) || (max !== undefined && day > max);
	}

	/** An edit: hands the parent a new day, or undefined for an unfinished field; a day outside `min` and `max` goes nowhere. */
	function offer(next: DateValue | undefined): void {
		const day = next === undefined ? undefined : dayOf(next);
		offered = { day };
		if (day !== undefined && isOutside(day)) return;
		if (day !== value) onChange(day);
	}

	/** Once focus leaves the field, segments still showing something other than `value` are rebuilt from it. */
	function settle(event: FocusEvent & { readonly currentTarget: HTMLElement }): void {
		const field = event.currentTarget;
		const into = event.relatedTarget;
		if (into instanceof Node && field.contains(into)) return;
		editing = false;
		const last = offered;
		offered = undefined;
		if (last === undefined) return;
		// Focus also leaves a field being taken off the page; only one still on it is rebuilt.
		queueMicrotask(() => {
			if (field.isConnected && last.day !== value) generation += 1;
		});
	}

	function enter(): void {
		editing = true;
	}

	/** The last `value` the field has seen. */
	let seen = untrack(() => value);

	/** A `value` the parent moves while nobody is editing rebuilds the segments, whatever they held. */
	$effect.pre(() => {
		if (value === seen) return;
		seen = value;
		if (editing) return;
		offered = undefined;
		generation += 1;
	});

	/** Whether a segment shows a value: a filled one shows digits, an empty one the library's letters. */
	const DIGIT = /\p{Decimal_Number}/u;

	function wordFor(part: 'day' | 'month' | 'year'): string {
		switch (part) {
			case 'day':
				return words.day;
			case 'month':
				return words.month;
			case 'year':
				return words.year;
		}
	}

	const classes = $derived([
		'zf-input',
		'zf-date',
		'slip',
		error !== undefined && 'zf-input--invalid',
		disabled && 'zf-input--disabled'
	]);
</script>

<FieldFrame {label} {hint} {error}>
	{#snippet children(ids: FieldIds)}
		{#key generation}
			{#if Root !== undefined}
				<Root
					bind:value={() => shown, offer}
					placeholder={start}
					minValue={lowest}
					maxValue={highest}
					{locale}
					granularity="day"
					{disabled}
					{required}
				>
					<!-- bits-ui's group and spinbuttons, every word they say taken from `words`. -->
					<DateField.Input>
						{#snippet child({
							props,
							segments
						}: {
							readonly props: Record<string, unknown>;
							readonly segments: readonly { readonly part: string; readonly value: string }[];
						})}
							<div
								{...props}
								class={classes}
								aria-labelledby={ids.labelId}
								aria-describedby={ids.describedBy}
								data-testid={testid}
								onfocusin={enter}
								onfocusout={settle}
							>
								{#each segments as segment, index (index)}
									{#if segment.part === 'day' || segment.part === 'month' || segment.part === 'year'}
										{@const part = segment.part}
										{@const filled = DIGIT.test(segment.value)}
										<DateField.Segment {part} id="{id}-{part}">
											{#snippet child({ props }: { readonly props: Record<string, unknown> })}
												<span
													{...props}
													class={['zf-date__segment', !filled && 'zf-date__segment--empty']}
													aria-label={wordFor(part)}
													aria-labelledby="{id}-{part} {ids.labelId}"
													aria-describedby={ids.describedBy}
													aria-valuetext={filled ? undefined : words.empty}
													aria-invalid={ids.invalid}
													aria-required={required ? 'true' : undefined}
													>{filled ? segment.value : words.empty}</span
												>
											{/snippet}
										</DateField.Segment>
									{:else}
										<span class="zf-date__literal" aria-hidden="true">{segment.value}</span>
									{/if}
								{/each}
							</div>
						{/snippet}
					</DateField.Input>
					{#if name !== undefined}<input type="hidden" {name} value={value ?? ''} />{/if}
				</Root>
			{/if}
		{/key}
	{/snippet}
</FieldFrame>
