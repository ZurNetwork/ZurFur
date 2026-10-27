<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn, userEvent, waitFor, within } from 'storybook/test';
	import { Select } from '@zurfur/ui';
	import { SELECT_WORDS, STAGES } from '../fixtures/fields.ts';
	import type { SelectArgs } from './args.ts';
	import { FLOATING_ROOM } from './room.ts';
	import SelectHolder from './SelectHolder.svelte';

	/** The page the list portals into, so a play can find it wherever it opened. */
	function wholePage(canvasElement: HTMLElement) {
		return within(canvasElement.ownerDocument.body);
	}

	const { Story } = defineMeta({
		title: 'Controls/Select',
		component: Select,
		render: template,
		tags: ['autodocs', 'simple'],
		args: {
			label: SELECT_WORDS.label,
			options: STAGES,
			value: 'sketch',
			onChange: fn()
		},
		parameters: {
			docs: {
				story: { height: FLOATING_ROOM },
				description: {
					component: `A simple component: a labelled field that opens a short list of choices. Use it for short, known lists. It keeps no copy of the choice, so what shows is always what its parent hands it.

Keyboard:

- Tab: reaches the field
- Enter, Space, ↓ or ↑: opens the list
- Type a letter: chooses the option that starts with it, or jumps to it while the list is open
- ↑ and ↓: move between options
- Home and End: jump to the first or last option
- Page Up and Page Down: jump ten options
- Enter or Space: chooses the option and closes the list
- Esc or Tab: closes the list`
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
	name="Opened"
	play={async ({ canvas, canvasElement }) => {
		const trigger = canvas.getByRole('combobox', { name: SELECT_WORDS.label });
		await userEvent.click(trigger);
		const list = await wholePage(canvasElement).findByRole('listbox', {
			name: SELECT_WORDS.label
		});
		await expect(list).toBeVisible();
		await expect(trigger).toHaveAttribute('aria-controls', list.id);
		const rows = within(list).getAllByRole('option');
		await expect(rows.map((row) => row.textContent.trim())).toEqual([
			'Sketch',
			'Lineart',
			'Colour',
			'Final'
		]);
		await expect(within(list).getByRole('option', { name: 'Sketch' })).toHaveAttribute(
			'aria-selected',
			'true'
		);
	}}
/>

<Story
	name="Pick"
	play={async ({ canvas, canvasElement, args }) => {
		const trigger = canvas.getByRole('combobox', { name: SELECT_WORDS.label });
		await expect(trigger).toHaveTextContent('Sketch');
		await userEvent.click(trigger);
		const list = await wholePage(canvasElement).findByRole('listbox');
		await userEvent.click(within(list).getByRole('option', { name: 'Colour' }));
		await expect(args.onChange).toHaveBeenCalledTimes(1);
		await expect(args.onChange).toHaveBeenCalledWith('colour');
		await expect(trigger).toHaveTextContent('Colour');
		await waitFor(() => expect(wholePage(canvasElement).queryByRole('listbox')).toBeNull());
	}}
/>

<Story
	name="Keyboard"
	play={async ({ canvas, canvasElement, args }) => {
		const page = wholePage(canvasElement);
		const trigger = canvas.getByRole('combobox', { name: SELECT_WORDS.label });
		await userEvent.tab();
		await expect(trigger).toHaveFocus();
		await userEvent.keyboard('{Enter}');
		const list = await page.findByRole('listbox');
		const row = (name: string) => within(list).getByRole('option', { name });
		await waitFor(() => expect(row('Sketch')).toHaveAttribute('data-highlighted'));
		await userEvent.keyboard('{ArrowDown}');
		await userEvent.keyboard('{ArrowDown}');
		await expect(row('Colour')).toHaveAttribute('data-highlighted');
		await userEvent.keyboard('{Enter}');
		await expect(args.onChange).toHaveBeenCalledTimes(1);
		await expect(args.onChange).toHaveBeenCalledWith('colour');
		await waitFor(() => expect(page.queryByRole('listbox')).toBeNull());
		await expect(trigger).toHaveFocus();
		await expect(trigger).toHaveTextContent('Colour');
		await userEvent.keyboard('{Enter}');
		const again = await page.findByRole('listbox');
		const rowAgain = (name: string) => within(again).getByRole('option', { name });
		await userEvent.keyboard('{Home}');
		await expect(rowAgain('Sketch')).toHaveAttribute('data-highlighted');
		await userEvent.keyboard('{End}');
		await expect(rowAgain('Final')).toHaveAttribute('data-highlighted');
		await userEvent.keyboard('{Escape}');
		await waitFor(() => expect(page.queryByRole('listbox')).toBeNull());
		await expect(trigger).toHaveFocus();
		await expect(args.onChange).toHaveBeenCalledTimes(1);
	}}
/>

<Story
	name="Type to Find"
	play={async ({ canvas, canvasElement, args }) => {
		const page = wholePage(canvasElement);
		const trigger = canvas.getByRole('combobox', { name: SELECT_WORDS.label });
		await userEvent.tab();
		await expect(trigger).toHaveFocus();
		await userEvent.keyboard('l');
		await expect(args.onChange).toHaveBeenCalledTimes(1);
		await expect(args.onChange).toHaveBeenCalledWith('lineart');
		await expect(trigger).toHaveTextContent('Lineart');
		await expect(page.queryByRole('listbox')).toBeNull();
		await userEvent.keyboard('{Enter}');
		const list = await page.findByRole('listbox');
		await userEvent.keyboard('c');
		await expect(within(list).getByRole('option', { name: 'Colour' })).toHaveAttribute(
			'data-highlighted'
		);
		await expect(args.onChange).toHaveBeenCalledTimes(1);
	}}
/>

<Story
	name="Release on an Option Chooses Nothing"
	play={async ({ canvas, canvasElement, args, userEvent: user }) => {
		const trigger = canvas.getByRole('combobox', { name: SELECT_WORDS.label });
		await user.pointer({ keys: '[MouseLeft>]', target: trigger });
		const list = await wholePage(canvasElement).findByRole('listbox');
		const row = within(list).getByRole('option', { name: 'Lineart' });
		await user.pointer({ keys: '[/MouseLeft]', target: row });
		await expect(args.onChange).not.toHaveBeenCalled();
		await expect(list).toBeVisible();
		await expect(trigger).toHaveTextContent('Sketch');
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
	play={async ({ canvas, canvasElement, args }) => {
		const trigger = canvas.getByRole('combobox', { name: SELECT_WORDS.label });
		await expect(trigger).toBeDisabled();
		await userEvent.tab();
		await expect(trigger).not.toHaveFocus();
		await userEvent.click(trigger, { pointerEventsCheck: 0 });
		await userEvent.keyboard('{ArrowDown}');
		await expect(wholePage(canvasElement).queryByRole('listbox')).toBeNull();
		await expect(args.onChange).not.toHaveBeenCalled();
		await expect(trigger).toHaveTextContent('Sketch');
	}}
/>
