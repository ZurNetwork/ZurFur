<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn, userEvent } from 'storybook/test';
	import { Select } from '@zurfur/ui';
	import { SELECT_WORDS, STAGES } from '../fixtures/fields.ts';
	import type { SelectArgs } from './args.ts';
	import SelectHolder from './SelectHolder.svelte';

	const { Story } = defineMeta({
		title: 'Controls/Select',
		component: Select,
		render: template,
		tags: ['autodocs'],
		args: {
			label: SELECT_WORDS.label,
			options: STAGES,
			value: 'sketch',
			onChange: fn()
		},
		parameters: {
			docs: {
				description: {
					component:
						'A simple component: a labelled list of choices that opens on the platform’s own picker. Use it for short, known lists. It keeps no copy of the choice, so what shows is always what its parent hands it.\n\n**Keyboard**\n\n- Tab → reaches the select\n- ↑ and ↓ → pick the previous or next option\n- Enter or Space → opens the list\n- Esc → closes the list\n- Tab again → moves on'
				}
			}
		}
	});
</script>

{#snippet template(args: SelectArgs)}
	<SelectHolder
		label={args.label}
		options={args.options}
		value={args.value}
		onChange={args.onChange}
		placeholder={args.placeholder}
		hint={args.hint}
		error={args.error}
		disabled={args.disabled ?? false}
		required={args.required ?? false}
	/>
{/snippet}

<Story name="Playground" />

<Story
	name="With a Placeholder"
	args={{ value: undefined, placeholder: SELECT_WORDS.placeholder }}
/>

<Story name="With a Hint" args={{ hint: SELECT_WORDS.hint }} />

<Story
	name="With an Error"
	args={{ value: undefined, placeholder: SELECT_WORDS.placeholder, error: SELECT_WORDS.error }}
/>

<Story name="Required" args={{ required: true }} />

<Story name="Disabled" args={{ value: 'lineart', disabled: true }} />

<Story
	name="Pick"
	play={async ({ canvas, args }) => {
		const select = canvas.getByRole('combobox', { name: SELECT_WORDS.label });
		await expect(select).toHaveDisplayValue('Sketch');
		await userEvent.selectOptions(select, 'colour');
		await expect(args.onChange).toHaveBeenCalledTimes(1);
		await expect(args.onChange).toHaveBeenCalledWith('colour');
		await expect(select).toHaveDisplayValue('Colour');
	}}
/>

<Story
	name="Keyboard"
	play={async ({ canvas, args }) => {
		await userEvent.tab();
		const select = canvas.getByRole('combobox', { name: SELECT_WORDS.label });
		await expect(select).toHaveFocus();
		await userEvent.selectOptions(select, 'lineart');
		await expect(args.onChange).toHaveBeenLastCalledWith('lineart');
		await expect(select).toHaveDisplayValue('Lineart');
		await expect(select).toHaveFocus();
		await userEvent.tab();
		await expect(select).not.toHaveFocus();
	}}
/>

<Story
	name="Focused"
	play={async ({ canvas }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('combobox', { name: SELECT_WORDS.label })).toHaveFocus();
	}}
/>

<Story
	name="Disabled Does Nothing"
	args={{ disabled: true }}
	play={async ({ canvas, args }) => {
		const select = canvas.getByRole('combobox', { name: SELECT_WORDS.label });
		await expect(select).toBeDisabled();
		await userEvent.tab();
		await expect(select).not.toHaveFocus();
		await userEvent.keyboard('{ArrowDown}');
		await expect(args.onChange).not.toHaveBeenCalled();
		await expect(select).toHaveDisplayValue('Sketch');
	}}
/>
