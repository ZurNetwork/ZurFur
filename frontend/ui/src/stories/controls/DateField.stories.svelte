<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn, userEvent, waitFor } from 'storybook/test';
	import { DateField } from '@zurfur/ui';
	import {
		DATE_TEXT,
		DATE_WORDS,
		DAY_FIRST,
		DUE_DAY,
		EARLIEST_DAY,
		LATEST_DAY,
		MONTH_FIRST,
		STEP_START_DAY
	} from '../fixtures/fields.ts';
	import type { DateFieldArgs } from './args.ts';
	import DateFieldHolder from './DateFieldHolder.svelte';

	const { Story } = defineMeta({
		title: 'Controls/Date Field',
		component: DateField,
		render: template,
		tags: ['autodocs'],
		args: {
			label: DATE_TEXT.label,
			value: undefined,
			onChange: fn(),
			...DAY_FIRST,
			words: DATE_WORDS
		},
		parameters: {
			docs: {
				description: {
					component:
						'A simple component: one field for a day, in three parts. The language sets the order of the parts. An empty part shows its own word, and a screen reader hears that it is empty. It keeps no copy of the day, so once an edit is over the field shows what its parent holds.\n\n**Keyboard**\n\n- Tab → reaches the first part\n- ← and → → move between the parts\n- ↑ and ↓ → change the part\n- Digits → type the part\n- Backspace → clears the part one digit at a time, and once it is empty the field holds no day'
				}
			}
		}
	});
</script>

{#snippet template(args: DateFieldArgs)}
	<DateFieldHolder
		label={args.label}
		value={args.value}
		onChange={args.onChange}
		locale={args.locale}
		words={args.words}
		min={args.min}
		max={args.max}
		startFrom={args.startFrom}
		hint={args.hint}
		error={args.error}
		disabled={args.disabled ?? false}
		required={args.required ?? false}
	/>
{/snippet}

<Story name="Playground" />

<Story name="With a Day" args={{ value: DUE_DAY }} />

<Story name="With a Hint" args={{ hint: DATE_TEXT.hint }} />

<Story
	name="With an Error"
	args={{ value: undefined, error: DATE_TEXT.error, min: EARLIEST_DAY }}
/>

<Story name="Month First" args={{ ...MONTH_FIRST, value: DUE_DAY }} />

<Story name="Required" args={{ required: true }} />

<Story name="Disabled" args={{ value: DUE_DAY, disabled: true }} />

<Story
	name="Typing"
	play={async ({ canvas, args }) => {
		const day = canvas.getByRole('spinbutton', { name: `day ${DATE_TEXT.label}` });
		await expect(day).toHaveTextContent('day');
		await expect(day).toHaveAttribute('aria-valuetext', 'empty');
		await userEvent.click(day);
		await userEvent.keyboard('18042026');
		await expect(args.onChange).toHaveBeenLastCalledWith('2026-04-18');
		await expect(day).toHaveTextContent('18');
		await expect(
			canvas.getByRole('spinbutton', { name: `month ${DATE_TEXT.label}` })
		).toHaveTextContent('04');
		await expect(
			canvas.getByRole('spinbutton', { name: `year ${DATE_TEXT.label}` })
		).toHaveTextContent('2026');
	}}
/>

<Story
	name="Keyboard"
	args={{ value: DUE_DAY, startFrom: STEP_START_DAY }}
	play={async ({ canvas, args }) => {
		await userEvent.tab();
		const day = canvas.getByRole('spinbutton', { name: `day ${DATE_TEXT.label}` });
		const month = canvas.getByRole('spinbutton', { name: `month ${DATE_TEXT.label}` });
		await expect(day).toHaveFocus();
		await userEvent.keyboard('{ArrowRight}');
		await expect(month).toHaveFocus();
		await userEvent.keyboard('{ArrowUp}');
		await expect(args.onChange).toHaveBeenLastCalledWith('2026-05-18');
		await expect(month).toHaveTextContent('05');
		await userEvent.keyboard('{ArrowDown}');
		await expect(args.onChange).toHaveBeenLastCalledWith('2026-04-18');
		await userEvent.keyboard('{ArrowLeft}');
		await expect(day).toHaveFocus();
		await userEvent.keyboard('{Backspace}');
		await expect(args.onChange).toHaveBeenLastCalledWith('2026-04-01');
		await userEvent.keyboard('{Backspace}');
		await expect(args.onChange).toHaveBeenLastCalledWith(undefined);
		await expect(day).toHaveTextContent('day');
		await expect(day).toHaveAttribute('aria-valuetext', 'empty');
	}}
/>

<Story
	name="Steps From a Day"
	args={{ startFrom: STEP_START_DAY }}
	play={async ({ canvas }) => {
		await userEvent.tab();
		const day = canvas.getByRole('spinbutton', { name: `day ${DATE_TEXT.label}` });
		await expect(day).toHaveFocus();
		await userEvent.keyboard('{ArrowUp}');
		await expect(day).toHaveTextContent('15');
		await expect(
			canvas.getByRole('spinbutton', { name: `month ${DATE_TEXT.label}` })
		).toHaveTextContent('month');
	}}
/>

<Story
	name="Within Limits"
	args={{ min: EARLIEST_DAY, max: LATEST_DAY }}
	play={async ({ canvas, canvasElement, args }) => {
		const dayName = `day ${DATE_TEXT.label}`;
		await userEvent.click(canvas.getByRole('spinbutton', { name: dayName }));
		await userEvent.keyboard('31122025');
		await expect(args.onChange).not.toHaveBeenCalled();
		const focused = canvasElement.ownerDocument.activeElement;
		if (focused instanceof HTMLElement) focused.blur();
		await waitFor(() =>
			expect(canvas.getByRole('spinbutton', { name: dayName })).toHaveTextContent('day')
		);
		await userEvent.click(canvas.getByRole('spinbutton', { name: dayName }));
		await userEvent.keyboard('01022026');
		await expect(args.onChange).toHaveBeenLastCalledWith('2026-02-01');
	}}
/>

<Story
	name="Focused"
	args={{ value: DUE_DAY }}
	play={async ({ canvas }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('spinbutton', { name: `day ${DATE_TEXT.label}` })).toHaveFocus();
	}}
/>

<Story
	name="Disabled Does Nothing"
	args={{ disabled: true }}
	play={async ({ canvas, args }) => {
		const day = canvas.getByRole('spinbutton', { name: `day ${DATE_TEXT.label}` });
		await userEvent.tab();
		await expect(day).not.toHaveFocus();
		await userEvent.type(day, '18', { pointerEventsCheck: 0 });
		await expect(args.onChange).not.toHaveBeenCalled();
	}}
/>
