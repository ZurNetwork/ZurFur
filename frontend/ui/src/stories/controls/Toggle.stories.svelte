<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn } from 'storybook/test';
	import { Toggle } from '@zurfur/ui';
	import { showQueue } from '../fixtures/actions.ts';
	import type { ToggleArgs } from './args.ts';
	import ToggleHolder from './ToggleHolder.svelte';

	const { Story } = defineMeta({
		title: 'Controls/Toggle',
		component: Toggle,
		tags: ['autodocs', 'simple'],
		args: { label: showQueue.label, checked: false, onCheckedChange: fn() },
		argTypes: {
			checked: { control: 'boolean' },
			disabled: { control: 'boolean' },
			name: { table: { disable: true } },
			testid: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: a switch with a label, for a setting that takes effect at once. A line under the label can say more. The page decides the value: a flip asks for the next one, and the switch shows what the page passes back. Pressing the label flips it too.

Keyboard:

- Tab: reaches it
- Space: flips it
- Enter: flips it`
				}
			}
		}
	});
</script>

{#snippet holder(args: ToggleArgs)}
	<ToggleHolder
		label={args.label}
		description={args.description}
		checked={args.checked}
		onCheckedChange={args.onCheckedChange}
		disabled={args.disabled}
		name={args.name}
		testid={args.testid}
	/>
{/snippet}

<Story
	name="Off"
	template={holder}
	play={async ({ args, canvas, userEvent }) => {
		const toggle = canvas.getByRole('switch', { name: showQueue.label });
		await expect(toggle).toHaveAttribute('aria-checked', 'false');
		await userEvent.click(toggle);
		await expect(args.onCheckedChange).toHaveBeenLastCalledWith(true);
		await expect(toggle).toHaveAttribute('aria-checked', 'true');
		await userEvent.click(canvas.getByText(showQueue.label));
		await expect(args.onCheckedChange).toHaveBeenLastCalledWith(false);
		await expect(toggle).toHaveAttribute('aria-checked', 'false');
	}}
/>

<Story
	name="On"
	template={holder}
	args={{ checked: true }}
	play={async ({ canvas }) => {
		await expect(canvas.getByRole('switch', { name: showQueue.label })).toHaveAttribute(
			'aria-checked',
			'true'
		);
	}}
/>

<Story
	name="With a Description"
	template={holder}
	args={{ description: showQueue.description }}
	play={async ({ canvas }) => {
		await expect(canvas.getByRole('switch', { name: showQueue.label })).toHaveAccessibleDescription(
			showQueue.description
		);
	}}
/>

<Story
	name="Disabled"
	template={holder}
	args={{ disabled: true }}
	play={async ({ args, canvas, userEvent }) => {
		const toggle = canvas.getByRole('switch', { name: showQueue.label });
		await expect(toggle).toBeDisabled();
		await userEvent.click(toggle);
		await expect(args.onCheckedChange).not.toHaveBeenCalled();
	}}
/>

<Story
	name="Keyboard"
	template={holder}
	play={async ({ args, canvas, userEvent }) => {
		await userEvent.tab();
		const toggle = canvas.getByRole('switch', { name: showQueue.label });
		await expect(toggle).toHaveFocus();
		await userEvent.keyboard(' ');
		await expect(args.onCheckedChange).toHaveBeenLastCalledWith(true);
		await expect(toggle).toHaveAttribute('aria-checked', 'true');
		await userEvent.keyboard('{Enter}');
		await expect(args.onCheckedChange).toHaveBeenLastCalledWith(false);
		await expect(toggle).toHaveAttribute('aria-checked', 'false');
	}}
/>

<Story
	name="Focused"
	template={holder}
	play={async ({ canvas, userEvent }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('switch', { name: showQueue.label })).toHaveFocus();
	}}
/>

<Story name="Playground" template={holder} args={{ description: showQueue.description }} />
