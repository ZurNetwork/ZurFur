<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn } from 'storybook/test';
	import { PillSwitch } from '@zurfur/ui';
	import {
		batchOptions,
		filterOptions,
		mixedOptions,
		stageOptions,
		viewOptions
	} from '../fixtures/actions.ts';
	import type { PillSwitchArgs } from './args.ts';
	import PillSwitchHolder from './PillSwitchHolder.svelte';

	const { Story } = defineMeta({
		title: 'Controls/Pill Switch',
		component: PillSwitch,
		tags: ['autodocs', 'simple'],
		args: { label: 'Filter', options: filterOptions, value: 'all', onChange: fn() },
		argTypes: {
			disabled: { control: 'boolean' },
			name: { table: { disable: true } },
			testid: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: a row of pills where exactly one is chosen, such as a filter. Each pill has a label, and can have an icon or a muted type in front of it. A type all the pills share shows once, beside the pills. The page decides the value: a choice asks for it, and the pills show what the page passes back. An option can be disabled, or the whole row.

Keyboard:

- Tab: reaches the chosen pill
- ← or →: moves to the previous or next pill and chooses it, wrapping round the ends
- ↑ or ↓: does the same
- Home: chooses the first pill
- End: chooses the last pill
- Space: chooses the pill that has focus`
				}
			}
		}
	});
</script>

{#snippet holder(args: PillSwitchArgs)}
	<PillSwitchHolder
		label={args.label}
		options={args.options}
		value={args.value}
		onChange={args.onChange}
		disabled={args.disabled}
		name={args.name}
		testid={args.testid}
	/>
{/snippet}

<Story
	name="Plain"
	template={holder}
	play={async ({ args, canvas, userEvent }) => {
		await expect(canvas.getByRole('radio', { name: 'All' })).toBeChecked();
		await userEvent.click(canvas.getByRole('radio', { name: 'Stickers' }));
		await expect(args.onChange).toHaveBeenLastCalledWith('stickers');
		await expect(canvas.getByRole('radio', { name: 'Stickers' })).toBeChecked();
		await expect(canvas.getByRole('radio', { name: 'All' })).not.toBeChecked();
	}}
/>

<Story
	name="With a Shared Type"
	template={holder}
	args={{ label: 'Batch', options: batchOptions }}
	play={async ({ args, canvas, userEvent }) => {
		await expect(canvas.getAllByText('batch:')).toHaveLength(1);
		await userEvent.click(canvas.getByRole('radio', { name: 'stickers' }));
		await expect(args.onChange).toHaveBeenLastCalledWith('stickers');
		await expect(canvas.getByRole('radio', { name: 'stickers' })).toBeChecked();
	}}
/>

<Story
	name="With Mixed Types"
	template={holder}
	args={{ label: 'Filter', options: mixedOptions }}
	play={async ({ args, canvas, userEvent }) => {
		await expect(canvas.getByText('batch:')).toBeInTheDocument();
		await expect(canvas.getByText('phase:')).toBeInTheDocument();
		await userEvent.click(canvas.getByRole('radio', { name: /sketch$/ }));
		await expect(args.onChange).toHaveBeenLastCalledWith('sketch');
	}}
/>

<Story
	name="With Icons"
	template={holder}
	args={{ label: 'View', options: viewOptions, value: 'list' }}
	play={async ({ args, canvas, userEvent }) => {
		await userEvent.click(canvas.getByRole('radio', { name: 'Files' }));
		await expect(args.onChange).toHaveBeenLastCalledWith('files');
		await expect(canvas.getByRole('radio', { name: 'Files' })).toBeChecked();
	}}
/>

<Story
	name="With a Disabled Option"
	template={holder}
	args={{ label: 'Stage', options: stageOptions, value: 'sketch' }}
	play={async ({ args, canvas, userEvent }) => {
		const colour = canvas.getByRole('radio', { name: 'Colour' });
		await expect(colour).toBeDisabled();
		await userEvent.click(colour);
		await expect(args.onChange).not.toHaveBeenCalled();
		await expect(canvas.getByRole('radio', { name: 'Sketch' })).toBeChecked();
	}}
/>

<Story
	name="Disabled"
	template={holder}
	args={{ disabled: true }}
	play={async ({ args, canvas, userEvent }) => {
		const stickers = canvas.getByRole('radio', { name: 'Stickers' });
		await expect(stickers).toBeDisabled();
		await userEvent.click(stickers);
		await expect(args.onChange).not.toHaveBeenCalled();
	}}
/>

<Story
	name="Keyboard"
	template={holder}
	play={async ({ args, canvas, userEvent }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('radio', { name: 'All' })).toHaveFocus();
		await userEvent.keyboard('{ArrowRight}');
		await expect(args.onChange).toHaveBeenLastCalledWith('sketches');
		await expect(canvas.getByRole('radio', { name: 'Sketches' })).toHaveFocus();
		await expect(canvas.getByRole('radio', { name: 'Sketches' })).toBeChecked();
		await userEvent.keyboard('{End}');
		await expect(args.onChange).toHaveBeenLastCalledWith('stickers');
		await userEvent.keyboard('{Home}');
		await expect(args.onChange).toHaveBeenLastCalledWith('all');
		await userEvent.keyboard('{ArrowLeft}');
		await expect(args.onChange).toHaveBeenLastCalledWith('stickers');
		await expect(canvas.getByRole('radio', { name: 'Stickers' })).toHaveFocus();
		await expect(args.onChange).toHaveBeenCalledTimes(4);
	}}
/>

<Story
	name="Focused"
	template={holder}
	play={async ({ canvas, userEvent }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('radio', { name: 'All' })).toHaveFocus();
	}}
/>

<Story name="Playground" template={holder} />
