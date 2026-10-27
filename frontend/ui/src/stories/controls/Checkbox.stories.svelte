<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn } from 'storybook/test';
	import { Checkbox } from '@zurfur/ui';
	import { selectAllSketches, shareSketches } from '../fixtures/actions.ts';
	import type { CheckboxArgs } from './args.ts';
	import CheckboxHolder from './CheckboxHolder.svelte';

	const { Story } = defineMeta({
		title: 'Controls/Checkbox',
		component: Checkbox,
		tags: ['autodocs'],
		args: { label: shareSketches.label, checked: false, onCheckedChange: fn() },
		argTypes: {
			checked: {
				control: 'inline-radio',
				options: ['unchecked', 'checked', 'mixed'],
				mapping: { unchecked: false, checked: true, mixed: 'mixed' }
			},
			disabled: { control: 'boolean' },
			name: { table: { disable: true } },
			value: { table: { disable: true } },
			testid: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: a box with a label, for a choice that is on or off, or Mixed when only some of what it stands for is on. A line under the label can say more. The page decides the value: a press asks for the next one, and the box shows what the page passes back. Pressing the box or its words does the same.

Keyboard:

- Tab: reaches it
- Space: ticks or unticks it`
				}
			}
		}
	});
</script>

{#snippet holder(args: CheckboxArgs)}
	<CheckboxHolder
		label={args.label}
		description={args.description}
		checked={args.checked}
		onCheckedChange={args.onCheckedChange}
		disabled={args.disabled}
		name={args.name}
		value={args.value}
		testid={args.testid}
	/>
{/snippet}

<Story
	name="Unchecked"
	template={holder}
	play={async ({ args, canvas, userEvent }) => {
		const box = canvas.getByRole('checkbox', { name: shareSketches.label });
		await expect(box).not.toBeChecked();
		await userEvent.click(canvas.getByText(shareSketches.label));
		await expect(args.onCheckedChange).toHaveBeenLastCalledWith(true);
		await expect(box).toBeChecked();
		await userEvent.click(canvas.getByText(shareSketches.label));
		await expect(args.onCheckedChange).toHaveBeenLastCalledWith(false);
		await expect(box).not.toBeChecked();
	}}
/>

<Story
	name="Checked"
	template={holder}
	args={{ checked: true }}
	play={async ({ canvas }) => {
		await expect(canvas.getByRole('checkbox', { name: shareSketches.label })).toBeChecked();
	}}
/>

<Story
	name="Mixed"
	template={holder}
	args={{ label: selectAllSketches.label, checked: 'mixed' }}
	play={async ({ canvas }) => {
		await expect(
			canvas.getByRole('checkbox', { name: selectAllSketches.label })
		).toBePartiallyChecked();
	}}
/>

<Story
	name="With a Description"
	template={holder}
	args={{ description: shareSketches.description }}
	play={async ({ canvas }) => {
		await expect(
			canvas.getByRole('checkbox', { name: shareSketches.label })
		).toHaveAccessibleDescription(shareSketches.description);
	}}
/>

<Story
	name="Disabled"
	template={holder}
	args={{ disabled: true }}
	play={async ({ args, canvas, userEvent }) => {
		await expect(canvas.getByRole('checkbox', { name: shareSketches.label })).toBeDisabled();
		await userEvent.click(canvas.getByText(shareSketches.label));
		await expect(args.onCheckedChange).not.toHaveBeenCalled();
	}}
/>

<Story
	name="Keyboard"
	template={holder}
	play={async ({ args, canvas, userEvent }) => {
		await userEvent.tab();
		const box = canvas.getByRole('checkbox', { name: shareSketches.label });
		await expect(box).toHaveFocus();
		await userEvent.keyboard(' ');
		await expect(args.onCheckedChange).toHaveBeenLastCalledWith(true);
		await expect(box).toBeChecked();
		await userEvent.keyboard(' ');
		await expect(args.onCheckedChange).toHaveBeenLastCalledWith(false);
		await expect(box).not.toBeChecked();
	}}
/>

<Story
	name="Focused"
	template={holder}
	play={async ({ canvas, userEvent }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('checkbox', { name: shareSketches.label })).toHaveFocus();
	}}
/>

<Story name="Playground" template={holder} args={{ description: shareSketches.description }} />
